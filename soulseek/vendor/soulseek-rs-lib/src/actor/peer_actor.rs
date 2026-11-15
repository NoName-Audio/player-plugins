use crate::actor::{Actor, ConnectionState};
use crate::client::ClientOperation;
use crate::dispatcher::MessageDispatcher;
use crate::message::peer::{
    FileSearchResponse, GetShareFileList, PeerInit, PlaceInQueueResponse, TransferRequest,
    TransferResponse, UploadFailedHandler,
};
use crate::message::server::MessageFactory;
#[allow(unused_imports)]
use crate::message::{Handlers, Message, MessageReader, MessageType};
use crate::path::SoulseekPath;
use crate::peer::Peer;
use crate::token::{PeerTransferToken, PierceToken};
use crate::types::{Download, SearchResult, Transfer};
use crate::{debug, error, trace, warn};

use std::io;
use std::time::{Duration, Instant};

use tokio::net::TcpStream;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

#[derive(Debug)]
pub enum PeerCommand {
    QueueUpload(SoulseekPath),
    RequestTransfer(Download),
    FolderContents { token: u32, directory: String, phase: std::sync::Arc<std::sync::atomic::AtomicU8>, reply: tokio::sync::oneshot::Sender<crate::error::Result<Vec<crate::File>>> },
}

pub(crate) enum PeerSignal {
    ProcessRead,
    ConnectionEstablished(TcpStream),
    ConnectionFailed(io::Error),
    SendMessage(Message),
    FileSearchResult(SearchResult),
    TransferRequest(Transfer),
    TransferResponse {
        token: PeerTransferToken,
        allowed: bool,
        reason: Option<String>,
    },
    PlaceInQueueResponse {
        filename: SoulseekPath,
        place: u32,
    },
    SetUsername(String),
}

pub struct PeerActor {
    peer: Peer,
    stream: Option<TcpStream>,
    connection_state: ConnectionState,
    reader: MessageReader,
    client_channel: UnboundedSender<ClientOperation>,
    signal_tx: UnboundedSender<PeerSignal>,
    signal_rx: UnboundedReceiver<PeerSignal>,
    dispatcher: Option<MessageDispatcher<PeerSignal>>,
    queued_commands: Vec<PeerCommand>,
    #[allow(dead_code)]
    own_username: String,
    needs_handshake: bool,
    shared_folders: u32,
    shared_files: u32,
    transfer_control: bool,
    last_activity: Instant,
    inbound_permit: Option<tokio::sync::OwnedSemaphorePermit>,
    connection_permit: Option<tokio::sync::OwnedSemaphorePermit>,
    folder_requests: Vec<(u32, String, tokio::sync::oneshot::Sender<crate::error::Result<Vec<crate::File>>>, std::sync::Arc<std::sync::atomic::AtomicU8>)>,
}

impl PeerActor {
    pub fn new(
        peer: Peer,
        stream: Option<TcpStream>,
        reader: Option<MessageReader>,
        client_channel: UnboundedSender<ClientOperation>,
        own_username: String,
        shared_folders: u32,
        shared_files: u32,
    ) -> Self {
        let needs_handshake = stream.is_none();
        let connection_state = if stream.is_some() {
            ConnectionState::Connected
        } else {
            ConnectionState::Disconnected
        };
        let (signal_tx, signal_rx) = mpsc::unbounded_channel::<PeerSignal>();

        Self {
            peer,
            stream,
            connection_state,
            reader: reader.unwrap_or_default(),
            client_channel,
            signal_tx,
            signal_rx,
            dispatcher: None,
            queued_commands: Vec::new(),
            own_username,
            needs_handshake,
            shared_folders,
            shared_files,
            transfer_control: false,
            last_activity: Instant::now(),
            inbound_permit: None,
            connection_permit: None,
            folder_requests: Vec::new(),
        }
    }

    pub(crate) fn set_inbound_permit(&mut self, permit: Option<tokio::sync::OwnedSemaphorePermit>) {
        self.inbound_permit = permit;
    }

    pub(crate) fn set_connection_permit(&mut self, permit: tokio::sync::OwnedSemaphorePermit) {
        self.connection_permit = Some(permit);
    }
    fn initialize_dispatcher(&mut self) {
        let mut handlers = Handlers::new();
        handlers.register_handler(FileSearchResponse);
        handlers.register_handler(TransferRequest);
        handlers.register_handler(TransferResponse);
        handlers.register_handler(GetShareFileList {
            shared_folders: self.shared_folders,
            shared_files: self.shared_files,
        });
        handlers.register_handler(UploadFailedHandler);
        handlers.register_handler(PlaceInQueueResponse);
        handlers.register_handler(PeerInit);

        self.dispatcher = Some(MessageDispatcher::new(self.signal_tx.clone(), handlers));
    }

    fn drain_signals(&mut self) {
        let signals: Vec<PeerSignal> = {
            let mut sigs = Vec::new();
            while let Ok(sig) = self.signal_rx.try_recv() {
                sigs.push(sig);
            }
            sigs
        };
        for sig in signals {
            self.handle_signal(sig);
        }
    }

    fn handle_command(&mut self, cmd: PeerCommand) {
        if !matches!(&cmd, PeerCommand::FolderContents { .. }) { self.transfer_control = true; }
        self.last_activity = Instant::now();
        match cmd {
            PeerCommand::FolderContents { token, directory, phase, reply } => {
                phase.fetch_max(2, std::sync::atomic::Ordering::Relaxed);
                let wire = crate::message::peer::folder_contents::request(token, &directory);
                self.folder_requests.push((token, directory, reply, phase.clone()));
                self.send_message(Message::new_with_data(wire[4..].to_vec()));
                phase.fetch_max(3, std::sync::atomic::Ordering::Relaxed);
            }
            PeerCommand::QueueUpload(filename) => {
                let message = MessageFactory::build_queue_upload_message(filename.as_str());
                self.send_message(message);
            }
            PeerCommand::RequestTransfer(download) => {
                let message = MessageFactory::build_transfer_request_message(
                    download.filename.as_str(),
                    download.token.0,
                );
                self.send_message(message);
            }
        }
    }

    fn handle_signal(&mut self, sig: PeerSignal) {
        match sig {
            PeerSignal::SendMessage(message) => {
                self.send_message(message);
            }
            PeerSignal::FileSearchResult(file_search) => {
                self.client_channel
                    .send(ClientOperation::SearchResult(file_search))
                    .unwrap();
                // Search responses use short-lived P connections. Retaining
                // every responder exhausted hundreds of sockets per query.
                if !self.transfer_control {
                    self.disconnect(None);
                }
            }
            PeerSignal::TransferRequest(transfer) => {
                self.transfer_control = true;
                let username = self.peer.username.clone();
                debug!("[peer:{}] TransferRequest for {}", username, transfer.token);

                self.client_channel
                    .send(ClientOperation::UpdateDownloadTokens(
                        transfer.clone(),
                        username.clone(),
                    ))
                    .unwrap();

                let transfer_response = MessageFactory::build_transfer_response_message(transfer);
                self.send_message(transfer_response);
            }
            PeerSignal::TransferResponse {
                token,
                allowed,
                reason,
            } => {
                let _username = self.peer.username.clone();
                debug!(
                    "[peer:{}] transfer response token: {} allowed: {}",
                    _username, token, allowed
                );

                if !allowed {
                    debug!(
                        "[peer:{}] Transfer rejected: {:?} - token {}, resetting timeout...",
                        _username, reason, token
                    );
                    if let Err(_e) = self
                        .client_channel
                        .send(ClientOperation::TransferRejected { token, reason })
                    {
                        error!(
                            "[peer:{}] Failed to send TransferRejected: {}",
                            _username, _e
                        );
                    }
                } else {
                    debug!(
                        "[peer:{}] Transfer allowed, ready to connect with token {:}",
                        _username, token
                    );
                    if let Err(_e) = self.client_channel.send(ClientOperation::DownloadFromPeer(
                        token,
                        self.peer.clone(),
                        allowed,
                    )) {
                        error!(
                            "[peer:{}] Failed to send DownloadFromPeer: {}",
                            _username, _e
                        );
                    }
                }
            }
            PeerSignal::PlaceInQueueResponse { filename, place } => {
                let _username = self.peer.username.clone();
                debug!(
                    "[peer:{}] Place in queue response - file: {}, place: {}",
                    _username, filename, place
                );
                if let Err(_e) = self
                    .client_channel
                    .send(ClientOperation::QueuePositionUpdated {
                        username: _username.clone(),
                        filename,
                        place,
                    })
                {
                    error!(
                        "[peer:{}] Failed to send QueuePositionUpdated: {}",
                        _username, _e
                    );
                }
            }
            PeerSignal::SetUsername(username) => {
                trace!("[peer:{}] SetUsername: {}", self.peer.username, username);
                self.peer.username = username;
            }
            PeerSignal::ProcessRead => {
                self.process_read();
            }
            PeerSignal::ConnectionEstablished(stream) => {
                self.stream = Some(stream);
                self.connection_state = ConnectionState::Connected;
                self.on_connection_established();
            }
            PeerSignal::ConnectionFailed(e) => {
                self.disconnect(Some(e));
            }
        }
    }

    fn process_read(&mut self) {
        if self.reader.buffer_len() > 0 {
            self.extract_and_process_messages();
        }

        // Drain available input promptly, but yield after 1 MiB per tick so a
        // single responder cannot monopolize the runtime.
        let mut read_budget = 1024 * 1024;
        while read_budget > 0 {
            let stream = match self.stream.as_ref() {
                Some(s) => s,
                None => return,
            };

            let mut temp_buffer = [0u8; 32 * 1024];
            match stream.try_read(&mut temp_buffer) {
                Ok(0) => {
                    self.extract_and_process_messages();
                    // EOF — remote closed the connection cleanly.
                    trace!(
                        "[peer:{}] EOF, remote closed connection",
                        self.peer.username
                    );
                    self.disconnect(None);
                    return;
                }
                Ok(n) => {
                    self.reader.push_bytes(&temp_buffer[..n]);
                    self.last_activity = Instant::now();
                    read_budget -= n;
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(ref e) if e.kind() == io::ErrorKind::TimedOut => {
                    debug!(
                        "Read operation timed out for peer actor {:}:{:}",
                        self.peer.host, self.peer.port
                    );
                    break;
                }
                Err(e) => {
                    let _username = self.peer.username.clone();
                    error!(
                        "[peer:{}] Error reading from peer: {} (kind: {:?}). Disconnecting.",
                        _username,
                        e,
                        e.kind()
                    );
                    self.disconnect(Some(e));
                    return;
                }
            }
        }
        self.extract_and_process_messages();
    }

    fn extract_and_process_messages(&mut self) {
        let _username = self.peer.username.clone();
        let mut _extracted_count = 0;
        loop {
            match self.reader.extract_message() {
                Ok(Some(mut message)) => {
                    _extracted_count += 1;
                    if message.get_message_code() == 37 {
                        let size = message.get_size();
                        let data = message.get_slice(8, size);
                        self.folder_requests.retain(|(_, _, reply, _)| !reply.is_closed());
                        let mut index = 0;
                        while index < self.folder_requests.len() {
                            let (token, directory, _, _) = &self.folder_requests[index];
                            match crate::message::peer::folder_contents::response(&data, &self.peer.username, *token, directory) {
                                Ok(None) => index += 1,
                                result => { let (_, _, reply, _) = self.folder_requests.swap_remove(index); let _ = reply.send(result.map(|v| v.unwrap_or_default())); }
                            }
                        }
                        if self.folder_requests.is_empty() && !self.transfer_control { self.disconnect(None); }
                        continue;
                    }
                    trace!(
                        "[peer:{}] ← Message #{}: {:?}",
                        _username,
                        _extracted_count,
                        message
                            .get_message_name(MessageType::Peer, message.get_message_code() as u32)
                            .map_err(|e| e.to_string())
                    );
                    if let Some(ref dispatcher) = self.dispatcher {
                        dispatcher.dispatch(&mut message);
                    } else {
                        warn!("[peer:{}] No dispatcher available!", _username);
                    }
                }
                Err(e) => {
                    warn!(
                        "[peer:{}] Error extracting message: {}. Disconnecting peer.",
                        _username, e
                    );
                    self.disconnect(Some(e));
                    return;
                }
                Ok(None) => {
                    break;
                }
            }
        }

        self.drain_signals();
    }

    fn send_message(&mut self, message: Message) {
        let stream = match self.stream.as_ref() {
            Some(s) => s,
            None => {
                error!("Cannot send message: stream is None");
                return;
            }
        };

        let _username = self.peer.username.clone();
        trace!(
            "[peer:{}] ➡ {:?}",
            _username,
            message
                .get_message_name(
                    MessageType::Peer,
                    u32::from_le_bytes(message.get_slice(0, 4).try_into().unwrap())
                )
                .map_err(|e| e.to_string())
        );

        let buf = message.get_buffer();
        match stream.try_write(&buf) {
            Ok(_n) if _n == buf.len() => {}
            Ok(_n) => {
                error!(
                    "[peer:{}] Partial write: {} of {} bytes",
                    _username,
                    _n,
                    buf.len()
                );
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                warn!(
                    "[peer:{}] Write would block, message may be lost",
                    _username
                );
            }
            Err(e) => {
                error!(
                    "[peer:{}] Error writing message: {}. Disconnecting.",
                    _username, e
                );
                self.disconnect(Some(e));
            }
        }
    }

    fn disconnect(&mut self, error: Option<io::Error>) {
        debug!("[peer:{}] disconnect", self.peer.username);
        self.stream.take();
        self.inbound_permit.take();
        self.connection_permit.take();
        self.connection_state = ConnectionState::Disconnected;
        for (token, directory, reply, phase) in std::mem::take(&mut self.folder_requests) {
            if !reply.is_closed() {
                let command = PeerCommand::FolderContents { token, directory, phase, reply };
                let _ = self.client_channel.send(ClientOperation::RetryFolder(self.peer.username.clone(), command));
            }
        }
        let queued = std::mem::take(&mut self.queued_commands);
        for command in queued {
            if matches!(&command, PeerCommand::FolderContents { .. }) {
                let _ = self.client_channel.send(ClientOperation::RetryFolder(self.peer.username.clone(), command));
            }
        }
        if let Err(_e) = self.client_channel.send(ClientOperation::PeerDisconnected(
            self.peer.username.clone(),
            error.map(Into::into),
        )) {
            error!("Failed to send disconnect notification: {}", _e);
        }
    }

    fn initiate_connection(&mut self) -> bool {
        let _username = self.peer.username.clone();
        let host = self.peer.host.clone();
        let port = self.peer.port;

        let socket_addr = format!("{}:{}", host, port).parse::<std::net::SocketAddr>();

        match socket_addr {
            Ok(addr) => {
                self.connection_state = ConnectionState::Connecting {
                    since: Instant::now(),
                };

                let signal_tx = self.signal_tx.clone();
                tokio::spawn(async move {
                    let timeout = Duration::from_secs(5);
                    let result = tokio::time::timeout(timeout, TcpStream::connect(addr)).await;

                    match result {
                        Ok(Ok(stream)) => {
                            stream.set_nodelay(true).ok();
                            let _ = signal_tx.send(PeerSignal::ConnectionEstablished(stream));
                        }
                        Ok(Err(e)) => {
                            let _ = signal_tx.send(PeerSignal::ConnectionFailed(e));
                        }
                        Err(_) => {
                            let _ = signal_tx.send(PeerSignal::ConnectionFailed(io::Error::new(
                                io::ErrorKind::TimedOut,
                                "connection timed out",
                            )));
                        }
                    }
                });
                true
            }
            Err(e) => {
                error!(
                    "[peer:{}] Invalid socket address {}:{} - {}",
                    _username, host, port, e
                );
                self.disconnect(Some(io::Error::new(io::ErrorKind::InvalidInput, e)));
                false
            }
        }
    }

    fn check_connection_status(&mut self) {
        let ConnectionState::Connecting { since } = self.connection_state else {
            return;
        };

        if since.elapsed() > Duration::from_secs(10) {
            error!(
                "[peer:{}] Connection timeout after 10 seconds",
                self.peer.username
            );
            self.disconnect(Some(io::Error::new(
                io::ErrorKind::TimedOut,
                "Connection timeout",
            )));
        }
    }

    fn on_connection_established(&mut self) {
        let _username = self.peer.username.clone();
        let token = self.peer.token.unwrap_or(PierceToken(0));

        let Some(ref stream) = self.stream else {
            return;
        };

        if self.needs_handshake {
            let handshake_msg = match self.peer.token {
                Some(token) => MessageFactory::build_pierce_firewall_message(token.0),
                None => MessageFactory::build_peer_init_message(&self.own_username, crate::peer::ConnectionType::P, 0),
            };
            match stream.try_write(&handshake_msg.get_buffer()) {
                Ok(_) => {
                    debug!(
                        "[peer:{}] Sent PierceFireWall handshake (token={})",
                        _username, token
                    );
                }
                Err(e) => {
                    error!(
                        "[peer:{}] Failed to send PierceFireWall handshake: {}",
                        _username, e
                    );
                    self.disconnect(Some(e));
                    return;
                }
            }
        }

        self.initialize_dispatcher();

        let queued = std::mem::take(&mut self.queued_commands);
        for cmd in queued {
            self.handle_command(cmd);
        }

        self.signal_tx.send(PeerSignal::ProcessRead).ok();
        self.process_read();
    }
}

impl Actor for PeerActor {
    type Message = PeerCommand;

    fn handle(&mut self, msg: Self::Message) {
        if !matches!(self.connection_state, ConnectionState::Connected) || self.stream.is_none() {
            self.queued_commands.push(msg);
            return;
        }
        self.handle_command(msg);
    }

    fn on_start(&mut self) {
        if self.stream.is_none() {
            self.initiate_connection();
        } else {
            self.connection_state = ConnectionState::Connected;
            self.on_connection_established();
        }
    }

    fn on_stop(&mut self) {
        trace!("[peer:{}] actor stopping", self.peer.username);
        self.disconnect(None);
    }

    fn tick(&mut self) {
        let had_folder_requests = !self.folder_requests.is_empty();
        self.folder_requests.retain(|(_, _, reply, _)| !reply.is_closed());
        if had_folder_requests && self.folder_requests.is_empty() && !self.transfer_control {
            self.disconnect(None);
        }
        // ConnectionEstablished owns a socket while queued. Always drain it
        // before expiry, otherwise an expired connecting actor retains it forever.
        self.drain_signals();
        if !self.transfer_control && self.folder_requests.is_empty() && self.last_activity.elapsed() >= Duration::from_secs(15) {
            if self.stream.is_some() { self.disconnect(None); }
            return;
        }
        match self.connection_state {
            ConnectionState::Connecting { .. } => {
                self.check_connection_status();
            }
            ConnectionState::Connected => {
                if self.stream.is_some() {
                    self.process_read();
                }
            }
            ConnectionState::Disconnected => {}
        }
    }
}

#[cfg(test)]
mod lifecycle_tests {
    #[tokio::test]
    async fn inbound_capacity_releases_on_disconnect_before_actor_drop() {
        let slots = std::sync::Arc::new(tokio::sync::Semaphore::new(64));
        let mut permits: Vec<_> = (0..64).map(|_| slots.clone().try_acquire_owned().unwrap()).collect();
        assert!(slots.clone().try_acquire_owned().is_err());
        let (sender, _receiver) = mpsc::unbounded_channel();
        let peer = Peer::new("peer".into(), crate::peer::ConnectionType::P, "127.0.0.1".into(), 0, None, 0, 0, 0);
        let mut actor = PeerActor::new(peer, None, None, sender, "own".into(), 0, 0);
        actor.set_inbound_permit(permits.pop());
        actor.disconnect(None);
        assert_eq!(slots.available_permits(), 1);
        actor.set_inbound_permit(permits.pop());
        drop(actor);
        assert_eq!(slots.available_permits(), 2);
    }

    #[tokio::test]
    async fn direct_and_reverse_control_handshakes_use_literal_wire_frames() {
        for token in [None, Some(PierceToken(99))] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let mut remote = TcpStream::connect(listener.local_addr().unwrap()).await.unwrap();
            let (stream, _) = listener.accept().await.unwrap();
            stream.writable().await.unwrap();
            let (sender, _receiver) = mpsc::unbounded_channel();
            let peer = Peer::new("peer".into(), crate::peer::ConnectionType::P, "127.0.0.1".into(), 0, token, 0, 0, 0);
            let mut actor = PeerActor::new(peer, Some(stream), None, sender, "own".into(), 0, 0);
            actor.needs_handshake = true;
            actor.on_connection_established();
            let expected: &[u8] = if token.is_some() { &[5,0,0,0,0,99,0,0,0] }
                else { b"\x11\0\0\0\x01\x03\0\0\0own\x01\0\0\0P\0\0\0\0" };
            let mut actual = vec![0; expected.len()];
            remote.read_exact(&mut actual).await.unwrap();
            assert_eq!(actual, expected);
        }
    }

    #[tokio::test]
    async fn late_connection_signal_does_not_retain_expired_socket() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mut remote = TcpStream::connect(listener.local_addr().unwrap()).await.unwrap();
        let (stream, _) = listener.accept().await.unwrap();
        let (sender, _receiver) = mpsc::unbounded_channel();
        let peer = Peer::new("peer".into(), crate::peer::ConnectionType::P, "127.0.0.1".into(), 0, None, 0, 0, 0);
        let mut actor = PeerActor::new(peer, None, None, sender, "own".into(), 0, 0);
        actor.needs_handshake = false;
        actor.last_activity = Instant::now() - Duration::from_secs(30);
        actor.signal_tx.send(PeerSignal::ConnectionEstablished(stream)).unwrap();
        actor.tick();
        assert!(actor.stream.is_none());
        let mut byte = [0];
        assert_eq!(tokio::time::timeout(Duration::from_millis(100), remote.read(&mut byte)).await.unwrap().unwrap(), 0);
    }

    use super::*;
    use tokio::io::AsyncReadExt;
    #[tokio::test]
    async fn drains_large_incomplete_response_in_one_poll() {
        use tokio::io::AsyncWriteExt;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mut remote = TcpStream::connect(listener.local_addr().unwrap()).await.unwrap();
        let (stream, _) = listener.accept().await.unwrap();
        let (sender, _receiver) = mpsc::unbounded_channel();
        let peer = Peer::new("peer".into(), crate::peer::ConnectionType::P, "127.0.0.1".into(), 0, None, 0, 0, 0);
        let mut actor = PeerActor::new(peer, Some(stream), None, sender, "own".into(), 0, 0);
        let mut response = vec![0u8; 64 * 1024];
        response[..4].copy_from_slice(&(128u32 * 1024).to_le_bytes());
        response[4..8].copy_from_slice(&9u32.to_le_bytes());
        remote.write_all(&response).await.unwrap();
        actor.stream.as_ref().unwrap().readable().await.unwrap();
        actor.process_read();
        assert_eq!(actor.reader.buffer_len(), response.len());
    }


    #[tokio::test]
    async fn search_response_closes_socket_but_transfer_control_survives() {
        for transfer_control in [false, true] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let mut remote = TcpStream::connect(listener.local_addr().unwrap()).await.unwrap();
            let (stream, _) = listener.accept().await.unwrap();
            let (sender, mut receiver) = mpsc::unbounded_channel();
            let peer = Peer::new("peer".into(), crate::peer::ConnectionType::P, "127.0.0.1".into(), 0, None, 0, 0, 0);
            let mut actor = PeerActor::new(peer, Some(stream), None, sender, "own".into(), 0, 0);
            actor.transfer_control = transfer_control;
            actor.handle_signal(PeerSignal::FileSearchResult(SearchResult {
                token: crate::token::SearchToken(1), files: vec![], slots: 0, speed: 0, username: "peer".into(),
            }));
            assert!(matches!(receiver.recv().await, Some(ClientOperation::SearchResult(_))));
            actor.last_activity = Instant::now() - Duration::from_secs(30);
            actor.tick();
            let mut byte = [0];
            let read = tokio::time::timeout(Duration::from_millis(30), remote.read(&mut byte)).await;
            if transfer_control {
                assert!(read.is_err());
                assert!(actor.stream.is_some());
            } else {
                assert_eq!(read.unwrap().unwrap(), 0);
                assert!(actor.stream.is_none());
            }
        }
    }
}
