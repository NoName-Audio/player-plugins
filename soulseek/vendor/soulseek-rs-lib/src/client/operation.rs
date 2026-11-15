use tokio::sync::oneshot;

use crate::path::SoulseekPath;
use crate::token::{DownloadToken, PeerTransferToken, SearchToken};
use crate::{
    Transfer,
    error::SoulseekRs,
    peer::{NewPeer, Peer},
    types::{Download, SearchResult},
};

pub enum ClientOperation {
    FolderContents { username: String, directory: String, token: u32, phase: std::sync::Arc<std::sync::atomic::AtomicU8>, reply: oneshot::Sender<crate::error::Result<Vec<crate::File>>> },
    FolderPeerReady(String, crate::actor::ActorHandle<crate::actor::peer_actor::PeerCommand>),
    CancelFolder(u32),
    RetryFolder(String, crate::actor::peer_actor::PeerCommand),
    // ── Server lifecycle ─────────────────────────────────────────────────────
    /// Server TCP connection was lost; reconnect will be handled by ServerActor.
    ServerDisconnected,
    /// (Re)login confirmed; replay pending downloads.
    LoginSucceeded,

    // ── Peer lifecycle ────────────────────────────────────────────────────────
    NewPeer(NewPeer),
    ConnectToPeer(Peer),
    PierceFireWall(Peer),
    PeerDisconnected(String, Option<SoulseekRs>),
    GetPeerAddressResponse {
        username: String,
        host: String,
        port: u32,
        obfuscation_type: u32,
        obfuscated_port: u16,
    },

    // ── Download events ───────────────────────────────────────────────────────
    /// Initiate or queue a download; routed by ConnectedWorker.
    RequestDownload(Download),
    DownloadFromPeer(PeerTransferToken, Peer, bool),
    UpdateDownloadTokens(Transfer, String),
    /// Peer told us our position in their upload queue; transitions download to QueuedRemotely.
    QueuePositionUpdated {
        username: String,
        filename: SoulseekPath,
        place: u32,
    },
    /// Peer sent TransferResponse(allowed=false); download stays queued, timeout resets.
    TransferRejected {
        token: PeerTransferToken,
        reason: Option<String>,
    },
    UploadFailed(String, SoulseekPath),
    /// A download finished (success or failure); carries token and path-or-error.
    DownloadCompleted(DownloadToken, Result<String, SoulseekRs>),
    /// Cancel a download by token; cleans up pending queue and active slots.
    CancelDownload(DownloadToken),
    /// Deadline for peer to respond to QueueUpload; ignored if already progressing.
    DownloadResponseTimeout(DownloadToken),

    // ── Search events ─────────────────────────────────────────────────────────
    /// Register a search entry in the worker (sent before FileSearch).
    InitiateSearch(SearchToken, String),
    SearchResult(SearchResult),

    // ── Queries (oneshot request/response) ───────────────────────────────────
    /// Listener queries worker for a download by peer token.
    QueryDownloadByToken(PeerTransferToken, oneshot::Sender<Option<Download>>),
    /// Public API: query all downloads.
    QueryDownloads(oneshot::Sender<Vec<Download>>),
    /// Public API: query search results for a key.
    QuerySearchResults(String, oneshot::Sender<Vec<SearchResult>>),
}
