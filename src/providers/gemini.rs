use async_trait::async_trait;
use serde::Serialize;
use serde_json::{Value, json};
use std::env;
use tools_rs::{FunctionCall, ToolCollection};

// Keep your existing crate imports
use crate::core::lib::ChatOptions;
use crate::core::{
    lib::{ChatError, ChatProvider},
    messages::{Messages, content::Content, text::Text},
};
use crate::messages::content::{CompleteReasonEnum, RoleEnum};
use crate::messages::parts::{PartEnum, Parts};

// =========================================================================
//  Configuration Structs
// =========================================================================

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GoogleMapsConfig {
    pub lat_lng: Option<(f32, f32)>,
    pub enable_widget: bool,
}

#[derive(Clone, Default)]
pub struct GoogleSearchConfig {
    pub dynamic_threshold: Option<f32>, // 0.0 to 1.0
}

#[derive(Clone, Default)]
pub struct FunctionCallingConfig {
    pub mode: Option<String>, // "AUTO", "ANY", "NONE"
    pub allowed_function_names: Option<Vec<String>>,
}

/// Holds configuration for all Native Tools and specific behaviors.
#[derive(Clone, Default)]
pub struct GeminiToolConfig {
    pub code_execution: bool,
    pub google_search: Option<GoogleSearchConfig>,
    pub google_maps: Option<GoogleMapsConfig>,
    pub function_calling: Option<FunctionCallingConfig>,
}

// =========================================================================
//  Builder
// =========================================================================

pub struct GeminiBuilder {
    model_name: Option<String>,
    api_key: Option<String>,
    tool_config: GeminiToolConfig,
}

impl Default for GeminiBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl GeminiBuilder {
    pub fn new() -> Self {
        Self {
            model_name: None,
            api_key: None,
            tool_config: GeminiToolConfig::default(),
        }
    }

    pub fn with_model(&mut self, model_name: String) -> &mut Self {
        self.model_name = Some(model_name);
        self
    }

    pub fn with_api_key(&mut self, api_key: String) -> &mut Self {
        self.api_key = Some(api_key);
        self
    }

    pub fn with_code_execution(&mut self) -> &mut Self {
        self.tool_config.code_execution = true;
        self
    }

    /// Enable Google Search with default dynamic settings
    pub fn with_google_search(&mut self) -> &mut Self {
        self.tool_config.google_search = Some(GoogleSearchConfig::default());
        self
    }

    /// Enable Google Search with specific dynamic threshold
    pub fn with_google_search_threshold(&mut self, threshold: f32) -> &mut Self {
        self.tool_config.google_search = Some(GoogleSearchConfig {
            dynamic_threshold: Some(threshold),
        });
        self
    }

    pub fn with_google_maps(&mut self, lat_lng: Option<(f32, f32)>, widget: bool) -> &mut Self {
        self.tool_config.google_maps = Some(GoogleMapsConfig {
            lat_lng,
            enable_widget: widget,
        });
        self
    }

    pub fn with_function_calling_mode(
        &mut self,
        mode: &str,
        allowed: Option<Vec<String>>,
    ) -> &mut Self {
        self.tool_config.function_calling = Some(FunctionCallingConfig {
            mode: Some(mode.to_string()),
            allowed_function_names: allowed,
        });
        self
    }

    pub fn build(&self) -> GeminiClient {
        GeminiClient {
            model_name: self
                .model_name
                .clone()
                .unwrap_or_else(|| "gemini-2.5-flash".to_string()),
            api_key: self.api_key.clone().unwrap_or_else(|| {
                env::var("GEMINI_API_KEY").expect("Failed to find GEMINI_API_KEY from .env")
            }),
            tool_config: self.tool_config.clone(),
        }
    }
}

// =========================================================================
//  Client
// =========================================================================

pub struct GeminiClient {
    model_name: String,
    api_key: String,
    tool_config: GeminiToolConfig,
}

impl GeminiClient {
    pub fn new(model_name: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let api_key = env::var("GEMINI_API_KEY")?;
        Ok(GeminiClient {
            model_name: model_name.to_string(),
            api_key,
            tool_config: GeminiToolConfig::default(),
        })
    }
}

#[async_trait]
impl ChatProvider for GeminiClient {
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

        // Build the entire body here. No patching needed afterwards.
        let body = build_request_body(messages, tools, structured_output, &self.tool_config)?;

        println!("Body: {:#?}", body);

        let req = reqwest::Client::new()
            .post(url)
            .header("Content-Type", "application/json")
            .header("x-goog-api-key", &self.api_key)
            .body(body.to_string());

        let res = req.send().await.unwrap();
        //.map_err(|e| ChatError::Provider(e.to_string()))?;

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
                println!("Error requesting completion: {}", err);
                Err(ChatError::Provider(err.without_url().to_string()))
            }
        }
    }
}

// =========================================================================
//  Request Building Logic
// =========================================================================

fn build_request_body(
    messages: &Messages,
    tools: Option<&ToolCollection>,
    structured_output: Option<&schemars::Schema>,
    native_config: &GeminiToolConfig,
) -> Result<Value, ChatError> {
    let mut body = json!({});

    // 1. Structured Output (Generation Config)
    if let Some(schema) = structured_output {
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

    // 3. Contents (History)
    if let Some(contents) = build_contents(messages) {
        body["contents"] = contents;
    }

    // 4. Tools (Definitions) & Tool Config (Runtime Args)
    // We process these together to ensure native tools are added to the list
    let (tools_json, config_json) = build_tools_and_config(tools, native_config)?;

    if let Some(t) = tools_json {
        body["tools"] = t;
    }
    if let Some(c) = config_json {
        body["toolConfig"] = c;
    }

    Ok(body)
}

/// Combines Custom Tools (via ToolCollection) and Native Tools into the correct
/// "tools" array structure (List of separate tool objects).
fn build_tools_and_config(
    custom_tools: Option<&ToolCollection>,
    native_config: &GeminiToolConfig,
) -> Result<(Option<Value>, Option<Value>), ChatError> {
    // Change 1: We use a Vector of distinct Value objects, not one single object.
    let mut tools_list = Vec::new();
    let mut tool_config_map = serde_json::Map::new();

    // --- A. Handle Custom Function Definitions ---
    if let Some(ct) = custom_tools {
        let declarations = ct
            .json()
            .map_err(|err| ChatError::Other(format!("Tools-rs serialization error: {}", err)))?;

        // Pushing a distinct object for functions
        tools_list.push(json!({
            "functionDeclarations": declarations
        }));
    }

    // --- B. Handle Native Tools ---

    // 1. Code Execution
    if native_config.code_execution {
        tools_list.push(json!({ "codeExecution": {} }));
    }

    // 2. Google Search
    if let Some(ref search_conf) = native_config.google_search {
        // Change 2: Use "googleSearch" (CamelCase) to match standard API & Config keys
        tools_list.push(json!({ "googleSearch": {} }));

        // Configuration
        if let Some(thresh) = search_conf.dynamic_threshold {
            tool_config_map.insert(
                "googleSearchRetrieval".to_string(),
                json!({
                    "dynamicRetrievalConfig": {
                        "mode": "MODE_DYNAMIC",
                        "dynamicThreshold": thresh
                    }
                }),
            );
        }
    }

    // 3. Google Maps
    if let Some(ref maps_conf) = native_config.google_maps {
        let mut maps_def = json!({});
        if maps_conf.enable_widget {
            maps_def["enableWidget"] = json!(true);
        }

        // Pushing a distinct object for Maps
        tools_list.push(json!({ "googleMaps": maps_def }));

        // Configuration
        if let Some((lat, lng)) = maps_conf.lat_lng {
            tool_config_map.insert(
                "retrievalConfig".to_string(),
                json!({
                        "latLng": {
                            "latitude": lat,
                            "longitude": lng
                        }
                }),
            );
        }
    }

    // --- C. Handle Function Calling Mode (Config Only) ---
    if let Some(ref fc_conf) = native_config.function_calling {
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

    // --- D. Finalize ---
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

// =========================================================================
//  Helpers (Sanitization, Parsing, Mapping)
// =========================================================================

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

fn content_to_gemini_with_parts(content: &Content, parts: Vec<Value>) -> Value {
    match content.role {
        RoleEnum::User => json!({ "role": "user", "parts": parts }),
        RoleEnum::Model => json!({ "role": "model", "parts": parts }),
        _ => json!({ "role": "user", "parts": parts }),
    }
}

fn part_to_gemini(part: &PartEnum) -> Value {
    match part {
        PartEnum::Text(text) => json!({ "text": text }),
        PartEnum::Reasoning(text) => json!({ "text": text }),
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
        _ => json!({ "text": "" }),
    }
}

// ... existing parsing logic ...
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
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn mock_messages() -> Messages {
        Messages::default()
    }

    #[test]
    fn test_google_search_camel_case_keys() {
        // GOAL: Ensure we generate "googleSearch" (not google_search)
        // and "googleSearchRetrieval" (not google_search_retrieval)

        let mut config = GeminiToolConfig::default();
        config.google_search = Some(GoogleSearchConfig {
            dynamic_threshold: Some(0.3),
        });

        let body = build_request_body(&mock_messages(), None, None, &config).unwrap();

        // 1. Check Tool Definition
        let tools_arr = body["tools"].as_array().unwrap();
        let tool_obj = &tools_arr[0];

        // This was failing before:
        assert!(
            tool_obj.get("googleSearch").is_some(),
            "Should contain 'googleSearch' key"
        );
        assert!(
            tool_obj.get("google_search").is_none(),
            "Should NOT contain 'google_search' key"
        );

        // 2. Check Tool Config
        let tool_config = body["toolConfig"].as_object().unwrap();

        // This was failing before:
        assert!(
            tool_config.contains_key("googleSearchRetrieval"),
            "Config should use camelCase 'googleSearchRetrieval'"
        );

        let retrieval = &tool_config["googleSearchRetrieval"];
        assert_eq!(retrieval["dynamicRetrievalConfig"]["dynamicThreshold"], 0.3);
    }

    #[test]
    fn test_google_maps_nesting() {
        // GOAL: Ensure retrievalConfig is nested inside googleMapsGrounding

        let mut config = GeminiToolConfig::default();
        config.google_maps = Some(GoogleMapsConfig {
            lat_lng: Some((10.0, 20.0)),
            enable_widget: false,
        });

        let body = build_request_body(&mock_messages(), None, None, &config).unwrap();

        let tool_config = body["toolConfig"].as_object().unwrap();

        // This checks for the hierarchy that was missing in your JSON
        let maps_grounding = tool_config
            .get("googleMapsGrounding")
            .expect("Top level key 'googleMapsGrounding' missing");

        let retrieval = maps_grounding
            .get("retrievalConfig")
            .expect("retrievalConfig should be nested inside googleMapsGrounding");

        assert_eq!(retrieval["latLng"]["latitude"], 10.0);
    }

    #[test]
    fn test_single_tool_object_structure() {
        // GOAL: Verify that even if we enable multiple tools, they merge nicely
        // (This style is often preferred by the API, though arrays of objects are allowed)

        let mut config = GeminiToolConfig::default();
        config.code_execution = true;
        config.google_search = Some(GoogleSearchConfig::default());
        config.google_maps = Some(GoogleMapsConfig {
            lat_lng: None,
            enable_widget: true,
        });

        let body = build_request_body(&mock_messages(), None, None, &config).unwrap();

        let tools_arr = body["tools"].as_array().unwrap();
        assert_eq!(
            tools_arr.len(),
            1,
            "Should merge all tools into one object for cleanliness"
        );

        let tool_obj = &tools_arr[0];
        assert!(tool_obj.get("codeExecution").is_some());
        assert!(tool_obj.get("googleSearch").is_some());
        assert!(tool_obj.get("googleMaps").is_some());
    }
}

