mod traits;
mod types;
pub mod sse;

#[cfg(feature = "reqwest-transport")]
mod reqwest;

pub use traits::Transport;
pub use types::{Event, EventStream, Request, Response, TransportError};

#[cfg(feature = "reqwest-transport")]
pub use self::reqwest::ReqwestTransport;
