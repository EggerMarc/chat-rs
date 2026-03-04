mod code_execution;
mod google_maps;
mod google_search;
pub mod lib;

use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_json::{Map, Value, json};
use std::env;
use tools_rs::{FunctionCall, ToolCollection};

use crate::core::lib::ChatOptions;
use crate::core::{
    lib::{ChatError, ChatProvider},
    messages::{Messages, content::Content, text::Text},
};
use crate::gemini::code_execution::CodeExecutionTool;
use crate::gemini::google_maps::GoogleMapsTool;
use crate::gemini::google_search::GoogleSearchTool;
use crate::gemini::lib::GeminiNativeTool;
use crate::lib::{ChatFailure, ChatResponse};
use crate::messages::content::{CompleteReasonEnum, RoleEnum};
use crate::messages::embeddings::Embeddings;
use crate::messages::file::File;
use crate::messages::parts::{self, PartEnum, Parts};
use crate::metadata::Metadata;
use crate::metadata::usage::Usage;

#[derive(Clone, Default)]
pub struct FunctionCallingConfig {
    pub mode: Option<String>, // "AUTO", "ANY", "NONE"
    pub allowed_function_names: Option<Vec<String>>,
}

#[derive(Default, Clone)]
pub enum EmbeddingsTask {
    SemanticSimilarity,
    Classification,
    Clustering,
    RetrievalDocument,
    RetrievalQuery,
    #[default]
    Embed,
}

#[derive(Clone, Default)]
pub struct EmbeddingsConfig {
    pub dimensions: Option<usize>,
    pub task: EmbeddingsTask,
}

pub struct GeminiBuilder {
    model_name: Option<String>,
    api_key: Option<String>,
    native_tools: Vec<Box<dyn GeminiNativeTool>>,
    function_config: Option<FunctionCallingConfig>,
    embeddings_config: Option<EmbeddingsConfig>,
}

impl Default for GeminiBuilder {
    /// Creates a new GeminiBuilder with default configuration.
    ///
    /// # Examples
    ///
    /// ```
    /// let _builder = GeminiBuilder::default();
    /// ```
    fn default() -> Self {
        Self::new()
    }
}

impl GeminiBuilder {
    /// Creates a new GeminiBuilder with default (empty) configuration.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut builder = GeminiBuilder::new();
    /// builder.with_model("gemini-2.0-flash-exp".to_string())
    ///        .with_api_key("MY_KEY".to_string());
    /// let client = builder.build();
    /// ```
    pub fn new() -> Self {
        Self {
            model_name: None,
            api_key: None,
            native_tools: Vec::new(),
            function_config: None,
            embeddings_config: None,
        }
    }

    /// Sets the model name to use when building a GeminiClient.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut b = GeminiBuilder::new();
    /// b.with_model("gemini-2.0-flash-exp".to_string());
    /// let client = b.build();
    /// ```
    pub fn with_model(&mut self, model_name: String) -> &mut Self {
        self.model_name = Some(model_name);
        self
    }

    /// Set the API key that will be used for Gemini requests by this builder.
    ///
    /// Returns a mutable reference to the builder for chaining.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut builder = GeminiBuilder::new();
    /// builder.with_api_key("sk-xxxx".to_string());
    /// ```
    pub fn with_api_key(&mut self, api_key: String) -> &mut Self {
        self.api_key = Some(api_key);
        self
    }

    /// Adds a CodeExecutionTool to the builder's native tools, enabling code-execution capabilities.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut b = GeminiBuilder::new();
    /// b.with_code_execution();
    /// // builder now contains a CodeExecutionTool and can be further configured or built
    /// ```
    pub fn with_code_execution(&mut self) -> &mut Self {
        self.native_tools.push(Box::new(CodeExecutionTool));
        self
    }

    /// Adds a Google Search native tool to the builder with the default (unset) dynamic threshold.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut builder = GeminiBuilder::new();
    /// builder.with_model("gemini-2.0-flash-exp".into())
    ///        .with_google_search();
    /// ```
    pub fn with_google_search(&mut self) -> &mut Self {
        self.native_tools.push(Box::new(GoogleSearchTool {
            dynamic_threshold: None,
        }));
        self
    }

    /// Adds a Google Search native tool configured with the given dynamic relevance threshold to the builder.
    ///
    /// The threshold controls the tool's dynamic scoring behavior (e.g., relevance sensitivity).
    ///
    /// # Examples
    ///
    /// ```
    /// let mut b = GeminiBuilder::new();
    /// b.with_google_search_threshold(0.7);
    /// ```
    pub fn with_google_search_threshold(&mut self, threshold: f32) -> &mut Self {
        self.native_tools.push(Box::new(GoogleSearchTool {
            dynamic_threshold: Some(threshold),
        }));
        self
    }

    /// Adds a Google Maps native tool to the builder with optional initial coordinates and an optional map widget.
    ///
    /// - `lat_lng`: Optional tuple with latitude and longitude to center the map when the tool is used.
    /// - `widget`: If `true`, enables an embeddable map widget for the tool; if `false`, the tool provides non-widget responses.
    ///
    /// # Returns
    ///
    /// A mutable reference to the builder to allow method chaining.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut builder = GeminiBuilder::new();
    /// builder.with_google_maps(Some((37.7749, -122.4194)), true);
    /// let client = builder.build();
    /// ```
    pub fn with_google_maps(&mut self, lat_lng: Option<(f32, f32)>, widget: bool) -> &mut Self {
        self.native_tools.push(Box::new(GoogleMapsTool {
            lat_lng,
            enable_widget: widget,
        }));
        self
    }

    /// Set the function-calling behavior and optional whitelist of allowed function names for the builder.
    ///
    /// `mode` should be one of `"AUTO"`, `"ANY"`, or `"NONE"`, controlling how the model may invoke functions.
    /// `allowed` can be provided to restrict function calls to a specific list of function names.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut b = GeminiBuilder::new();
    /// b.with_function_calling_mode("AUTO", Some(vec!["search".into(), "calc".into()]));
    /// let client = b.build();
    /// ```
    ///
    /// Returns a mutable reference to the modified builder.
    pub fn with_function_calling_mode(
        &mut self,
        mode: &str,
        allowed: Option<Vec<String>>,
    ) -> &mut Self {
        self.function_config = Some(FunctionCallingConfig {
            mode: Some(mode.to_string()),
            allowed_function_names: allowed,
        });
        self
    }

    pub fn with_embeddings(&mut self, dimensions: Option<usize>) -> &mut Self {
        if let Some(config) = &self.embeddings_config {
            self.embeddings_config = Some(EmbeddingsConfig {
                dimensions,
                ..config.clone()
            })
        } else {
            self.embeddings_config = Some(EmbeddingsConfig {
                dimensions,
                ..Default::default()
            });
        }
        self
    }

    pub fn with_embeddings_task(&mut self, task: EmbeddingsTask) -> &mut Self {
        if let Some(config) = &self.embeddings_config {
            self.embeddings_config = Some(EmbeddingsConfig {
                task,
                ..config.clone()
            })
        } else {
            self.embeddings_config = Some(EmbeddingsConfig {
                task,
                ..Default::default()
            })
        };
        self
    }

    /// Builds a `GeminiClient` from this builder, applying sensible defaults where fields are unset.
    ///
    /// The returned client is configured with the builder's current `model_name`, `api_key`,
    /// `native_tools`, and `function_config`. If `model_name` was not set, it defaults to
    /// "gemini-2.0-flash-exp". If `api_key` was not set, the `GEMINI_API_KEY` environment
    /// variable is read; the function will panic if that environment variable is missing.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut builder = GeminiBuilder::new();
    /// // with no model or api key set, build will use defaults and/or environment
    /// // (this example assumes GEMINI_API_KEY is set in the environment)
    /// let client = builder.build();
    /// assert!(client.model_name == "gemini-2.0-flash-exp");
    ///
    /// // setting values on the builder is preserved in the produced client
    /// let mut b2 = GeminiBuilder::new();
    /// b2.with_model("custom-model".to_string()).with_api_key("sk-test".to_string());
    /// let c2 = b2.build();
    /// assert_eq!(c2.model_name, "custom-model");
    /// assert_eq!(c2.api_key, "sk-test");
    /// ```
    pub fn build(&mut self) -> GeminiClient {
        GeminiClient {
            model_name: self
                .model_name
                .clone()
                .unwrap_or_else(|| "gemini-2.0-flash-exp".to_string()),
            api_key: self.api_key.clone().unwrap_or_else(|| {
                env::var("GEMINI_API_KEY").expect("Failed to find GEMINI_API_KEY from .env")
            }),
            native_tools: self.native_tools.clone(),
            function_config: self.function_config.clone(),
            embeddings_config: self.embeddings_config.clone(),
        }
    }
}

pub struct GeminiClient {
    model_name: String,
    api_key: String,
    native_tools: Vec<Box<dyn GeminiNativeTool>>,
    function_config: Option<FunctionCallingConfig>,
    embeddings_config: Option<EmbeddingsConfig>,
}

impl GeminiClient {
    /// Creates a GeminiClient configured with the `GEMINI_API_KEY` taken from the environment.
    ///
    /// If the `GEMINI_API_KEY` environment variable is missing or cannot be read, this function
    /// returns an error.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::env;
    /// // Set the env var for the example
    /// env::set_var("GEMINI_API_KEY", "test-key");
    /// let client = crate::providers::gemini::GeminiClient::new("gemini-2.0-flash-exp").unwrap();
    /// assert_eq!(client.model_name, "gemini-2.0-flash-exp");
    /// ```
    pub fn new(model_name: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let api_key = env::var("GEMINI_API_KEY")?;

        Ok(GeminiClient {
            model_name: model_name.to_string(),
            api_key,
            native_tools: Vec::new(),
            function_config: None,
            embeddings_config: None,
        })
    }
}

#[async_trait]
impl ChatProvider for GeminiClient {
    /// Send the provided messages to the Gemini model and return the model's parsed response as `Content`.
    ///
    /// If an embeddings configuration is set on the client, the request will target Gemini's embedding task and return embedding results; otherwise it will request generated content.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// // Given a configured `GeminiClient` named `client` and a `Messages` value:
    /// // let content = client.complete(&messages, None, None, None).await.unwrap();
    /// ```
    ///
    /// # Returns
    ///
    /// `Ok(ChatResponse)` containing the parsed `Content` and associated metadata on success; `Err(ChatFailure)` if the HTTP request, response parsing, or API call fails.
    async fn complete(
        &self,
        messages: &Messages,
        custom_tools: Option<&ToolCollection>,
        _options: Option<&ChatOptions>,
        structured_output: Option<&schemars::Schema>,
    ) -> Result<ChatResponse, ChatFailure> {
        let task = if self.embeddings_config.is_some() {
            ":embedContent"
        } else {
            ":generateContent"
        };

        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}{}",
            self.model_name, task
        );

        let body = build_request_body(
            messages,
            &self.model_name,
            custom_tools,
            structured_output,
            &self.native_tools,
            self.function_config.as_ref(),
            self.embeddings_config.as_ref(),
        )
        .map_err(|err| ChatFailure {
            metadata: None,
            err,
        })?;

        let req = reqwest::Client::new()
            .post(url)
            .header("Content-Type", "application/json")
            .header("x-goog-api-key", &self.api_key)
            .body(body.to_string());

        let res = req.send().await.map_err(|e| ChatFailure {
            err: ChatError::Provider(e.to_string()),
            metadata: None,
        })?;
        match res.error_for_status() {
            Ok(data) => {
                let text = data.text().await.map_err(|e| ChatFailure {
                    err: ChatError::Provider(e.to_string()),
                    metadata: None,
                })?;

                let json: Value = serde_json::from_str(&text).map_err(|e| ChatFailure {
                    err: ChatError::InvalidResponse(e.to_string()),
                    metadata: None,
                })?;

                let content = parse_gemini_response(&json).map_err(|err| ChatFailure {
                    err,
                    metadata: Some(parse_metadata(&json)),
                })?;
                let metadata = parse_metadata(&json);
                Ok(ChatResponse {
                    content,
                    metadata: Some(metadata),
                })
            }
            Err(err) => {
                eprintln!("Gemini API Error: {}", err);
                Err(ChatFailure {
                    err: ChatError::Provider(err.without_url().to_string()),
                    metadata: None,
                })
            }
        }
    }
}

/// Assembles a Gemini Generative Language API request body from messages, tools, and optional configurations.
///
/// The produced JSON may include a sanitized `generationConfig` (when `structured_output` is provided),
/// a `system_instruction` (from system-role messages), `contents` (from non-system messages),
/// and optional `tools` and `toolConfig` fields built from `custom_tools`, `native_tools`, and `function_config`.
/// When `embeddings_config` is present the body is constructed for an embeddings request and will contain
/// `model`, `content` (from the last message's parts), an optional `taskType`, and optional `output_dimensionality`.
///
/// # Returns
///
/// A `serde_json::Value` representing the assembled request body, or a `ChatError` if schema serialization
/// or tool/config assembly fails.
///
/// # Examples
///
/// ```no_run
/// // Conceptual example (types omitted for brevity):
/// // let messages: Messages = ...;
/// // let custom_tools: Option<&ToolCollection> = None;
/// // let schema: Option<&schemars::Schema> = None;
/// // let native_tools: Vec<Box<dyn GeminiNativeTool>> = vec![];
/// // let function_config: Option<&FunctionCallingConfig> = None;
/// // let embeddings_config: Option<&EmbeddingsConfig> = None;
/// // let body = build_request_body(
/// //     &messages,
/// //     "gemini-2.0",
/// //     custom_tools,
/// //     schema,
/// //     &native_tools,
/// //     function_config,
/// //     embeddings_config,
/// // )?;
/// ```
fn build_request_body(
    messages: &Messages,
    model_name: &str,
    custom_tools: Option<&ToolCollection>,
    structured_output: Option<&schemars::Schema>,
    native_tools: &[Box<dyn GeminiNativeTool>],
    function_config: Option<&FunctionCallingConfig>,
    embeddings_config: Option<&EmbeddingsConfig>,
) -> Result<Value, ChatError> {
    validate_combinations(custom_tools, structured_output, native_tools);

    let mut body = json!({});

    if let Some(config) = embeddings_config {
        let model_name = format!(
            "models/{}",
            model_name.split(":").next().unwrap_or(model_name)
        );

        body["model"] = Value::String(model_name.to_string());

        let content = messages.last().ok_or(ChatError::Provider(
            "Sent empty content to embed, expected Text parts".to_string(),
        ))?;

        let parts: Vec<Value> = content
            .parts
            .0
            .iter()
            .map(|part| match part {
                PartEnum::Text(t) | PartEnum::Reasoning(t) => Ok(json!({ "text": t.as_str() })),
                _ => Err(ChatError::InvalidResponse(
                    "Embedding requests require text-like parts only".to_string(),
                )),
            })
            .collect::<Result<Vec<_>, _>>()?;

        if parts.is_empty() {
            return Err(ChatError::InvalidResponse(
                "Sent empty content to embed, expected Text parts".to_string(),
            ));
        }

        body["content"] = json!({ "parts": parts });
        match config.task {
            EmbeddingsTask::SemanticSimilarity => {
                body["taskType"] = Value::String("SEMANTIC_SIMILARITY".to_string());
            }
            EmbeddingsTask::Clustering => {
                body["taskType"] = Value::String("CLUSTERING".to_string())
            }
            EmbeddingsTask::Embed => {}
            _ => {}
        };

        if let Some(dims) = config.dimensions {
            body["output_dimensionality"] = Value::Number(dims.into());
        }

        return Ok(body);
    }

    if let Some(schema) = structured_output {
        let mut clean_schema = serde_json::to_value(schema)
            .map_err(|e| ChatError::Other(format!("Schema serialization error: {}", e)))?;
        sanitize_schema_for_gemini(&mut clean_schema);

        body["generationConfig"] = json!({
            "responseMimeType": "application/json",
            "responseSchema": clean_schema
        });
    }

    if let Some(system) = build_system_instruction(messages) {
        body["system_instruction"] = system;
    }

    if let Some(contents) = build_contents(messages) {
        body["contents"] = contents;
    }

    let (tools_json, config_json) =
        build_tools_and_config(custom_tools, native_tools, function_config)?;

    if let Some(t) = tools_json {
        body["tools"] = t;
    }
    if let Some(c) = config_json {
        body["toolConfig"] = c;
    }

    Ok(body)
}

/// Logs warnings for incompatible feature combinations when building a Gemini request.
///
/// Specifically warns if:
/// - a native Google Search tool is present together with function declarations (`custom_tools`),
/// - a native Google Search tool is present together with structured output (`structured_output`),
/// - function declarations are present together with structured output.
///
/// # Arguments
///
/// * `custom_tools` - Optional collection of user-provided function declarations.
/// * `structured_output` - Optional JSON Schema used to request structured/parsing output.
/// * `native_tools` - Slice of native tools; tools that return `true` from `is_search()` are treated as Google Search.
///
/// # Examples
///
/// ```
/// // No warnings
/// validate_combinations(None, None, &[]);
/// ```
fn validate_combinations(
    custom_tools: Option<&ToolCollection>,
    structured_output: Option<&schemars::Schema>,
    native_tools: &[Box<dyn GeminiNativeTool>],
) {
    let has_search = native_tools.iter().any(|t| t.is_search());
    let has_functions = custom_tools.is_some();
    let has_structured_output = structured_output.is_some();

    if has_search && has_functions {
        eprintln!(
            "WARNING: Google Search is generally not compatible with Function Declarations in the same request."
        );
    }
    if has_search && has_structured_output {
        eprintln!(
            "WARNING: Google Search is generally not compatible with Structured Parsers (JSON Schema) in the same request."
        );
    }
    if has_functions && has_structured_output {
        eprintln!(
            "WARNING: Structured Parsers (JSON Schema) are not valid combined with Function Declarations."
        );
    }
}

/// Builds the JSON `tools` array and a per-tool `toolConfig` object for the Gemini request.
///
/// Produces an optional JSON array of tool declarations (combining custom tool declarations and native tool declarations)
/// and an optional JSON object of per-tool configuration entries (including native tool configs and an optional
/// `functionCallingConfig` assembled from `function_config`).
///
/// # Parameters
///
/// - `custom_tools`: Optional collection of custom tools whose JSON declarations will be included under
///   a `functionDeclarations` entry if present.
/// - `native_tools`: Slice of native tools; each contributes a tool declaration and may add a key/value entry
///   to the returned tool configuration.
/// - `function_config`: Optional function-calling configuration; if it has fields set, they are inserted under
///   the `functionCallingConfig` key in the tool configuration.
///
/// # Returns
///
/// A tuple `(tools, tool_config)` where:
/// - `tools` is `Some(Value::Array(_))` when there is at least one tool declaration, otherwise `None`.
/// - `tool_config` is `Some(Value::Object(_))` when there is at least one configuration entry, otherwise `None`.
///
/// # Examples
///
/// ```
/// // Call with no custom or native tools and no function config:
/// let res = crate::providers::gemini::build_tools_and_config(None, &[], None).unwrap();
/// assert_eq!(res, (None, None));
/// ```
fn build_tools_and_config(
    custom_tools: Option<&ToolCollection>,
    native_tools: &[Box<dyn GeminiNativeTool>],
    function_config: Option<&FunctionCallingConfig>,
) -> Result<(Option<Value>, Option<Value>), ChatError> {
    let mut tools_list = Vec::new();
    let mut tool_config_map = serde_json::Map::new();

    if let Some(ct) = custom_tools {
        let declarations = ct
            .json()
            .map_err(|err| ChatError::Other(format!("Tools-rs serialization error: {}", err)))?;

        tools_list.push(json!({
            "functionDeclarations": declarations
        }));
    }

    for tool in native_tools {
        tools_list.push(tool.to_tool_declaration());

        if let Some((key, value)) = tool.to_tool_config() {
            tool_config_map.insert(key, value);
        }
    }

    if let Some(fc_conf) = function_config {
        let mut fc_json = json!({});
        if let Some(ref mode) = fc_conf.mode {
            fc_json["mode"] = json!(mode);
        }
        if let Some(ref allowed) = fc_conf.allowed_function_names {
            fc_json["allowedFunctionNames"] = json!(allowed);
        }

        if !fc_json.as_object().unwrap().is_empty() {
            tool_config_map.insert("functionCallingConfig".to_string(), fc_json);
        }
    }

    let final_tools = if tools_list.is_empty() {
        None
    } else {
        Some(Value::Array(tools_list))
    };

    let final_config = if tool_config_map.is_empty() {
        None
    } else {
        Some(Value::Object(tool_config_map))
    };

    Ok((final_tools, final_config))
}

/// Recursively remove keys that Gemini does not accept from a JSON schema value.
///
/// This function walks the given `serde_json::Value` in-place and removes the following keys
/// from any object it encounters: `$schema`, `title`, `$id`, `additionalProperties`, and `definitions`.
/// Arrays and nested objects are traversed recursively so the sanitization is applied throughout the schema.
///
/// # Examples
///
/// ```
/// use serde_json::json;
/// let mut schema = json!({
///     "$schema": "http://example",
///     "title": "Example",
///     "properties": {
///         "inner": {
///             "$id": "id",
///             "definitions": { "x": {} },
///             "type": "object"
///         }
///     },
///     "additionalProperties": false
/// });
///
/// sanitize_schema_for_gemini(&mut schema);
///
/// // Top-level removals
/// assert!(!schema.as_object().unwrap().contains_key("$schema"));
/// assert!(!schema.as_object().unwrap().contains_key("title"));
/// assert!(!schema.as_object().unwrap().contains_key("additionalProperties"));
///
/// // Nested removals
/// let inner = &schema["properties"]["inner"];
/// assert!(!inner.as_object().unwrap().contains_key("$id"));
/// assert!(!inner.as_object().unwrap().contains_key("definitions"));
/// ```
fn sanitize_schema_for_gemini(schema: &mut Value) {
    if let Value::Object(map) = schema {
        map.remove("$schema");
        map.remove("title");
        map.remove("$id");
        map.remove("additionalProperties");
        map.remove("definitions");
        for (_, v) in map {
            sanitize_schema_for_gemini(v);
        }
    } else if let Value::Array(arr) = schema {
        for v in arr {
            sanitize_schema_for_gemini(v);
        }
    }
}

/// Build a Gemini-formatted system instruction object from messages.
///
/// Extracts all parts from messages whose role is `System`, converts each part to Gemini format,
/// and returns `Some` JSON object with a `"parts"` array when system parts are present; returns
/// `None` if there are no system-role parts.
///
/// # Examples
///
/// ```
/// // Given `messages` containing system parts:
/// let maybe_sys = build_system_instruction(&messages);
/// if let Some(obj) = maybe_sys {
///     // obj is a JSON object like `{ "parts": [ ... ] }`
///     assert!(obj.get("parts").is_some());
/// } else {
///     // no system parts were present
/// }
/// ```
fn build_system_instruction(messages: &Messages) -> Option<Value> {
    let sys_parts: Vec<Value> = messages
        .0
        .iter()
        .filter(|content| content.role == RoleEnum::System)
        .flat_map(|content| content.parts.0.iter().map(part_to_gemini))
        .collect();

    if sys_parts.is_empty() {
        None
    } else {
        Some(json!({ "parts": sys_parts }))
    }
}

/// Build the array of content objects for a Gemini request from `messages`.
///
/// Non-system messages are converted into Gemini content objects. Parts that are function responses
/// are collected into separate content entries with role `"function"`, while all other parts for a
/// message are combined into a single content entry with the message's role.
///
/// # Examples
///
/// ```
/// // Assuming `Messages`, `Content`, and `PartEnum` are in scope and constructible:
/// let messages = Messages(vec![
///     Content { role: RoleEnum::User, parts: Parts(vec![PartEnum::Text("hi".into())]) },
/// ]);
/// let body = build_contents(&messages);
/// assert!(body.is_some());
/// ```
fn build_contents(messages: &Messages) -> Option<Value> {
    let mut contents: Vec<Value> = Vec::new();

    for content in &messages.0 {
        if content.role == RoleEnum::System {
            continue;
        }

        let (function_responses, other_parts): (Vec<_>, Vec<_>) = content
            .parts
            .0
            .iter()
            .partition(|part| matches!(part, PartEnum::FunctionResponse(_)));

        if !other_parts.is_empty() {
            let parts: Vec<Value> = other_parts.iter().map(|p| part_to_gemini(p)).collect();
            contents.push(content_to_gemini_with_parts(content, parts));
        }

        if !function_responses.is_empty() {
            let parts: Vec<Value> = function_responses
                .iter()
                .map(|p| part_to_gemini(p))
                .collect();
            contents.push(json!({
                "role": "function",
                "parts": parts
            }));
        }
    }

    if contents.is_empty() {
        None
    } else {
        Some(Value::Array(contents))
    }
}

/// Convert a `Content` and its serialized parts into a Gemini-formatted content JSON object.
///
/// The resulting object contains a `role` field set to `"user"` for `RoleEnum::User`, `"model"` for
/// `RoleEnum::Model`, and `"user"` for any other roles, and a `parts` field containing the given parts.
///
/// # Examples
///
/// ```
/// use serde_json::json;
/// // assume Content and RoleEnum are in scope; create a user content for the example
/// let content = Content { role: RoleEnum::User, ..Default::default() };
/// let parts = vec![json!({"text":"hello"})];
/// let gemini = content_to_gemini_with_parts(&content, parts);
/// assert_eq!(gemini["role"], "user");
/// assert!(gemini["parts"].is_array());
/// ```
fn content_to_gemini_with_parts(content: &Content, parts: Vec<Value>) -> Value {
    let role_str = match content.role {
        RoleEnum::User => "user",
        RoleEnum::Model => "model",
        RoleEnum::System => "user",
    };
    json!({ "role": role_str, "parts": parts })
}

/// Convert a PartEnum into the JSON object shape expected by the Gemini API.
///
/// The returned `serde_json::Value` represents a single Gemini "part":
/// - `Text` and `Reasoning` produce `{ "text": "..." }`.
/// - `FunctionCall` produces `{ "functionCall": { "name": <name>, "args": <args> } }`.
/// - `FunctionResponse` produces `{ "functionResponse": { "name": <name>, "response": <object-or-content> } }`
///   where non-object responses are wrapped as `{ "content": <value> }`.
/// - `File::Url` produces `{ "file_data": { "file_uri": <uri>, "mime_type": <opt> } }`.
/// - `File::Bytes` produces `{ "inline_data": { "mime_type": <mime>, "data": <bytes> } }`.
/// - All other variants produce `{ "text": "" }`.
///
/// # Examples
///
/// ```
/// use serde_json::Value;
/// // Construct a text part; exact constructors depend on the crate's types.
/// let p = PartEnum::Text(String::from("hello"));
/// let v: Value = part_to_gemini(&p);
/// assert_eq!(v["text"], "hello");
/// ```
fn part_to_gemini(part: &PartEnum) -> Value {
    match part {
        PartEnum::Text(t) => json!({ "text": t.0}),
        PartEnum::Reasoning(r) => json!({ "text": r.0}),
        PartEnum::FunctionCall(fc) => json!({
            "functionCall": {
                "name": fc.name,
                "args": fc.arguments
            }
        }),
        PartEnum::FunctionResponse(fr) => {
            let response_content = if fr.result.is_object() {
                fr.result.clone()
            } else {
                json!({ "content": fr.result })
            };
            json!({
                "functionResponse": {
                    "name": fr.name,
                    "response": response_content
                }
            })
        }
        PartEnum::File(file) => match file {
            File::Url(url) => {
                let mut file_data = Map::new();
                file_data.insert("file_uri".to_string(), json!(url.url));

                if let Some(mimetype) = url.mimetype.clone() {
                    file_data.insert("mime_type".to_string(), json!(mimetype));
                }

                json!({
                    "file_data": Value::Object(file_data)
                })
            }
            File::Bytes(raw) => {
                let data_b64 = STANDARD.encode(&raw.bytes);
                json!({ "inline_data": { "mime_type": raw.mimetype, "data": data_b64}})
            }
        },
        _ => json!({ "text": ""}),
    }
}

/// Parse a Gemini API response into a Content value.
///
/// When `embedding.values` is present, parses embeddings into parts and returns a model Content.
/// Otherwise extracts the first candidate's `content`, converts its parts, maps the role and finish reason, and returns the resulting Content.
/// Returns an error if required fields are missing or if parts/embeddings parsing fails.
///
/// # Examples
///
/// ```
/// use serde_json::json;
///
/// let resp = json!({
///     "candidates": [
///         {
///             "content": {
///                 "parts": [ { "text": "hello" } ],
///                 "role": "model"
///             },
///             "finishReason": "STOP"
///         }
///     ]
/// });
///
/// let content = crate::providers::gemini::parse_gemini_response(&resp).unwrap();
/// assert_eq!(content.parts.len(), 1);
/// ```
fn parse_gemini_response(json: &Value) -> Result<Content, ChatError> {
    if let Some(value) = json.get("embedding").and_then(|json| json.get("values")) {
        let parts = parse_embeddings(value)?;
        return Ok(Content {
            role: RoleEnum::Model,
            parts,
            complete_reason: CompleteReasonEnum::None,
        });
    }

    let candidate = json
        .get("candidates")
        .and_then(|c| c.get(0))
        .ok_or_else(|| ChatError::InvalidResponse("No candidates in response".to_string()))?;

    let content_json = candidate
        .get("content")
        .ok_or_else(|| ChatError::InvalidResponse("No content in candidate".to_string()))?;

    let parts = parse_parts(content_json)?;
    let role = parse_role(content_json);
    let complete_reason = parse_finish_reason(candidate);

    let content = Content {
        parts,
        role,
        complete_reason,
    };
    Ok(content)
}

/// Parses the `"parts"` array of a Gemini content JSON object into the internal `Parts` representation.
///
/// Returns `Ok(Parts)` containing parsed `Text` and `FunctionCall` parts found in `content_json`.
/// Returns `Err(ChatError::InvalidResponse)` if a `functionCall` item is missing its `name` or `args` fields.
///
/// # Examples
///
/// ```
/// use serde_json::json;
/// let content = json!({
///     "parts": [
///         { "text": "hello" },
///         { "functionCall": { "name": "doThing", "args": { "x": 1 } } }
///     ]
/// });
/// let parts = parse_parts(&content).unwrap();
/// assert_eq!(parts.len(), 2);
/// ```
fn parse_parts(content_json: &Value) -> Result<Parts, ChatError> {
    let mut parts = Parts::default();
    if let Some(arr) = content_json.get("parts").and_then(|v| v.as_array()) {
        for item in arr {
            if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                parts.push(PartEnum::Text(Text::new(text)));
            }

            if let Some(fc) = item.get("functionCall") {
                let name = fc.get("name").and_then(|v| v.as_str()).ok_or_else(|| {
                    ChatError::InvalidResponse("Missing function call name".to_string())
                })?;
                let args = fc.get("args").and_then(|v| v.as_object()).ok_or_else(|| {
                    ChatError::InvalidResponse("Missing function call args".to_string())
                })?;
                parts.push(PartEnum::from_function_call(FunctionCall::new(
                    name.to_string(),
                    Value::Object(args.clone()),
                )));
            }
            if let Some(exec) = item.get("executableCode") {
                let language = exec
                    .get("language")
                    .and_then(|v| v.as_str())
                    .unwrap_or("code")
                    .to_lowercase();

                let code = exec.get("code").and_then(|v| v.as_str()).unwrap_or("");

                let md = format!("```{}\n{}\n```", language, code);

                parts.push(PartEnum::from_text(md));
            }

            if let Some(result) = item.get("codeExecutionResult") {
                let status = result
                    .get("outcome")
                    .and_then(|v| v.as_str())
                    .unwrap_or("result");

                let output = result.get("output").and_then(|v| v.as_str()).unwrap_or("");

                let md = format!("```{}\n{}\n```", status, output);

                parts.push(PartEnum::from_text(md));
            }
        }
    }
    Ok(parts)
}

/// Map a Gemini content JSON object's "role" field to the corresponding `RoleEnum`.
///
/// `content_json` is expected to be a JSON object that may contain a `"role"` string.
/// If the `"role"` field is missing or not one of the recognized values, this function
/// maps it to `RoleEnum::Model`.
///
/// # Returns
///
/// `RoleEnum` corresponding to the role: `User` for `"user"`, `System` for `"system"`,
/// `Model` for `"function"` or any other value.
///
/// # Examples
///
/// ```
/// use serde_json::json;
///
/// let user = json!({ "role": "user" });
/// assert_eq!(parse_role(&user), RoleEnum::User);
///
/// let system = json!({ "role": "system" });
/// assert_eq!(parse_role(&system), RoleEnum::System);
///
/// let func = json!({ "role": "function" });
/// assert_eq!(parse_role(&func), RoleEnum::Model);
///
/// let missing = json!({});
/// assert_eq!(parse_role(&missing), RoleEnum::Model);
/// ```
fn parse_role(content_json: &Value) -> RoleEnum {
    match content_json
        .get("role")
        .and_then(|v| v.as_str())
        .unwrap_or("model")
    {
        "user" => RoleEnum::User,
        "system" => RoleEnum::System,
        "function" => RoleEnum::Model,
        _ => RoleEnum::Model,
    }
}

/// Map a Gemini candidate's `finishReason` field to a `CompleteReasonEnum`.
///
/// Returns `CompleteReasonEnum::Stop` for `"STOP"`,
/// `CompleteReasonEnum::MaxTokens` for `"MAX_TOKENS"`,
/// and `CompleteReasonEnum::ContentFilter` for `"SAFETY"`, `"RECITATION"`, or `"OTHER"`.
/// Any missing or unrecognized value yields `CompleteReasonEnum::None`.
///
/// # Examples
///
/// ```
/// use serde_json::json;
/// let c = json!({ "finishReason": "STOP" });
/// assert_eq!(crate::parse_finish_reason(&c), CompleteReasonEnum::Stop);
///
/// let c2 = json!({ "finishReason": "UNKNOWN" });
/// assert_eq!(crate::parse_finish_reason(&c2), CompleteReasonEnum::None);
/// ```
fn parse_finish_reason(candidate: &Value) -> CompleteReasonEnum {
    let finish_reason = candidate
        .get("finishReason")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    match finish_reason {
        "STOP" => CompleteReasonEnum::Stop,
        "MAX_TOKENS" => CompleteReasonEnum::MaxTokens,
        "SAFETY" | "RECITATION" | "OTHER" => CompleteReasonEnum::Other(finish_reason.to_string()),
        _ => CompleteReasonEnum::None,
    }
}

pub fn parse_metadata(body: &Value) -> Metadata {
    Metadata {
        id: body
            .get("responseId")
            .and_then(Value::as_str)
            .map(str::to_string),
        model_slug: body
            .get("modelVersion")
            .and_then(Value::as_str)
            .map(str::to_string),
        usage: parse_usage(body),
        ..Metadata::default()
    }
}

/// Extracts token usage counts from a Gemini response JSON object.
///
/// Reads `usageMetadata.promptTokenCount`, `usageMetadata.candidatesTokenCount`, and
/// `usageMetadata.totalTokenCount` and returns a `Usage` with `input_tokens`, `output_tokens`,
/// and `total_tokens`. If any fields are missing, `promptTokenCount` and `candidatesTokenCount`
/// default to 0 and `totalTokenCount` defaults to `promptTokenCount + candidatesTokenCount`.
///
/// # Examples
///
/// ```
/// use serde_json::json;
///
/// let body = json!({
///     "usageMetadata": {
///         "promptTokenCount": 5u64,
///         "candidatesTokenCount": 7u64,
///         "totalTokenCount": 12u64
///     }
/// });
///
/// let usage = crate::providers::gemini::parse_usage(&body);
/// assert_eq!(usage.input_tokens, 5);
/// assert_eq!(usage.output_tokens, 7);
/// assert_eq!(usage.total_tokens, 12);
/// ```
pub fn parse_usage(body: &Value) -> Usage {
    let u = body.get("usageMetadata");

    let input = u
        .and_then(|v| v.get("promptTokenCount"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let output = u
        .and_then(|v| v.get("candidatesTokenCount"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let total = u
        .and_then(|v| v.get("totalTokenCount"))
        .and_then(Value::as_u64)
        .unwrap_or(input + output);

    Usage {
        input_tokens: input as usize,
        output_tokens: output as usize,
        total_tokens: total as usize,
    }
}

/// Parse a JSON value containing embedding vectors into a Parts collection.
///
/// Accepts either a single embedding as an array of numbers (e.g. [f1, f2, ...])
/// or a batched form (an array of embedding arrays). Each numeric value is
/// converted to `f32` and wrapped in an `Embeddings` part which is pushed into
/// the returned `Parts`.
///
/// Returns an error if the JSON is not an array, if a batched entry is not an
/// array, or if any element cannot be interpreted as a number.
///
/// # Examples
///
/// ```
/// use serde_json::json;
/// // single embedding
/// let v = json!([0.1, 0.2, 0.3]);
/// let parts = crate::providers::gemini::parse_embeddings(&v).unwrap();
/// assert!(!parts.is_empty());
///
/// // batched embeddings
/// let b = json!([[0.1, 0.2], [0.3, 0.4]]);
/// let parts_batched = crate::providers::gemini::parse_embeddings(&b).unwrap();
/// assert_eq!(parts_batched.len(), 2);
/// ```
pub fn parse_embeddings(value: &Value) -> Result<Parts, ChatError> {
    let mut parts = Parts::default();

    let array = value
        .as_array()
        .ok_or_else(|| ChatError::InvalidResponse("Embedding values not array".to_string()))?;

    if array.first().and_then(|v| v.as_array()).is_some() {
        for embedding in array {
            let inner = embedding.as_array().ok_or_else(|| {
                ChatError::InvalidResponse("Invalid batched embedding".to_string())
            })?;

            let vector: Vec<f32> = inner
                .iter()
                .map(|v| {
                    v.as_f64()
                        .ok_or_else(|| {
                            ChatError::InvalidResponse("Invalid embedding number".to_string())
                        })
                        .map(|n| n as f32)
                })
                .collect::<Result<Vec<_>, _>>()?;

            parts.push(parts::PartEnum::from_embeddings(Embeddings::from(vector)));
        }
    } else {
        let vector: Vec<f32> = array
            .iter()
            .map(|v| {
                v.as_f64()
                    .ok_or_else(|| {
                        ChatError::InvalidResponse("Invalid embedding number".to_string())
                    })
                    .map(|n| n as f32)
            })
            .collect::<Result<Vec<_>, _>>()?;

        parts.push(parts::PartEnum::from_embeddings(Embeddings::from(vector)));
    }
    Ok(parts)
}
