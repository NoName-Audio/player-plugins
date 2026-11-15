use std::{error::Error, fmt};

/// Custom error type for the Soulseek download library
#[derive(Debug)]
pub enum SoulseekRs {
    NetworkError(std::io::Error),
    /// Authentication failed during login
    AuthenticationFailed,
    /// Error parsing messages or data
    ParseError(String),
    /// Operation timed out
    Timeout,
    /// Connection was closed unexpectedly
    ConnectionClosed,
    /// Invalid message format or content
    InvalidMessage(String),
    /// Server not connected
    NotConnected,
    /// Compression/decompression error
    CompressionError(String),
    /// Download was cancelled by the caller
    DownloadCancelled,
    /// Download timed out due to no progress
    DownloadTimedOut,
    /// Classified download failure; the original cause is retained but never logged as diagnostics.
    DownloadFailure {
        category: &'static str,
        source: Box<dyn Error + Send + Sync>,
    },
}

impl fmt::Display for SoulseekRs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SoulseekRs::NetworkError(err) => {
                write!(f, "Network error: {}", err)
            }
            SoulseekRs::AuthenticationFailed => {
                write!(f, "Authentication failed")
            }
            SoulseekRs::ParseError(msg) => write!(f, "Parse error: {}", msg),
            SoulseekRs::Timeout => write!(f, "Operation timed out"),
            SoulseekRs::ConnectionClosed => write!(f, "Connection closed"),
            SoulseekRs::InvalidMessage(msg) => {
                write!(f, "Invalid message: {}", msg)
            }
            SoulseekRs::NotConnected => write!(f, "Not connected to server"),
            SoulseekRs::CompressionError(msg) => {
                write!(f, "Compression error: {}", msg)
            }
            SoulseekRs::DownloadCancelled => write!(f, "Download cancelled"),
            SoulseekRs::DownloadTimedOut => write!(f, "Download timed out (no progress)"),
            SoulseekRs::DownloadFailure { category, .. } => write!(f, "Download failed ({category})"),
        }
    }
}

impl Error for SoulseekRs {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            SoulseekRs::NetworkError(err) => Some(err),
            SoulseekRs::DownloadFailure { source, .. } => Some(source.as_ref()),
            _ => None,
        }
    }
}

impl From<std::io::Error> for SoulseekRs {
    fn from(err: std::io::Error) -> Self {
        SoulseekRs::NetworkError(err)
    }
}

impl SoulseekRs {
    /// Privacy-safe classification: never derive diagnostic fields from error text.
    pub fn failure_category(&self) -> &'static str {
        match self {
            Self::NetworkError(error) => network_failure_category(error.kind()),
            Self::DownloadFailure { category, .. } => category,
            Self::AuthenticationFailed => "authentication_failed",
            Self::ParseError(_) => "parse_error",
            Self::Timeout => "timeout",
            Self::ConnectionClosed => "peer_disconnected",
            Self::InvalidMessage(_) => "invalid_message",
            Self::NotConnected => "not_connected",
            Self::CompressionError(_) => "compression_error",
            Self::DownloadCancelled => "cancelled",
            Self::DownloadTimedOut => "no_progress_timeout",
        }
    }
}

pub fn network_failure_category(kind: std::io::ErrorKind) -> &'static str {
    use std::io::ErrorKind;
    match kind {
        ErrorKind::ConnectionRefused => "connection_refused",
        ErrorKind::ConnectionReset => "connection_reset",
        ErrorKind::ConnectionAborted => "connection_aborted",
        ErrorKind::NotConnected => "not_connected",
        ErrorKind::TimedOut => "network_timeout",
        ErrorKind::UnexpectedEof => "unexpected_eof",
        ErrorKind::BrokenPipe => "broken_pipe",
        ErrorKind::AddrNotAvailable => "address_unavailable",
        ErrorKind::PermissionDenied => "permission_denied",
        ErrorKind::WouldBlock => "would_block",
        _ => "network_error",
    }
}

impl From<std::num::ParseIntError> for SoulseekRs {
    fn from(err: std::num::ParseIntError) -> Self {
        SoulseekRs::ParseError(format!("Integer parse error: {}", err))
    }
}

impl From<String> for SoulseekRs {
    fn from(err: String) -> Self {
        SoulseekRs::CompressionError(err)
    }
}

/// Result type alias for the Soulseek library
pub type Result<T> = std::result::Result<T, SoulseekRs>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn network_categories_ignore_private_error_messages() {
        for (kind, category) in [
            (std::io::ErrorKind::ConnectionRefused, "connection_refused"),
            (std::io::ErrorKind::ConnectionReset, "connection_reset"),
            (std::io::ErrorKind::UnexpectedEof, "unexpected_eof"),
            (std::io::ErrorKind::TimedOut, "network_timeout"),
        ] {
            let error = SoulseekRs::from(std::io::Error::new(kind, "private-user /private/file"));
            assert_eq!(error.failure_category(), category);
        }
    }

    #[test]
    fn classified_failure_retains_cause_without_displaying_it() {
        let error = SoulseekRs::DownloadFailure {
            category: "handshake_failed",
            source: Box::new(std::io::Error::other("private-user /private/file")),
        };
        assert_eq!(error.failure_category(), "handshake_failed");
        assert!(error.source().unwrap().to_string().contains("private-user"));
        assert!(!error.to_string().contains("private"));
    }
}
