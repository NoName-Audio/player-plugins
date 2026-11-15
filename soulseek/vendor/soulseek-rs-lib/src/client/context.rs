use crate::peer::PeerRegistry;

pub struct ClientContext {
    pub peer_registry: PeerRegistry,
    pub folder_peers: std::sync::Mutex<std::collections::HashMap<u32, String>>,
}

impl ClientContext {
    pub fn new(peer_registry: PeerRegistry) -> Self {
        Self { peer_registry, folder_peers: std::sync::Mutex::new(std::collections::HashMap::new()) }
    }
}
