//! # Chat-RS
//!
//! A multi-provider LLM framework.
//!
//! This crate provides a unified, type-safe API for interacting with Large Language Models
//! like Google Gemini, Anthropic Claude, and OpenAI. It features compile-time safe builders,
//! automatic retry loops, and native tool execution.

pub use chat_core::{parts, retry_strategy};

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

#[cfg(feature = "completions")]
pub mod completions {
    pub use chat_completions::*;
}

#[cfg(feature = "responses")]
pub mod responses {
    pub use chat_responses::*;
}

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

#[cfg(feature = "ollama")]
pub mod ollama {
    pub use chat_ollama::*;
}

#[cfg(feature = "huggingface")]
pub mod huggingface {
    pub use chat_huggingface::*;
}

#[cfg(feature = "cerebras")]
pub mod cerebras {
    pub use chat_cerebras::*;
}

#[cfg(feature = "deepseek")]
pub mod deepseek {
    pub use chat_deepseek::*;
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

    #[cfg(feature = "completions")]
    pub use crate::completions;

    #[cfg(feature = "responses")]
    pub use crate::responses;

    #[cfg(feature = "gemini")]
    pub use crate::gemini;

    #[cfg(feature = "claude")]
    pub use crate::claude;

    #[cfg(feature = "openai")]
    pub use crate::openai;

    #[cfg(feature = "ollama")]
    pub use crate::ollama;

    #[cfg(feature = "huggingface")]
    pub use crate::huggingface;

    #[cfg(feature = "cerebras")]
    pub use crate::cerebras;

    #[cfg(feature = "deepseek")]
    pub use crate::deepseek;

    #[cfg(feature = "router")]
    pub use crate::router;
}
