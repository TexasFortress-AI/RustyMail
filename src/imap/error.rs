use thiserror::Error;
// Remove FlagError import as it doesn't seem to exist
// use imap_types::flag::FlagError;
// Remove unused Infallible
// use std::convert::Infallible;

#[derive(Error, Debug, Clone)]
pub enum ImapError {
    #[error("Connection Error: {0}")]
    Connection(String),
    #[error("TLS Error: {0}")]
    Tls(String),
    #[error("Authentication Error: {0}")]
    Auth(String),
    #[error("Parse Error: {0}")]
    Parse(String),
    #[error("Bad Server Response: {0}")]
    BadResponse(String),
    #[error("Mailbox Operation Error: {0}")]
    Mailbox(String),
    #[error("Fetch Error: {0}")]
    Fetch(String),
    #[error("Append Error: {0}")]
    Append(String),
    #[error("Operation Error: {0}")]
    Operation(String),
    #[error("Command Error: {0}")]
    Command(String),
    #[error("Configuration Error: {0}")]
    Config(String),
    #[error("IO Error: {0}")]
    Io(String),
    #[error("Internal Error: {0}")]
    Internal(String),

    // Add other specific errors as needed from async-imap or imap-types
}

// Simplify the From<async_imap::error::Error> implementation
impl From<async_imap::error::Error> for ImapError {
    fn from(err: async_imap::error::Error) -> Self {
        log::warn!("Converting async_imap::Error to generic ImapError: {:?}", err);
        match err {
            // Convert io_err to String for the Io variant
            async_imap::error::Error::Io(io_err) => ImapError::Io(io_err.to_string()),
            _ => ImapError::Operation(err.to_string()),
        }
    }
}

// Remove the manual From<std::io::Error> as #[from] handles it.
// impl From<std::io::Error> for ImapError { ... }

// Remove From<rustls::Error> for now, can be added later if required.
// impl From<tokio_rustls::rustls::Error> for ImapError { ... }

// Remove From implementation for FlagError
// impl From<FlagError<'_, Infallible>> for ImapError {
//     fn from(err: FlagError<'_, Infallible>) -> Self {
//         ImapError::Operation(format!("Flag error: {}", err))
//     }
// }


