use std::io;
use std::sync::Arc;
use std::time::Duration;

use tokio::io::AsyncReadExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::oneshot;

use crate::client::{ClientContext, ClientOperation};
use crate::message::{Message, MessageReader};
use crate::peer::download_peer::spawn_direct_download;
use crate::peer::{ConnectionType, Peer};
use crate::token::PeerTransferToken;
use crate::{debug, error, info};

const PEER_INIT_MESSAGE_CODE: u8 = 1;
const PIERCE_FIREWALL_MESSAGE_CODE: u8 = 0;
const PEER_INIT_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_INBOUND_CONNECTIONS: usize = 64;

#[derive(Clone)]
struct ConnectionContext {
    client_sender: UnboundedSender<ClientOperation>,
    client_context: Arc<ClientContext>,
    own_username: String,
    inbound_slots: Arc<tokio::sync::Semaphore>,
}

struct PeerInitData {
    username: String,
    connection_type: ConnectionType,
    token: u32,
}

async fn read_peer_init_message(
    stream: &mut TcpStream,
    reader: &mut MessageReader,
) -> io::Result<Message> {
    // Read exactly one framed init. Reading an arbitrary chunk steals raw file
    // token/data bytes from the socket and makes behavior depend on TCP packets.
    let mut length = [0u8; 4];
    stream.read_exact(&mut length).await?;
    let size = u32::from_le_bytes(length) as usize;
    if !(1..=64 * 1024).contains(&size) {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid peer init size"));
    }
    let mut body = vec![0; size];
    stream.read_exact(&mut body).await?;
    reader.push_bytes(&length);
    reader.push_bytes(&body);
    reader.extract_message().map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid peer init"))?
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "incomplete peer init"))
}

fn parse_pierce_firewall_token(message: &mut Message) -> Option<PeerTransferToken> {
    message.set_pointer(4);
    let message_code = message.read_int8();

    if message_code != PIERCE_FIREWALL_MESSAGE_CODE {
        return None;
    }

    Some(PeerTransferToken(message.read_int32()))
}

fn parse_peer_init_message(mut message: Message) -> Option<PeerInitData> {
    message.set_pointer(4);
    let message_code = message.read_int8();

    if message_code != PEER_INIT_MESSAGE_CODE {
        return None;
    }

    Some(PeerInitData {
        username: message.read_string(),
        connection_type: message.read_string().parse().unwrap(),
        token: message.read_int32(),
    })
}


fn handle_peer_connection(
    peer: Peer,
    stream: TcpStream,
    reader: MessageReader,
    context: &ConnectionContext,
    _peer_ip: &str,
    _peer_port: u16,
) {
    let Ok(permit) = context.inbound_slots.clone().try_acquire_owned() else { return; };
    match context.client_context.peer_registry.register_inbound_peer(
        peer.clone(),
        Some(stream),
        Some(reader),
        permit,
    ) {
        Ok(handle) => {
            let _ = context.client_sender.send(ClientOperation::FolderPeerReady(peer.username, handle));
        },
        Err(_e) => {
            error!(
                "Failed to spawn peer actor for {:?}: {:?}",
                peer.username, _e
            );
        }
    }
}

async fn handle_incoming_connection(stream: TcpStream, context: ConnectionContext) {
    let Ok(peer_addr) = stream.peer_addr() else {
        error!("[listener] failed to get peer address");
        return;
    };

    let peer_ip = peer_addr.ip().to_string();
    let peer_port = peer_addr.port();
    let mut stream = stream;
    let mut reader = MessageReader::new();

    let mut message = match tokio::time::timeout(
        PEER_INIT_TIMEOUT,
        read_peer_init_message(&mut stream, &mut reader),
    )
    .await
    {
        Ok(Ok(msg)) => msg,
        Ok(Err(_e)) => {
            error!("[listener:{peer_ip}:{peer_port}] Failed to read peer init message: {_e}");
            return;
        }
        Err(_) => {
            debug!("[listener:{peer_ip}:{peer_port}] Peer init timed out, dropping connection");
            return;
        }
    };

    // Check for PierceFireWall message (code 0)
    if let Some(token) = parse_pierce_firewall_token(&mut message) {
        debug!(
            "[listener:{peer_ip}:{peer_port}] PierceFireWall token: {}",
            token
        );
        let folder_peer = context.client_context.folder_peers.lock().unwrap().remove(&token.0);
        if let Some(username) = folder_peer {
            let peer = Peer::new(username.clone(), ConnectionType::P, peer_ip, peer_port.into(), None, 0, 0, 0);
            if let Ok(handle) = context.client_context.peer_registry.register_transfer_peer(peer, Some(stream)) {
                let _ = context.client_sender.send(ClientOperation::FolderPeerReady(username, handle));
            }
            return;
        }

        // Query the worker for the download by token
        let (tx, rx) = oneshot::channel();
        let _ = context
            .client_sender
            .send(ClientOperation::QueryDownloadByToken(token, tx));
        let Some(download) = rx.await.ok().flatten() else {
            debug!(
                "[listener:{peer_ip}:{peer_port}] No download found for PierceFireWall token: {}",
                token
            );
            return;
        };

        let std_stream = stream.into_std().unwrap();
        spawn_direct_download(
            download,
            peer_ip,
            peer_port.into(),
            token.0,
            context.own_username.clone(),
            Some(std_stream),
            context.client_sender.clone(),
        );
        return;
    }

    let Some(init_data) = parse_peer_init_message(message) else {
        error!("[listener:{peer_ip}:{peer_port}] Invalid or unknown peer init message");
        return;
    };

    debug!(
        "[listener:{peer_ip}:{peer_port}] peerInit username: {} connection_type: {} token: {}",
        init_data.username, init_data.connection_type, init_data.token
    );

    let peer = Peer::new(
        init_data.username.clone(),
        init_data.connection_type.clone(),
        peer_ip.clone(),
        peer_port.into(),
        None,
        0,
        0,
        0,
    );

    match init_data.connection_type {
        ConnectionType::P => {
            handle_peer_connection(peer, stream, reader, &context, &peer_ip, peer_port)
        }

        ConnectionType::F => {
            // The transfer token is raw data following the framed init and may
            // arrive in a separate packet. Do not require it to be coalesced.
            let mut token_bytes = [0u8; 4];
            match tokio::time::timeout(PEER_INIT_TIMEOUT, stream.read_exact(&mut token_bytes)).await {
                Ok(Ok(_)) => {},
                _ => return,
            }
            let download_token = PeerTransferToken(u32::from_le_bytes(token_bytes));

            // Query the worker for the download
            let (tx, rx) = oneshot::channel();
            let _ = context
                .client_sender
                .send(ClientOperation::QueryDownloadByToken(download_token, tx));
            let Some(download) = rx.await.ok().flatten() else {
                error!(
                    "[listener:{}:{}] No download found for file connection token: {}",
                    peer_ip, peer_port, download_token
                );
                return;
            };

            let std_stream = stream.into_std().unwrap();
            spawn_direct_download(
                download,
                peer.host,
                peer.port,
                init_data.token,
                context.own_username.clone(),
                Some(std_stream),
                context.client_sender.clone(),
            );
        }
        ConnectionType::D => {
            debug!(
                "[listener:{peer_ip}:{peer_port}] connection type is D, not supported yet, closing connection. "
            );
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum ListenError {
    #[error("failed to bind listener to port {0}")]
    FailedToBindListener(#[from] io::Error),
}

pub struct Listen;

impl Listen {
    pub async fn start(
        port: u32,
        client_sender: UnboundedSender<ClientOperation>,
        client_context: Arc<ClientContext>,
        own_username: String,
    ) -> Result<(), ListenError> {
        info!("[listener] starting listener on port {port}");

        let listener = TcpListener::bind(format!("0.0.0.0:{port}"))
            .await
            .map_err(ListenError::FailedToBindListener)?;

        let context = ConnectionContext {
            client_sender,
            client_context,
            own_username,
            inbound_slots: Arc::new(tokio::sync::Semaphore::new(MAX_INBOUND_CONNECTIONS)),
        };

        let handshakes = Arc::new(tokio::sync::Semaphore::new(16));
        loop {
            match listener.accept().await {
                Ok((stream, _addr)) => {
                    let Ok(handshake) = handshakes.clone().try_acquire_owned() else { continue; };
                    let context = context.clone();
                    tokio::spawn(async move {
                        handle_incoming_connection(stream, context).await;
                        drop(handshake);
                    });
                }
                Err(_e) => {
                    error!("[listener] Failed to accept connection: {}", _e);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;
    #[tokio::test]
    async fn init_reader_leaves_file_token_on_socket() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mut sender = TcpStream::connect(listener.local_addr().unwrap()).await.unwrap();
        let (mut receiver, _) = listener.accept().await.unwrap();
        let mut wire = crate::message::server::MessageFactory::build_peer_init_message("peer", ConnectionType::F, 0).get_buffer();
        wire.extend_from_slice(&42u32.to_le_bytes());
        wire.extend_from_slice(b"file");
        sender.write_all(&wire).await.unwrap();
        let mut reader = MessageReader::new();
        let message = read_peer_init_message(&mut receiver, &mut reader).await.unwrap();
        assert_eq!(parse_peer_init_message(message).unwrap().username, "peer");
        let mut raw = [0; 8]; receiver.read_exact(&mut raw).await.unwrap();
        assert_eq!(&raw[..4], &42u32.to_le_bytes());
        assert_eq!(&raw[4..], b"file");
    }

    #[tokio::test]
    async fn file_token_can_follow_init_in_separate_writes() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let mut sender = TcpStream::connect(listener.local_addr().unwrap()).await.unwrap();
        let (mut receiver, _) = listener.accept().await.unwrap();
        let init = crate::message::server::MessageFactory::build_peer_init_message("peer", ConnectionType::F, 0).get_buffer();
        sender.write_all(&init).await.unwrap();
        let mut reader = MessageReader::new();
        read_peer_init_message(&mut receiver, &mut reader).await.unwrap();
        let task = tokio::spawn(async move {
            let mut token = [0; 4]; receiver.read_exact(&mut token).await.unwrap(); token
        });
        sender.write_all(&[42, 0]).await.unwrap();
        tokio::task::yield_now().await;
        sender.write_all(&[0, 0]).await.unwrap();
        assert_eq!(task.await.unwrap(), 42u32.to_le_bytes());
    }
}
