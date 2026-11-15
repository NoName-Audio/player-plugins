use std::env;
use std::fs;
use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::net::ToSocketAddrs;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use tokio::sync::mpsc::UnboundedSender;

use crate::client::ClientOperation;
use crate::error::SoulseekRs;
use crate::message::server::MessageFactory;
use crate::token::PeerTransferToken;
use crate::types::{Download, DownloadStatus};
use crate::{error, trace};

const START_DOWNLOAD: [u8; 8] = [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
const READ_BUFFER_SIZE: usize = 8192;
const PROGRESS_UPDATE_CHUNKS: usize = 15; // ~120KB (15 * 8192 bytes)
/// How often the read loop wakes up to check cancel/timeout flags when the peer is silent.
const READ_CHECK_INTERVAL: Duration = Duration::from_secs(1);
/// Hard stall timeout: give up if no bytes arrive within this window.
const STALL_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug)]
pub enum DownloadError {
    ConnectionFailed(io::Error),
    InvalidAddress(String),
    HandshakeFailed(io::Error),
    StreamReadError(io::Error),
    StreamWriteError(io::Error),
    TokenNotFound(PeerTransferToken),
    DownloadInfoMissing,
    FileWriteError(io::Error),
    PathResolutionError(String),
    InvalidTokenBytes,
    /// Download was cancelled via [`Download::cancel`].
    Cancelled,
    /// No progress received within the configured timeout duration.
    NoProgressTimeout,
}

impl From<DownloadError> for SoulseekRs {
    fn from(e: DownloadError) -> Self {
        match e {
            DownloadError::Cancelled => SoulseekRs::DownloadCancelled,
            DownloadError::NoProgressTimeout => SoulseekRs::DownloadTimedOut,
            other => {
                let category = match &other {
                    DownloadError::ConnectionFailed(e) => match e.kind() {
                        io::ErrorKind::ConnectionRefused => "peer_connection_refused",
                        io::ErrorKind::TimedOut => "peer_connect_timeout",
                        _ => "peer_connect_failed",
                    },
                    DownloadError::HandshakeFailed(_) => "peer_handshake_failed",
                    DownloadError::StreamReadError(e) => match e.kind() {
                        io::ErrorKind::UnexpectedEof => "peer_truncated_transfer",
                        io::ErrorKind::TimedOut => "peer_read_timeout",
                        io::ErrorKind::ConnectionReset => "peer_connection_reset",
                        _ => "peer_read_failed",
                    },
                    DownloadError::StreamWriteError(_) => "peer_write_failed",
                    DownloadError::FileWriteError(_) => "local_file_write_failed",
                    DownloadError::PathResolutionError(_) => "local_path_failed",
                    DownloadError::InvalidAddress(_) => "peer_address_invalid",
                    _ => "peer_transfer_protocol_failed",
                };
                SoulseekRs::DownloadFailure { category, source: Box::new(other) }
            },
        }
    }
}

/// Spawns a blocking task that runs a direct download and reports the result
/// back to the worker via [`ClientOperation::DownloadCompleted`].
///
/// Use this for all `download_direct` paths (outbound and inbound) so error
/// mapping and the send pattern stay in one place.
pub fn spawn_direct_download(
    download: Download,
    host: String,
    port: u32,
    peer_token: u32,
    own_username: String,
    stream: Option<TcpStream>,
    op_tx: UnboundedSender<ClientOperation>,
) {
    let token = download.token;
    let peer = DownloadPeer::new(
        download.username.clone(),
        host.clone(),
        port,
        peer_token,
        own_username,
    );
    tokio::task::spawn_blocking(move || {
        let result = peer
            .download_direct(download, stream)
            .map(|(_, path)| path)
            .map_err(|e| {
                if !matches!(
                    e,
                    DownloadError::Cancelled | DownloadError::NoProgressTimeout
                ) {
                    error!(
                        "soulseek_socket socket=F phase=failed error={}", e
                    );
                }
                SoulseekRs::from(e)
            });
        let _ = op_tx.send(ClientOperation::DownloadCompleted(token, result));
    });
}

impl std::fmt::Display for DownloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConnectionFailed(e) => write!(f, "Connection failed: {}", e),
            Self::InvalidAddress(addr) => write!(f, "Invalid address: {}", addr),
            Self::HandshakeFailed(e) => write!(f, "Handshake failed: {}", e),
            Self::StreamReadError(e) => write!(f, "Stream read error: {}", e),
            Self::StreamWriteError(e) => write!(f, "Stream write error: {}", e),
            Self::TokenNotFound(token) => write!(f, "Token not found: {}", token),
            Self::DownloadInfoMissing => write!(f, "Download info missing"),
            Self::FileWriteError(e) => write!(f, "File write error: {}", e),
            Self::PathResolutionError(msg) => write!(f, "Path resolution error: {}", msg),
            Self::InvalidTokenBytes => write!(f, "Invalid token bytes received"),
            Self::Cancelled => write!(f, "Download cancelled"),
            Self::NoProgressTimeout => write!(f, "Download timed out (no progress)"),
        }
    }
}

impl std::error::Error for DownloadError {}

impl From<io::Error> for DownloadError {
    fn from(error: io::Error) -> Self {
        Self::StreamReadError(error)
    }
}

struct FileManager;

impl FileManager {
    fn expand_path(path: &str) -> PathBuf {
        if let Some(stripped) = path.strip_prefix('~') {
            if let Ok(home) = env::var("HOME") {
                PathBuf::from(home).join(stripped.trim_start_matches('/'))
            } else {
                PathBuf::from(path)
            }
        } else {
            PathBuf::from(path)
        }
    }
}

pub struct DownloadPeer {
    username: String,
    host: String,
    port: u32,
    #[allow(dead_code)]
    own_username: String,
    /// Token sent in the pierce-firewall handshake (pierce path only).
    token: u32,
}

impl DownloadPeer {
    #[must_use]
    pub fn new(
        username: String,
        host: String,
        port: u32,
        token: u32,
        own_username: String,
    ) -> Self {
        Self {
            username,
            host,
            port,
            own_username,
            token,
        }
    }

    fn establish_connection(&self) -> Result<TcpStream, DownloadError> {
        let socket_address = format!("{}:{}", self.host, self.port)
            .to_socket_addrs()
            .map_err(DownloadError::ConnectionFailed)?
            .next()
            .ok_or_else(|| DownloadError::InvalidAddress(format!("{}:{}", self.host, self.port)))?;

        let stream = TcpStream::connect_timeout(&socket_address, Duration::from_secs(20))
            .map_err(DownloadError::ConnectionFailed)?;

        // Use a short read timeout so the read loop can check cancel/progress-timeout flags
        // between poll intervals rather than blocking for 30 s on a stalled peer.
        stream
            .set_read_timeout(Some(READ_CHECK_INTERVAL))
            .map_err(DownloadError::ConnectionFailed)?;
        stream
            .set_write_timeout(Some(Duration::from_secs(5)))
            .map_err(DownloadError::ConnectionFailed)?;
        stream
            .set_nodelay(true)
            .map_err(DownloadError::ConnectionFailed)?;

        Ok(stream)
    }

    fn open_output_file(path: &str) -> Result<io::BufWriter<fs::File>, DownloadError> {
        if let Some(parent) = Path::new(path).parent() {
            fs::create_dir_all(parent).map_err(DownloadError::FileWriteError)?;
        }
        let f = fs::File::create(path).map_err(DownloadError::FileWriteError)?;
        Ok(io::BufWriter::new(f))
    }

    fn resolve_download_path(download: &Download) -> Result<String, DownloadError> {
        let download_directory = &download.download_directory;
        let mut expanded_path = FileManager::expand_path(download_directory);

        if !expanded_path.is_dir() {
            expanded_path = expanded_path
                .parent()
                .ok_or_else(|| {
                    DownloadError::PathResolutionError(format!(
                        "Cannot resolve parent directory for: {}",
                        expanded_path.display()
                    ))
                })?
                .to_path_buf();
        }

        let final_path = expanded_path.join(download.filename.filename());

        final_path
            .to_str()
            .ok_or_else(|| {
                DownloadError::PathResolutionError(format!(
                    "Path contains invalid UTF-8: {}",
                    final_path.display()
                ))
            })
            .map(String::from)
    }

    /// Download a file where the download info is known upfront (direct connection).
    ///
    /// Sends `START_DOWNLOAD`, then streams data to disk. Progress is reported
    /// via `download.sender` — no shared context lock is held during I/O.
    pub fn download_direct(
        self,
        download: Download,
        stream: Option<TcpStream>,
    ) -> Result<(Download, String), DownloadError> {
        trace!(
            "[download_peer:{}] download_direct, stream present: {}",
            self.username,
            stream.is_some()
        );

        let _ = download.sender.send(DownloadStatus::Protocol {
            phase: if stream.is_some() { "file_inbound_token_received" } else { "file_outbound_connect" },
            bytes: 0, total: download.size,
        });
        let mut stream = match stream {
            Some(s) => {
                s.set_nonblocking(false)
                    .map_err(DownloadError::ConnectionFailed)?;
                s.set_read_timeout(Some(READ_CHECK_INTERVAL))
                    .map_err(DownloadError::ConnectionFailed)?;
                s.set_write_timeout(Some(Duration::from_secs(5)))
                    .map_err(DownloadError::ConnectionFailed)?;
                s.set_nodelay(true)
                    .map_err(DownloadError::ConnectionFailed)?;
                s
            }
            None => {
                let mut stream = self.establish_connection()?;
                // The uploader sends FileTransferInit on every F socket, even
                // when the downloader established TCP. See Nicotine+'s
                // slskproto.py _process_file_init_message/_process_file_output.
                let init = MessageFactory::build_peer_init_message(
                    &self.own_username,
                    crate::peer::ConnectionType::F,
                    0,
                );
                stream.write_all(&init.get_buffer()).map_err(DownloadError::HandshakeFailed)?;
                crate::info!("soulseek_socket socket=F direction=outbound phase=peer_init_sent");
                let _ = download.sender.send(DownloadStatus::Protocol { phase: "file_outbound_peer_init_sent", bytes: 0, total: download.size });
                let mut transfer_token = [0; 4];
                stream.read_exact(&mut transfer_token).map_err(DownloadError::HandshakeFailed)?;
                if u32::from_le_bytes(transfer_token) != self.token {
                    return Err(DownloadError::InvalidTokenBytes);
                }
                crate::info!("soulseek_socket socket=F direction=outbound phase=transfer_token_received bytes=4");
                let _ = download.sender.send(DownloadStatus::Protocol { phase: "file_outbound_token_received", bytes: 4, total: download.size });
                stream
            },
        };

        let path = Self::resolve_download_path(&download)?;
        let mut writer = Self::open_output_file(&path)?;

        stream
            .write_all(&START_DOWNLOAD)
            .map_err(DownloadError::StreamWriteError)?;
        crate::info!("soulseek_socket socket=F phase=offset_sent bytes=8 expected_bytes={}", download.size);
        let _ = download.sender.send(DownloadStatus::Protocol { phase: "file_offset_sent", bytes: 8, total: download.size });

        trace!("[download_peer:{}] sent START_DOWNLOAD", self.username);

        let _total_bytes = Self::read_stream(&self.username, &mut stream, &mut writer, &download)?;

        writer.flush().map_err(DownloadError::FileWriteError)?;

        trace!(
            "[download_peer:{}] download_direct complete: {} bytes → {}",
            self.username, _total_bytes, path
        );

        Ok((download, path))
    }

    /// Download a file where the download info is resolved from the first bytes of the stream
    /// (pierce-firewall path).
    ///
    /// Sends a pierce-firewall handshake, reads a 4-byte token from the peer,
    /// calls `resolve_download` to look up the corresponding `Download`,
    /// then streams data to disk.
    ///
    /// On error, returns `(Option<PeerTransferToken>, DownloadError)` where the token is `Some` if it
    /// was resolved before the failure, or `None` if the failure occurred during the handshake.
    pub fn download_pierced(
        self,
        resolve_download: impl Fn(PeerTransferToken) -> Option<Download>,
        stream: Option<TcpStream>,
    ) -> Result<(Download, String), (Option<PeerTransferToken>, DownloadError)> {
        trace!(
            "[download_peer:{}] download_pierced, stream present: {}",
            self.username,
            stream.is_some()
        );

        let mut stream = match stream {
            Some(s) => {
                s.set_nonblocking(false)
                    .map_err(|e| (None, DownloadError::ConnectionFailed(e)))?;
                s.set_read_timeout(Some(READ_CHECK_INTERVAL))
                    .map_err(|e| (None, DownloadError::ConnectionFailed(e)))?;
                s.set_write_timeout(Some(Duration::from_secs(5)))
                    .map_err(|e| (None, DownloadError::ConnectionFailed(e)))?;
                s.set_nodelay(true)
                    .map_err(|e| (None, DownloadError::ConnectionFailed(e)))?;
                s
            }
            None => self.establish_connection().map_err(|e| (None, e))?,
        };

        // Send pierce-firewall message so the peer can identify this connection.
        let message = MessageFactory::build_pierce_firewall_message(self.token);
        stream
            .write_all(&message.get_buffer())
            .map_err(|e| (None, DownloadError::HandshakeFailed(e)))?;
        stream
            .flush()
            .map_err(|e| (None, DownloadError::HandshakeFailed(e)))?;

        trace!(
            "[download_peer:{}] sent pierce firewall token: {}",
            self.username, self.token
        );

        // Consume only the raw transfer token; file bytes remain on the socket.
        let mut first_buf = [0u8; 4];
        let mut first_read = 0usize;
        let mut last_data_time = Instant::now();

        while first_read < 4 {
            match stream.read(&mut first_buf[first_read..4]) {
                Ok(0) => {
                    return Err((
                        None,
                        DownloadError::StreamReadError(io::Error::new(
                            io::ErrorKind::UnexpectedEof,
                            "peer closed connection before sending download token",
                        )),
                    ));
                }
                Ok(bytes_read) => {
                    first_read += bytes_read;
                    last_data_time = Instant::now();
                }
                Err(e)
                    if e.kind() == io::ErrorKind::WouldBlock
                        || e.kind() == io::ErrorKind::TimedOut =>
                {
                    if last_data_time.elapsed() >= STALL_TIMEOUT {
                        return Err((
                            None,
                            DownloadError::StreamReadError(io::Error::new(
                                io::ErrorKind::TimedOut,
                                "peer did not send download token",
                            )),
                        ));
                    }
                }
                Err(e) => return Err((None, DownloadError::StreamReadError(e))),
            }
        }


        let token = PeerTransferToken(u32::from_le_bytes(
            first_buf[..4]
                .try_into()
                .map_err(|_| (None, DownloadError::InvalidTokenBytes))?,
        ));

        trace!(
            "[download_peer:{}] resolved download token: {}",
            self.username, token
        );

        // Token is now known — all subsequent errors carry Some(token).
        let download =
            resolve_download(token).ok_or((Some(token), DownloadError::TokenNotFound(token)))?;
        let _ = download.sender.send(DownloadStatus::Protocol { phase: "file_reverse_pierce_token_received", bytes: 4, total: download.size });

        stream
            .write_all(&START_DOWNLOAD)
            .map_err(|e| (Some(token), DownloadError::StreamWriteError(e)))?;
        let _ = download.sender.send(DownloadStatus::Protocol { phase: "file_offset_sent", bytes: 8, total: download.size });

        let path = Self::resolve_download_path(&download).map_err(|e| (Some(token), e))?;
        let mut writer = Self::open_output_file(&path).map_err(|e| (Some(token), e))?;

        let total_bytes = 0;

        let _final_bytes = Self::read_stream_with_offset(
            &self.username,
            &mut stream,
            &mut writer,
            &download,
            total_bytes,
        )
        .map_err(|e| (Some(token), e))?;

        writer
            .flush()
            .map_err(|e| (Some(token), DownloadError::FileWriteError(e)))?;

        trace!(
            "[download_peer:{}] download_pierced complete: {} bytes → {}",
            self.username,
            total_bytes + _final_bytes,
            path
        );

        Ok((download, path))
    }

    /// Inner loop: read data from `stream`, write to `writer`, send progress via `download.sender`.
    fn read_stream(
        username: &str,
        stream: &mut TcpStream,
        writer: &mut io::BufWriter<fs::File>,
        download: &Download,
    ) -> Result<usize, DownloadError> {
        Self::read_stream_with_offset(username, stream, writer, download, 0)
    }

    fn read_stream_with_offset(
        _username: &str,
        stream: &mut TcpStream,
        writer: &mut io::BufWriter<fs::File>,
        download: &Download,
        initial_bytes: usize,
    ) -> Result<usize, DownloadError> {
        let mut total_bytes = initial_bytes;
        let mut chunk_counter = 0usize;
        let mut last_update_time = Instant::now();
        let mut last_data_time = Instant::now();
        let mut read_buffer = [0u8; READ_BUFFER_SIZE];

        trace!("[download_peer:{}] reading stream data", _username);

        loop {
            if total_bytes as u64 == download.size {
                break;
            }
            let remaining = (download.size - total_bytes as u64).min(READ_BUFFER_SIZE as u64) as usize;
            match stream.read(&mut read_buffer[..remaining]) {
                Ok(0) => {
                    trace!(
                        "[download_peer:{}] connection closed by peer, {} bytes read",
                        _username, total_bytes
                    );
                    crate::info!("soulseek_socket socket=F phase=data_eof received_bytes={} expected_bytes={}", total_bytes, download.size);
                    let _ = download.sender.send(DownloadStatus::Protocol { phase: "file_data_eof", bytes: total_bytes as u64, total: download.size });
                    return Err(DownloadError::StreamReadError(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "peer closed before the advertised file size",
                    )));
                }
                Ok(bytes_read) => {
                    writer
                        .write_all(&read_buffer[..bytes_read])
                        .map_err(DownloadError::FileWriteError)?;
                    total_bytes += bytes_read;
                    chunk_counter += 1;
                    last_data_time = Instant::now();

                    // Check for manual cancellation after each chunk.
                    if download.cancel.load(Ordering::Relaxed) {
                        trace!("[download_peer:{}] cancelled by caller", _username);
                        return Err(DownloadError::Cancelled);
                    }

                    if chunk_counter.is_multiple_of(PROGRESS_UPDATE_CHUNKS) {
                        writer.flush().map_err(DownloadError::FileWriteError)?;
                        let elapsed = last_update_time.elapsed().as_secs_f64();
                        let speed = if elapsed > 0.0 {
                            (PROGRESS_UPDATE_CHUNKS * READ_BUFFER_SIZE) as f64 / elapsed
                        } else {
                            0.0
                        };
                        let _ = download.sender.send(DownloadStatus::InProgress {
                            bytes_downloaded: total_bytes as u64,
                            total_bytes: download.size,
                            speed_bytes_per_sec: speed,
                        });
                        last_update_time = Instant::now();
                    }

                    if total_bytes >= download.size as usize {
                        break;
                    }
                }
                Err(e)
                    if e.kind() == io::ErrorKind::WouldBlock
                        || e.kind() == io::ErrorKind::TimedOut =>
                {
                    // The 1-second read timeout fired; check flags before retrying.
                    if download.cancel.load(Ordering::Relaxed) {
                        trace!(
                            "[download_peer:{}] cancelled by caller (stalled)",
                            _username
                        );
                        return Err(DownloadError::Cancelled);
                    }
                    if let Some(timeout) = download.progress_timeout
                        && last_update_time.elapsed() >= timeout
                    {
                        trace!(
                            "[download_peer:{}] no progress for {:?}, cancelling",
                            _username, timeout
                        );
                        return Err(DownloadError::NoProgressTimeout);
                    }
                    if last_data_time.elapsed() >= STALL_TIMEOUT {
                        return Err(DownloadError::StreamReadError(io::Error::new(
                            io::ErrorKind::TimedOut,
                            "peer stopped sending data",
                        )));
                    }
                    // Peer is briefly slow; keep waiting.
                }
                Err(e) => {
                    crate::info!("soulseek_socket socket=F phase=data_read_failed kind={:?} received_bytes={} expected_bytes={}", e.kind(), total_bytes, download.size);
                    let _ = download.sender.send(DownloadStatus::Protocol { phase: "file_data_read_failed", bytes: total_bytes as u64, total: download.size });
                    return Err(DownloadError::StreamReadError(e));
                }
            }
        }
        crate::info!("soulseek_socket socket=F phase=data_complete received_bytes={} expected_bytes={}", total_bytes, download.size);
        let _ = download.sender.send(DownloadStatus::Protocol { phase: "file_data_complete", bytes: total_bytes as u64, total: download.size });

        Ok(total_bytes - initial_bytes)
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};

    #[test]
    fn outbound_handshake_precedes_offset_and_truncated_files_fail() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let uploader = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_read_timeout(Some(std::time::Duration::from_secs(2))).unwrap();
            // Independent wire fixture: uint32 frame length, uint8 PeerInit
            // code, length-prefixed username/type, uint32 obsolete token.
            let expected = b"\x11\0\0\0\x01\x03\0\0\0own\x01\0\0\0F\0\0\0\0";
            let mut handshake = [0; 21];
            stream.read_exact(&mut handshake).unwrap();
            assert_eq!(&handshake, expected);
            // FileTransferInit is uploader -> downloader, not TCP initiator
            // -> acceptor. A real uploader waits for the offset after this.
            stream.write_all(&42u32.to_le_bytes()).unwrap();
            let mut offset = [0; 8]; stream.read_exact(&mut offset).unwrap();
            assert_eq!(offset, [0; 8]);
            stream.write_all(b"short").unwrap();
        });
        let directory = std::env::temp_dir().join(format!("soulseek-wire-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let (sender, _) = tokio::sync::mpsc::unbounded_channel();
        let download = crate::types::Download {
            username: "peer".into(), filename: crate::SoulseekPath::from_wire("test.bin".into()),
            token: crate::token::DownloadToken(1), peer_token: Some(crate::token::PeerTransferToken(42)),
            size: 20, download_directory: directory.to_string_lossy().into_owned(),
            status: crate::types::DownloadStatus::QueuedLocally, sender,
            cancel: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            progress_timeout: None, queue_timeout_handle: None,
        };
        let mut exact_download = download.clone();
        exact_download.size = 5;
        let reverse_download = exact_download.clone();
        let error = DownloadPeer::new("peer".into(), "127.0.0.1".into(), port.into(), 42, "own".into()).download_direct(download, None).unwrap_err();
        uploader.join().unwrap();
        assert!(matches!(&error, super::DownloadError::StreamReadError(e) if e.kind() == std::io::ErrorKind::UnexpectedEof));
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let mut uploader = std::net::TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (stream, _) = listener.accept().unwrap();
        let task = std::thread::spawn(move || {
            let mut offset = [0; 8]; uploader.read_exact(&mut offset).unwrap();
            uploader.write_all(b"audio").unwrap();
        });
        let (_, path) = DownloadPeer::new("peer".into(), String::new(), 0, 42, "own".into()).download_direct(exact_download, Some(stream)).unwrap();
        task.join().unwrap();
        assert_eq!(std::fs::read(path).unwrap(), b"audio");
        // Reverse server rendezvous: PierceFireWall frame, uploader token,
        // downloader offset, then literal file bytes (no filesize header).
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let uploader = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_read_timeout(Some(std::time::Duration::from_secs(2))).unwrap();
            let mut init = [0; 9];
            stream.read_exact(&mut init).unwrap();
            assert_eq!(init, [5, 0, 0, 0, 0, 99, 0, 0, 0]);
            stream.write_all(&[42, 0, 0, 0]).unwrap();
            let mut offset = [0; 8];
            stream.read_exact(&mut offset).unwrap();
            assert_eq!(offset, [0; 8]);
            stream.write_all(b"audio").unwrap();
        });
        let (_, path) = DownloadPeer::new("peer".into(), "127.0.0.1".into(), port.into(), 99, "own".into())
            .download_pierced(|token| (token.0 == 42).then(|| reverse_download.clone()), None).unwrap();
        uploader.join().unwrap();
        assert_eq!(std::fs::read(path).unwrap(), b"audio");
        std::fs::remove_dir_all(directory).unwrap();
    }
    use super::DownloadPeer;

    #[test]
    fn test_establish_connection_invalid_address() {
        let download_peer = DownloadPeer::new(
            "user".to_string(),
            "invalid-host".to_string(),
            9999,
            123,
            "own_user".to_string(),
        );
        let result = download_peer.establish_connection();
        assert!(result.is_err());
    }
}
