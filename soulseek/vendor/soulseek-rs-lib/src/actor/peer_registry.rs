use crate::actor::peer_actor::{PeerActor, PeerCommand};
use crate::actor::{ActorHandle, ActorSystem};
use crate::client::ClientOperation;
use crate::debug;
use crate::message::MessageReader;
use crate::path::SoulseekPath;
use crate::peer::Peer;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio::net::TcpStream;
use tokio::sync::mpsc::UnboundedSender;

pub struct PeerRegistry {
    peers: Arc<Mutex<HashMap<String, ActorHandle<PeerCommand>>>>,
    actor_system: Arc<ActorSystem>,
    client_channel: UnboundedSender<ClientOperation>,
    own_username: String,
    shared_folders: u32,
    shared_files: u32,
    connection_slots: Arc<tokio::sync::Semaphore>,
    transfer_slots: Arc<tokio::sync::Semaphore>,
}

impl PeerRegistry {
    pub fn new(
        actor_system: Arc<ActorSystem>,
        client_channel: UnboundedSender<ClientOperation>,
        own_username: String,
        shared_folders: u32,
        shared_files: u32,
    ) -> Self {
        Self {
            peers: Arc::new(Mutex::new(HashMap::new())),
            actor_system,
            client_channel,
            own_username,
            shared_folders,
            shared_files,
            connection_slots: Arc::new(tokio::sync::Semaphore::new(64)),
            transfer_slots: Arc::new(tokio::sync::Semaphore::new(8)),
        }
    }

    pub fn register_inbound_peer(
        &self, peer: Peer, stream: Option<TcpStream>, reader: Option<MessageReader>,
        permit: tokio::sync::OwnedSemaphorePermit,
    ) -> Result<ActorHandle<PeerCommand>, String> {
        self.register_peer_with_permit(peer, stream, reader, Some(permit))
    }
    pub fn register_transfer_peer(&self, peer: Peer, stream: Option<TcpStream>) -> Result<ActorHandle<PeerCommand>, String> {
        let permit = self.transfer_slots.clone().try_acquire_owned()
            .map_err(|_| "Transfer control capacity reached".to_string())?;
        self.register_peer_with_permit(peer, stream, None, Some(permit))
    }

    pub fn register_peer(
        &self,
        peer: Peer,
        stream: Option<TcpStream>,
        reader: Option<MessageReader>,
    ) -> Result<ActorHandle<PeerCommand>, String> {
        self.register_peer_with_permit(peer, stream, reader, None)
    }

    fn register_peer_with_permit(
        &self, peer: Peer, stream: Option<TcpStream>, reader: Option<MessageReader>,
        permit: Option<tokio::sync::OwnedSemaphorePermit>,
    ) -> Result<ActorHandle<PeerCommand>, String> {
        let global_permit = if permit.is_some() { None } else {
            Some(self.connection_slots.clone().try_acquire_owned()
                .map_err(|_| "Peer connection capacity reached".to_string())?)
        };
        let username = peer.username.clone();

        let mut actor = PeerActor::new(
            peer,
            stream,
            reader,
            self.client_channel.clone(),
            self.own_username.clone(),
            self.shared_folders,
            self.shared_files,
        );
        actor.set_inbound_permit(permit);
        if let Some(permit) = global_permit { actor.set_connection_permit(permit); }

        let handle = self.actor_system.spawn(actor);

        let mut peers = self.peers.lock().unwrap();
        peers.insert(username.clone(), handle.clone());

        Ok(handle)
    }

    pub fn get_peer(&self, username: &str) -> Option<ActorHandle<PeerCommand>> {
        let peers = self.peers.lock().unwrap();
        peers.get(username).cloned()
    }

    pub fn remove_peer(&self, username: &str) -> Option<ActorHandle<PeerCommand>> {
        let mut peers = self.peers.lock().unwrap();
        let handle = peers.remove(username);

        if handle.is_some() {
            debug!("[peer_registry] Removed peer actor for {}", username);
        }

        handle
    }

    pub fn get_all_usernames(&self) -> Vec<String> {
        let peers = self.peers.lock().unwrap();
        peers.keys().cloned().collect()
    }

    pub fn count(&self) -> usize {
        let peers = self.peers.lock().unwrap();
        peers.len()
    }

    pub fn contains(&self, username: &str) -> bool {
        let peers = self.peers.lock().unwrap();
        peers.contains_key(username)
    }

    pub fn send_to_peer(&self, username: &str, message: PeerCommand) -> Result<(), String> {
        // Keep lookup/send/removal under one lock so a replacement actor cannot
        // be removed by a failed send to the old one.
        let mut peers = self.peers.lock().unwrap();
        let handle = peers.get(username)
            .ok_or_else(|| format!("Peer {} not found in registry", username))?;
        match handle.send(message) {
            Ok(()) => Ok(()),
            Err(error) => {
                peers.remove(username);
                Err(error)
            }
        }
    }

    pub fn queue_upload(&self, username: &str, filename: SoulseekPath) -> Result<(), String> {
        self.send_to_peer(username, PeerCommand::QueueUpload(filename))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn closed_actor_is_removed_so_address_response_can_reconnect() {
        let (client_channel, _) = tokio::sync::mpsc::unbounded_channel();
        let registry = PeerRegistry::new(Arc::new(ActorSystem::default()), client_channel, "own".into(), 0, 0);
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
        drop(receiver);
        registry.peers.lock().unwrap().insert("peer".into(), ActorHandle { sender });
        assert!(registry.contains("peer"));
        assert!(registry.queue_upload("peer", SoulseekPath::from_wire("file".into())).is_err());
        assert!(!registry.contains("peer"));
        let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
        registry.peers.lock().unwrap().insert("peer".into(), ActorHandle { sender });
        assert!(registry.queue_upload("peer", SoulseekPath::from_wire("file".into())).is_ok());
        assert!(receiver.try_recv().is_ok());
    }
}

impl Clone for PeerRegistry {
    fn clone(&self) -> Self {
        Self {
            peers: self.peers.clone(),
            actor_system: self.actor_system.clone(),
            client_channel: self.client_channel.clone(),
            own_username: self.own_username.clone(),
            shared_folders: self.shared_folders,
            shared_files: self.shared_files,
            connection_slots: self.connection_slots.clone(),
            transfer_slots: self.transfer_slots.clone(),
        }
    }
}
