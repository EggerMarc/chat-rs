//! # Chat-RS
//!
//! A multi-provider LLM framework.
//!
//! This crate provides a unified, type-safe API for interacting with Large Language Models
//! like Google Gemini, Anthropic Claude, and OpenAI. It features compile-time safe builders,
//! automatic retry loops, and native tool execution.

pub use chat_core::retry_strategy;

pub mod transport {
    pub use chat_core::transport::*;
}

pub use chat_core::{
    builder::ChatBuilder,
    chat::Chat,
    chat::state::{Structured, Unstructured},
    error::{ChatError, ChatFailure},
    traits::{CompletionProvider, EmbeddingsProvider},
    transport::Transport,
    types,
    types::{
        callback::{CallbackRetryContext, CallbackStrategy, RetryStrategy},
        messages::{
            Messages,
            content::Content,
            parts::{PartEnum, Parts},
            tool::{Tool, ToolStatus},
        },
        metadata::Metadata,
        options::ChatOptions,
        provider_meta::ProviderMeta,
        response::{ChatOutcome, ChatResponse, EmbeddingsResponse, PauseReason},
        tools::{Action, ScopedCollection, TypedCollection},
    },
};

#[cfg(feature = "stream")]
pub use chat_core::{
    chat::state::Streamed, traits::ChatProvider, traits::StreamProvider,
    types::response::StreamEvent,
};

#[cfg(feature = "gemini")]
pub mod gemini {
    pub use chat_gemini::*;
}


#[cfg(feature = "claude")]
pub mod claude {
    pub use chat_claude::*;
}

#[cfg(feature = "openai")]
pub mod openai {
    pub use chat_openai::*;
}

#[cfg(feature = "router")]
pub mod router {
    pub use chat_router::*;
}

pub mod prelude {
    pub use crate::ChatOptions;
    pub use crate::Messages;
    pub use crate::types;
    pub use crate::{ChatError, ChatFailure};
    pub use crate::{CompletionProvider, EmbeddingsProvider};

    #[cfg(feature = "gemini")]
    pub use crate::gemini;

    #[cfg(feature = "claude")]
    pub use crate::claude;

    #[cfg(feature = "openai")]
    pub use crate::openai;

    #[cfg(feature = "router")]
    pub use crate::router;
}
