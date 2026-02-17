mod code_execution;
mod google_maps;
mod google_search;
pub mod lib;

use async_trait::async_trait;
use serde_json::{Value, json};
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
use crate::messages::parts::{PartEnum, Parts};
use crate::metadata::Metadata;
use crate::metadata::usage::Usage;

#[derive(Clone, Default)]
pub struct FunctionCallingConfig {
    pub mode: Option<String>, // "AUTO", "ANY", "NONE"
    pub allowed_function_names: Option<Vec<String>>,
}

pub struct GeminiBuilder {
    model_name: Option<String>,
    api_key: Option<String>,
    native_tools: Vec<Box<dyn GeminiNativeTool>>,
    function_config: Option<FunctionCallingConfig>,
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
    /// Creates a new GeminiBuilder initialized with empty/default fields.
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
        }
    }
}

pub struct GeminiClient {
    model_name: String,
    api_key: String,
    native_tools: Vec<Box<dyn GeminiNativeTool>>,
    function_config: Option<FunctionCallingConfig>,
}

impl GeminiClient {
    /// Creates a GeminiClient for the given model name using the `GEMINI_API_KEY` from the environment.
    ///
    /// The function reads the `GEMINI_API_KEY` environment variable and returns a configured client
    /// with no native tools and no function-calling configuration. Returns an `Err` if the
    /// `GEMINI_API_KEY` environment variable is not set or cannot be read.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::env;
    /// // In tests set the env var before calling `new`.
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
        })
    }
}

#[async_trait]
impl ChatProvider for GeminiClient {
    /// Send messages to the Gemini (Google Generative Language) model and parse its response into a `Content`.
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
    /// `Content` parsed from the model's response on success; returns a `ChatError` if the request, response parsing, or API call fails.
    async fn complete(
        &self,
        messages: &Messages,
        custom_tools: Option<&ToolCollection>,
        _options: Option<&ChatOptions>,
        structured_output: Option<&schemars::Schema>,
    ) -> Result<ChatResponse, ChatFailure> {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
            self.model_name
        );

        let body = build_request_body(
            messages,
            custom_tools,
            structured_output,
            &self.native_tools,
            self.function_config.as_ref(),
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

/// Builds the JSON request body for the Gemini Generative Language API from the provided
/// messages, optional custom tools, optional structured output schema, native tools, and
/// optional function-calling configuration.
///
/// The returned JSON contains any of: a sanitized `generationConfig` (when `structured_output` is
/// provided), `system_instruction` (from system-role messages), `contents` (from non-system
/// messages), and optional `tools` and `toolConfig` fields assembled from `custom_tools`,
/// `native_tools`, and `function_config`.
///
/// # Returns
///
/// A `serde_json::Value` representing the assembled request body, or a `ChatError` if schema
/// serialization or tool/config assembly fails.
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
/// // let body = build_request_body(&messages, custom_tools, schema, &native_tools, function_config)?;
/// ```
fn build_request_body(
    messages: &Messages,
    custom_tools: Option<&ToolCollection>,
    structured_output: Option<&schemars::Schema>,
    native_tools: &[Box<dyn GeminiNativeTool>],
    function_config: Option<&FunctionCallingConfig>,
) -> Result<Value, ChatError> {
    validate_combinations(custom_tools, structured_output, native_tools);

    let mut body = json!({});

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

/// Convert an internal PartEnum into the JSON representation expected by the Gemini API.
///
/// Maps PartEnum variants to the corresponding Gemini part object:
/// - `Text` and `Reasoning` become `{"text": "..."}`
/// - `FunctionCall` becomes `{"functionCall": {"name": ..., "args": ...}}`
/// - `FunctionResponse` becomes `{"functionResponse": {"name": ..., "response": ...}}`
/// - Any other variant becomes `{"text": ""}`
///
/// # Examples
///
/// ```
/// use serde_json::Value;
/// // assume PartEnum::Text exists and is constructible
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
        _ => json!({ "text": ""}),
    }
}

/// Parse a Gemini API response JSON into the crate's `Content` representation.
///
/// The function extracts the first candidate from the response, reads its `content` object,
/// converts its parts into internal `Part` values, and maps the role and finish reason into
/// `RoleEnum` and `CompleteReasonEnum`. Returns an error when the response does not contain
/// an expected `candidates[0].content` structure or when parts parsing fails.
///
/// # Returns
///
/// `Content` containing parsed `parts`, `role`, and `complete_reason`.
///
/// # Examples
///
/// ```
/// use serde_json::json;
/// // Construct a minimal Gemini-like response containing one candidate with content and a text part.
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
/// // Expect one parsed part and a model role
/// assert_eq!(content.parts.len(), 1);
/// ```
fn parse_gemini_response(json: &Value) -> Result<Content, ChatError> {
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

/// Maps a Gemini candidate's "finishReason" string to the corresponding CompleteReasonEnum.
///
/// This inspects the candidate object's `finishReason` field and converts known values:
/// - `"STOP"` -> `CompleteReasonEnum::Stop`
/// - `"MAX_TOKENS"` -> `CompleteReasonEnum::MaxTokens`
/// - `"SAFETY"`, `"RECITATION"`, or `"OTHER"` -> `CompleteReasonEnum::ContentFilter`
/// Any missing or unrecognized value yields `CompleteReasonEnum::None`.
///
/// # Parameters
///
/// - `candidate`: JSON object representing a Gemini candidate; the function reads its `finishReason` string.
///
/// # Returns
///
/// `CompleteReasonEnum` matching the candidate's `finishReason`, or `CompleteReasonEnum::None` if absent or unrecognized.
///
/// # Examples
///
/// ```
/// use serde_json::json;
///
/// let c = json!({ "finishReason": "STOP" });
/// assert_eq!(parse_finish_reason(&c), CompleteReasonEnum::Stop);
///
/// let c2 = json!({ "finishReason": "UNKNOWN" });
/// assert_eq!(parse_finish_reason(&c2), CompleteReasonEnum::None);
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
