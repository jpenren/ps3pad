use std::io;

use thiserror::Error;

/// Errors produced by the platform layer.
#[derive(Debug, Error)]
pub enum PlatformError {
    #[error("{context}: {source}")]
    Io {
        context: &'static str,
        #[source]
        source: io::Error,
    },

    #[error("this feature is not implemented on {os}")]
    Unsupported { os: &'static str },

    #[error("no supported controller found")]
    NoSupportedDevice,

    #[error("controller disappeared")]
    Disconnected,

    #[error("{0}")]
    Message(String),

    #[error(transparent)]
    Protocol(#[from] ps3pad_core::ParseError),
}

impl PlatformError {
    pub fn io(context: &'static str, source: io::Error) -> Self {
        Self::Io { context, source }
    }

    pub fn message(message: impl Into<String>) -> Self {
        Self::Message(message.into())
    }

    pub fn is_disconnected(&self) -> bool {
        match self {
            Self::Disconnected => true,
            Self::Io { source, .. } => matches!(
                source.kind(),
                io::ErrorKind::BrokenPipe
                    | io::ErrorKind::NotConnected
                    | io::ErrorKind::UnexpectedEof
            ),
            _ => false,
        }
    }
}
