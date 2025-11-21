use async_trait::async_trait;
use serde_json::{Value, json};
use std::env;
use tools_rs::{FunctionCall, ToolCollection};

// Assuming these imports exist in your project structure
use crate::core::lib::ChatOptions;
use crate::core::{
    lib::{ChatError, ChatProvider},
    messages::{Messages, content::Content},
};
use crate::messages::content::{CompleteReasonEnum, RoleEnum};
use crate::messages::parts::{PartEnum, Parts};
use crate::messages::text::Text;

pub struct GeminiClient {
    model_name: String,
    api_key: String,
}

impl GeminiClient {
    pub fn new(model_name: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let api_key = env::var("GEMINI_API_KEY")?;
        Ok(GeminiClient {
            model_name: model_name.to_string(),
            api_key,
        })
    }
}

#[async_trait]
impl ChatProvider for GeminiClient {
    /// Send the provided message history (and optional tools/schema) to the configured Gemini model and parse the response into a `Content`.
    ///
    /// The request body is constructed from `messages`, optional `tools`, and optional `structured_output` (a JSON Schema) and sent to Gemini's `generateContent` endpoint for the client's `model_name`. The returned `Content` contains the parsed parts, role, and completion reason extracted from Gemini's first candidate.
    ///
    /// # Returns
    ///
    /// `Ok(Content)` with the model's response parsed into parts, role, and completion reason on success; `Err(ChatError)` if the HTTP request, response reading, or JSON parsing fails or the provider returns an error.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use my_crate::providers::gemini::GeminiClient;
    /// # use my_crate::{Messages, ToolCollection, ChatOptions};
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = GeminiClient::new("models/my-gemini-model")?;
    /// let messages = Messages::default();
    /// let tools: Option<&ToolCollection> = None;
    /// let options: Option<&ChatOptions> = None;
    /// let result = client.complete(&messages, tools, options, None).await?;
    /// println!("Received parts: {:?}", result.parts);
    /// # Ok(()) }
    /// ```
    async fn complete(
        &self,
        messages: &Messages,
        tools: Option<&ToolCollection>,
        _options: Option<&ChatOptions>,
        structured_output: Option<&schemars::Schema>,
    ) -> Result<Content, ChatError> {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
            self.model_name
        );

        let body = build_request_body(messages, tools, structured_output)?;

        let req = reqwest::Client::new()
            .post(url)
            .header("Content-Type", "application/json")
            .header("x-goog-api-key", &self.api_key)
            .body(body.to_string());

        // Debugging: Print body to verify schema structure
        // println!("Gemini Request Body: {}", body);
        println!("Body: {:#?}", body);
        let res = req
            .send()
            .await
            .map_err(|e| ChatError::Provider(e.to_string()))?;

        match res.error_for_status() {
            Ok(data) => {
                let text = data
                    .text()
                    .await
                    .map_err(|e| ChatError::Provider(e.to_string()))?;

                let json: Value = serde_json::from_str(&text)
                    .map_err(|e| ChatError::InvalidResponse(e.to_string()))?;

                let content = parse_gemini_response(&json)?;
                Ok(content)
            }
            Err(err) => {
                // It's often helpful to see the error body from Google
                println!("Error requesting completion: {}", err);
                Err(ChatError::Provider(err.without_url().to_string()))
            }
        }
    }
}

/// Builds the JSON request body to send to the Gemini `generateContent` endpoint.
///
/// If `structured_output` is provided, the schema is serialized, sanitized for Gemini (removing
/// fields Gemini rejects) and embedded under `generationConfig` with `responseMimeType: "application/json"`.
/// Includes optional `system_instruction`, `contents`, and `tools` fields when those are present
/// in `messages` and `tools`.
///
/// # Examples
///
/// ```
/// // Assumes `Messages::default()` and other types are available in the current crate.
/// let messages = Messages::default();
/// let body = build_request_body(&messages, None, None).unwrap();
/// // The produced body is a JSON object suitable for Gemini; at minimum it will be an object.
/// assert!(body.is_object());
/// ```
fn build_request_body(
    messages: &Messages,
    tools: Option<&ToolCollection>,
    structured_output: Option<&schemars::Schema>,
) -> Result<Value, ChatError> {
    let mut body = json!({});

    // 1. Handle Structured Output / JSON Schema
    if let Some(schema) = structured_output {
        // Gemini requires specific JSON Schema format (no $schema, no titles usually)
        let mut clean_schema = serde_json::to_value(schema)
            .map_err(|e| ChatError::Other(format!("Schema serialization error: {}", e)))?;

        sanitize_schema_for_gemini(&mut clean_schema);

        body["generationConfig"] = json!({
            "responseMimeType": "application/json",
            "responseSchema": clean_schema
        });
    }

    // 2. System Instructions
    if let Some(system) = build_system_instruction(messages) {
        body["system_instruction"] = system;
    }

    // 3. Message History
    if let Some(contents) = build_contents(messages) {
        body["contents"] = contents;
    }

    // 4. Tools
    if let Some(tools_value) = build_tools(tools)? {
        body["tools"] = tools_value;
        // Optional: Add tool_config if you need to force function calling
        // body["toolConfig"] = json!({ "functionCallingConfig": { "mode": "AUTO" } });
    }

    Ok(body)
}

/// Remove fields that Gemini's API rejects from a JSON Schema, in-place.
///
/// This function walks `schema` recursively and removes the keys: `$schema`, `title`,
/// `$id`, `additionalProperties`, and `definitions` from any JSON object it encounters.
/// It mutates the provided `Value` directly and descends into nested objects and arrays.
///
/// # Examples
///
/// ```
/// use serde_json::json;
/// let mut schema = json!({
///     "$schema": "http://example",
///     "title": "T",
///     "properties": {
///         "x": { "$id": "id", "type": "string", "additionalProperties": true }
///     },
///     "definitions": { "A": { "type": "number" } }
/// });
///
/// sanitize_schema_for_gemini(&mut schema);
///
/// assert!(!schema.as_object().unwrap().contains_key("$schema"));
/// assert!(!schema.as_object().unwrap().contains_key("title"));
/// assert!(!schema.as_object().unwrap().contains_key("definitions"));
/// let props = &schema["properties"]["x"];
/// assert!(!props.as_object().unwrap().contains_key("$id"));
/// assert!(!props.as_object().unwrap().contains_key("additionalProperties"));
/// ```
fn sanitize_schema_for_gemini(schema: &mut Value) {
    if let Value::Object(map) = schema {
        map.remove("$schema");
        map.remove("title");
        map.remove("$id");
        map.remove("additionalProperties"); // Crucial: Gemini rejects this
        map.remove("definitions");
        // Gemini prefers "type" to be present.
        // schemars sometimes relies on implicit types or $ref which Gemini handles okay,
        // but stripping metadata is the most important part.

        for (_, v) in map {
            sanitize_schema_for_gemini(v);
        }
    } else if let Value::Array(arr) = schema {
        for v in arr {
            sanitize_schema_for_gemini(v);
        }
    }
}

/// Constructs a Gemini-formatted system instruction object from the provided messages.
///
/// Produces a JSON object with a "parts" array containing the converted parts from all messages whose role is `System`. If no system-role parts are present, nothing is produced.
///
/// # Returns
///
/// `Some(Value)` containing an object of the form `{"parts": [...]}` when at least one system part exists, `None` otherwise.
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

/// Convert a Messages history into the Gemini `contents` JSON array used in a generateContent request.
///
/// Omits system-role messages. Non-function parts from user/model messages are serialized into message
/// objects that retain their role; function response parts are grouped into one or more message objects
/// with role `"function"`.
///
/// # Returns
///
/// `Some(Value::Array(...))` containing serialized message objects when there is at least one non-system
/// message, `None` when the input contains only system messages or is empty.
///
/// # Examples
///
/// ```no_run
/// let maybe_contents = build_contents(&messages);
/// if let Some(contents) = maybe_contents {
///     // `contents` is a `serde_json::Value::Array` ready to be inserted into the Gemini request body
/// }
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

        // Standard User/Model text/call parts
        if !other_parts.is_empty() {
            let parts: Vec<Value> = other_parts.iter().map(|p| part_to_gemini(p)).collect();
            contents.push(content_to_gemini_with_parts(content, parts));
        }

        // Function Responses must be sent with role "function"
        // Gemini expects one message per function response usually,
        // but allows multiple parts if multiple tools were called in parallel.
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

/// Builds the optional Gemini `tools` JSON fragment from a ToolCollection.
///
/// If `tools` is `Some`, serializes the tool declarations and returns a JSON value formatted
/// for Gemini (an array containing an object with a `functionDeclarations` field).
/// If `tools` is `None`, returns `Ok(None)`.
///
/// Serialization failures are returned as `ChatError::Other`.
///
/// # Examples
///
/// ```
/// # use serde_json::Value;
/// # // assume ToolCollection and ChatError are in scope for the doctest environment
/// let none = build_tools(None).unwrap();
/// assert!(none.is_none());
/// ```
fn build_tools(tools: Option<&ToolCollection>) -> Result<Option<Value>, ChatError> {
    match tools {
        Some(t) => {
            let declarations = t.json().map_err(|err| {
                ChatError::Other(format!("Tools-rs serialization error: {}", err))
            })?;
            // Gemini expects: { "function_declarations": [...] }
            Ok(Some(json!([{ "functionDeclarations": declarations }])))
        }
        None => Ok(None),
    }
}

/// Converts a `Content` and its Gemini `parts` into a Gemini-formatted message object.
///
/// The returned JSON object has `role` set to either `"user"` or `"model"` based on
/// `content.role`, and includes the provided `parts`. System and function roles are
/// represented as `"user"`.
///
/// # Examples
///
/// ```
/// use serde_json::json;
///
/// // assuming RoleEnum and Content are in scope
/// let content = Content { role: RoleEnum::User, parts: vec![] };
/// let parts = vec![json!({"text": "hello"})];
/// let msg = content_to_gemini_with_parts(&content, parts);
/// assert_eq!(msg["role"], "user");
/// assert_eq!(msg["parts"][0]["text"], "hello");
/// ```
fn content_to_gemini_with_parts(content: &Content, parts: Vec<Value>) -> Value {
    match content.role {
        RoleEnum::User => json!({ "role": "user", "parts": parts }),
        RoleEnum::Model => json!({ "role": "model", "parts": parts }),
        // System handled separately, Function handled separately
        _ => json!({ "role": "user", "parts": parts }),
    }
}

/// Convert a PartEnum into the JSON structure expected by the Gemini API.
///
/// This produces a serde_json::Value representing a single Gemini "part" for inclusion
/// in request/response content. Text and reasoning parts become `{ "text": ... }`;
/// function calls become `{ "functionCall": { "name": ..., "args": ... } }`;
/// function responses become `{ "functionResponse": { "name": ..., "response": { ... } } }`
/// and non-handled variants produce an empty text part.
///
/// Returns a JSON `Value` formatted according to Gemini's messaging conventions.
///
/// # Examples
///
/// ```no_run
/// # use serde_json::json;
/// # use your_crate::providers::gemini::{part_to_gemini, PartEnum, Text, FunctionCall, FunctionResponse};
/// let text_part = PartEnum::Text(Text::new("hello"));
/// let json_val = part_to_gemini(&text_part);
/// assert_eq!(json_val, json!({ "text": "hello" }));
///
/// let fc = FunctionCall { name: "sum".to_string(), arguments: json!({ "a": 1, "b": 2 }) };
/// let call_part = PartEnum::FunctionCall(fc);
/// let json_call = part_to_gemini(&call_part);
/// assert_eq!(json_call["functionCall"]["name"], "sum");
/// ```
fn part_to_gemini(part: &PartEnum) -> Value {
    match part {
        PartEnum::Text(text) => json!({ "text": text }),

        // Reasoning is not yet a standard part in Gemini public API, treating as text or ignoring
        PartEnum::Reasoning(text) => json!({ "text": text }),

        PartEnum::FunctionCall(fc) => json!({
            "functionCall": {
                "name": fc.name,
                "args": fc.arguments
            }
        }),

        PartEnum::FunctionResponse(fr) => {
            // IMPORTANT: Gemini requires 'response' to be a JSON Object.
            // If fr.result is a string, wrap it. If it's already an object, use it.
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

        _ => json!({ "text": "" }),
    }
}

// ... Parsing logic remains mostly the same ...

/// Extracts a Content value from a Gemini JSON response by reading the first candidate's content.
///
/// Parses the first entry of the `candidates` array, extracts its `content`, converts that into
/// message parts, determines the sender role, and maps the candidate finish reason into a
/// completion reason to produce a `Content`.
///
/// # Returns
///
/// `Content` constructed from the first candidate's `content` (parts, role, and completion reason).
/// Returns `Err(ChatError::InvalidResponse(...))` if `candidates` or `content` are missing, and
/// propagates any parsing errors produced by the helper parsers.
///
/// # Examples
///
/// ```
/// use serde_json::json;
/// // minimal synthetic Gemini-like response
/// let resp = json!({
///     "candidates": [
///         {
///             "content": {
///                 "role": "user",
///                 "parts": [{ "text": "Hello" }]
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

    Ok(Content {
        parts,
        role,
        complete_reason,
    })
}

/// Parse a Gemini `content` JSON object into a collection of message parts.
///
/// This reads the optional `"parts"` array from `content_json` and converts each item
/// into the corresponding `PartEnum` (supports `"text"` and `"functionCall"` entries).
///
/// # Errors
///
/// Returns `ChatError::InvalidResponse` when a `"functionCall"` item is present but
/// is missing a required `"name"` or `"args"` field.
///
/// # Examples
///
/// ```
/// use serde_json::json;
///
/// let content = json!({
///     "parts": [
///         { "text": "hello" },
///         { "functionCall": { "name": "doThing", "args": { "x": 1 } } }
///     ]
/// });
///
/// let parts = parse_parts(&content).expect("parsed parts");
/// assert!(!parts.is_empty());
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

/// Determine the message role encoded in a Gemini content object.
///
/// The function reads the `role` string field from `content_json` (defaults to `"model"` if missing)
/// and maps it to the corresponding `RoleEnum`: `"user"` → `RoleEnum::User`, `"system"` → `RoleEnum::System`,
/// `"function"` → `RoleEnum::Model`, any other value → `RoleEnum::Model`.
///
/// # Examples
///
/// ```
/// use serde_json::json;
/// let v = json!({ "role": "user" });
/// assert_eq!(parse_role(&v), RoleEnum::User);
/// let v2 = json!({});
/// assert_eq!(parse_role(&v2), RoleEnum::Model);
/// ```
fn parse_role(content_json: &Value) -> RoleEnum {
    match content_json
        .get("role")
        .and_then(|v| v.as_str())
        .unwrap_or("model")
    {
        "user" => RoleEnum::User,
        "system" => RoleEnum::System,
        "function" => RoleEnum::Model, // Function output comes from user/tool, but responses are model
        _ => RoleEnum::Model,
    }
}

/// Maps a Gemini candidate's `"finishReason"` field to a completion reason.
///
/// Returns the corresponding `CompleteReasonEnum` for the candidate's `"finishReason"`:
/// - `"STOP"` -> `CompleteReasonEnum::Stop`
/// - `"MAX_TOKENS"` -> `CompleteReasonEnum::MaxTokens`
/// - `"SAFETY"`, `"RECITATION"`, `"OTHER"` -> `CompleteReasonEnum::ContentFilter`
/// - missing or any other value -> `CompleteReasonEnum::None`
///
/// # Examples
///
/// ```
/// use serde_json::json;
///
/// let candidate = json!({ "finishReason": "STOP" });
/// assert_eq!(crate::parse_finish_reason(&candidate), crate::CompleteReasonEnum::Stop);
///
/// let unknown = json!({});
/// assert_eq!(crate::parse_finish_reason(&unknown), crate::CompleteReasonEnum::None);
/// ```
fn parse_finish_reason(candidate: &Value) -> CompleteReasonEnum {
    match candidate
        .get("finishReason")
        .and_then(|v| v.as_str())
        .unwrap_or("")
    {
        "STOP" => CompleteReasonEnum::Stop,
        "MAX_TOKENS" => CompleteReasonEnum::MaxTokens,
        "SAFETY" | "RECITATION" | "OTHER" => CompleteReasonEnum::ContentFilter,
        _ => CompleteReasonEnum::None,
    }
}