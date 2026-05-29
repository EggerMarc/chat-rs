//! Generic OpenAI Responses API client.
//!
//! Targets the `/responses` endpoint and its SSE event stream
//! (`response.created`, `response.output_text.delta`, etc.). This crate
//! owns the wire types — provider crates that ship a Responses-API
//! surface (today: `chat-openai`; planned: `chat-groq`'s Responses
//! path) wrap [`ResponsesBuilder`] and preset URL + auth + any
//! provider-specific tool declarations.
//!
//! Provider-specific native tools (OpenAI's `web_search`,
//! `image_generation`, etc.) are passed in as **pre-materialized
//! `Value`s** via [`ResponsesBuilder::with_tool_declaration`]. The
//! wrapper owns the tool trait; this crate stays trait-agnostic.
//!
//! ```no_run
//! use chat_responses::ResponsesBuilder;
//!
//! let client = ResponsesBuilder::new()
//!     .with_base_url("https://api.openai.com/v1")
//!     .with_model("gpt-4o")
//!     .with_api_key("sk-...")
//!     .build();
//! ```

mod api;
mod client;

use std::marker::PhantomData;

use chat_core::types::provider_meta::ProviderMeta;

pub use crate::api::types::error::{
    ResponsesErrorDetail, ResponsesErrorResponse, handle_responses_error,
};
pub use crate::api::types::request::{ReasoningConfig, ResponsesRequest, ResponsesRequestConfig};
pub use crate::api::types::response::{
    ResponsesApiResponse, ResponsesContentPart, ResponsesFunctionCall, ResponsesImageGenerationCall,
    ResponsesMessage, ResponsesOutputItem, ResponsesReasoning, ResponsesSummaryPart, ResponsesUsage,
    ResponsesWebSearchCall, output_items_to_parts,
};
pub use crate::client::ResponsesClient;
pub use chat_core::error::{ChatError, ChatFailure};
pub use chat_core::transport::{Request, ReqwestTransport, Response, Transport, TransportError};

use serde_json::Value;

pub struct WithoutModel;
pub struct WithModel;

pub struct WithoutUrl;
pub struct WithUrl;

pub struct ResponsesBuilder<M = WithoutModel, U = WithoutUrl, T: Transport = ReqwestTransport> {
    model_name: Option<String>,
    api_key: Option<String>,
    scheme: String,
    host: String,
    base_path: String,
    extra_tool_declarations: Vec<Value>,
    reasoning_effort: Option<String>,
    use_previous_response_id: bool,
    store: Option<bool>,
    transport: Option<T>,
    meta: ProviderMeta,
    _m: PhantomData<M>,
    _u: PhantomData<U>,
}

impl Default for ResponsesBuilder<WithoutModel, WithoutUrl, ReqwestTransport> {
    fn default() -> Self {
        Self::new()
    }
}

impl ResponsesBuilder<WithoutModel, WithoutUrl, ReqwestTransport> {
    pub fn new() -> Self {
        Self {
            model_name: None,
            api_key: None,
            scheme: String::new(),
            host: String::new(),
            base_path: String::new(),
            extra_tool_declarations: Vec::new(),
            reasoning_effort: None,
            use_previous_response_id: true,
            store: None,
            transport: Some(ReqwestTransport::default()),
            meta: ProviderMeta::default(),
            _m: PhantomData,
            _u: PhantomData,
        }
    }
}

impl<M, U, T: Transport> ResponsesBuilder<M, U, T> {
    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    pub fn with_reasoning_effort(mut self, effort: impl Into<String>) -> Self {
        self.reasoning_effort = Some(effort.into());
        self
    }

    pub fn without_previous_response_id(mut self) -> Self {
        self.use_previous_response_id = false;
        self
    }

    pub fn with_store(mut self, store: bool) -> Self {
        self.store = Some(store);
        self
    }

    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.meta.description = Some(description.into());
        self
    }

    pub fn with_metadata(
        mut self,
        key: impl Into<String>,
        value: impl std::any::Any + Send + Sync + 'static,
    ) -> Self {
        self.meta.data.insert(key.into(), Box::new(value));
        self
    }

    /// Replace the whole `ProviderMeta`. Useful for wrappers that
    /// accumulate metadata on their own builder and want to pass it
    /// through wholesale at construction time.
    pub fn with_meta(mut self, meta: ProviderMeta) -> Self {
        self.meta = meta;
        self
    }

    /// Append a single pre-materialized tool declaration. Wrappers use
    /// this to inject their provider-specific native tools without
    /// coupling the wire layer to a NativeTool trait.
    pub fn with_tool_declaration(mut self, declaration: Value) -> Self {
        self.extra_tool_declarations.push(declaration);
        self
    }

    pub fn with_tool_declarations(mut self, declarations: impl IntoIterator<Item = Value>) -> Self {
        self.extra_tool_declarations.extend(declarations);
        self
    }

    pub fn with_transport<T2: Transport>(self, transport: T2) -> ResponsesBuilder<M, U, T2> {
        ResponsesBuilder {
            model_name: self.model_name,
            api_key: self.api_key,
            scheme: self.scheme,
            host: self.host,
            base_path: self.base_path,
            extra_tool_declarations: self.extra_tool_declarations,
            reasoning_effort: self.reasoning_effort,
            use_previous_response_id: self.use_previous_response_id,
            store: self.store,
            transport: Some(transport),
            meta: self.meta,
            _m: PhantomData,
            _u: PhantomData,
        }
    }
}

impl<U, T: Transport> ResponsesBuilder<WithoutModel, U, T> {
    pub fn with_model(self, model: impl Into<String>) -> ResponsesBuilder<WithModel, U, T> {
        ResponsesBuilder {
            model_name: Some(model.into()),
            api_key: self.api_key,
            scheme: self.scheme,
            host: self.host,
            base_path: self.base_path,
            extra_tool_declarations: self.extra_tool_declarations,
            reasoning_effort: self.reasoning_effort,
            use_previous_response_id: self.use_previous_response_id,
            store: self.store,
            transport: self.transport,
            meta: self.meta,
            _m: PhantomData,
            _u: PhantomData,
        }
    }
}

impl<M, T: Transport> ResponsesBuilder<M, WithoutUrl, T> {
    pub fn with_base_url(self, url: impl Into<String>) -> ResponsesBuilder<M, WithUrl, T> {
        let url = url.into();
        let parsed = url::Url::parse(&url).expect("Invalid base URL");
        let scheme = parsed.scheme().to_string();
        let host = parsed.host_str().expect("No host in URL").to_string()
            + &parsed.port().map(|p| format!(":{p}")).unwrap_or_default();
        let base_path = parsed.path().trim_end_matches('/').to_string();
        ResponsesBuilder {
            model_name: self.model_name,
            api_key: self.api_key,
            scheme,
            host,
            base_path,
            extra_tool_declarations: self.extra_tool_declarations,
            reasoning_effort: self.reasoning_effort,
            use_previous_response_id: self.use_previous_response_id,
            store: self.store,
            transport: self.transport,
            meta: self.meta,
            _m: PhantomData,
            _u: PhantomData,
        }
    }
}

impl<T: Transport> ResponsesBuilder<WithModel, WithUrl, T> {
    pub fn build(self) -> ResponsesClient<T> {
        let api_key = self
            .api_key
            .expect("No API key. Call .with_api_key() before .build().");

        let transport = self.transport.expect("transport set");
        let model = self.model_name.expect("model set");

        ResponsesClient {
            model_name: model,
            api_key,
            scheme: self.scheme,
            host: self.host,
            base_path: self.base_path,
            transport,
            extra_tool_declarations: self.extra_tool_declarations,
            reasoning_effort: self.reasoning_effort,
            use_previous_response_id: self.use_previous_response_id,
            last_response_id: None,
            store: self.store,
            meta: self.meta,
        }
    }
}
