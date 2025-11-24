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
/// "tools" array and "toolConfig" object structure.
fn build_tools_and_config(
    custom_tools: Option<&ToolCollection>,
    native_config: &GeminiToolConfig,
) -> Result<(Option<Value>, Option<Value>), ChatError> {
    let mut tools_array = Vec::new();
    let mut tool_config_map = serde_json::Map::new();

    // --- A. Handle Custom Function Definitions ---
    if let Some(ct) = custom_tools {
        let declarations = ct
            .json()
            .map_err(|err| ChatError::Other(format!("Tools-rs serialization error: {}", err)))?;
        // Gemini expects: { "functionDeclarations": [...] } inside the tools array
        tools_array.push(json!({ "functionDeclarations": declarations }));
    }

    // --- B. Handle Native Tools (Definitions & Configs) ---

    // 1. Code Execution
    if native_config.code_execution {
        tools_array.push(json!({ "codeExecution": {} }));
    }

    // 2. Google Search
    if let Some(ref search_conf) = native_config.google_search {
        // Definition
        tools_array.push(json!({ "googleSearch": {} }));

        // Configuration (Dynamic Threshold)
        if let Some(thresh) = search_conf.dynamic_threshold {
            tool_config_map.insert(
                "google_search_retrieval".to_string(),
                json!({
                    "dynamic_retrieval_config": {
                        "mode": "MODE_DYNAMIC",
                        "dynamic_threshold": thresh
                    }
                }),
            );
        }
    }

    // 3. Google Maps
    if let Some(ref maps_conf) = native_config.google_maps {
        // Definition (Widget flag lives here)
        // Only include fields that are explicitly true or set to avoid empty objects if not needed
        let mut maps_def = json!({});
        if maps_conf.enable_widget {
            maps_def["enableWidget"] = json!(true);
        }
        // Even if empty, the key must exist to enable the tool
        tools_array.push(json!({ "googleMaps": maps_def }));

        // Configuration (Location Bounds live here)
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

    // --- D. Construct Final JSON Values ---
    let final_tools = if tools_array.is_empty() {
        None
    } else {
        Some(Value::Array(tools_array))
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

    // --- Helper to create empty messages ---
    fn mock_messages() -> Messages {
        // Assuming Messages::new() exists or default is implemented
        Messages::default()
    }

    #[test]
    fn test_google_maps_full_configuration() {
        // GOAL: Verify the split between 'tools' (widget) and 'toolConfig' (lat_lng/grounding)

        // 1. Setup
        let mut config = GeminiToolConfig::default();
        config.google_maps = Some(GoogleMapsConfig {
            lat_lng: Some((37.7749, -122.4194)),
            enable_widget: true,
        });

        let messages = mock_messages();

        // 2. Act
        let body =
            build_request_body(&messages, None, None, &config).expect("Failed to build body");

        // 3. Assertions

        // A. Check Tools Array (Definition)
        let tools_arr = body["tools"]
            .as_array()
            .expect("'tools' should be an array");
        let maps_tool = tools_arr
            .iter()
            .find(|t| t.get("googleMaps").is_some())
            .expect("googleMaps definition missing in tools array");

        // Verify widget flag is inside the tool definition
        assert_eq!(
            maps_tool["googleMaps"]["enable_widget"], true,
            "enable_widget should be inside tools[].googleMaps"
        );

        // B. Check Tool Config (Grounding/Bounds)
        let tool_config = body["toolConfig"]
            .as_object()
            .expect("'toolConfig' should be an object");

        // The API key for configuration is "google_maps_grounding", NOT "googleMaps"
        let grounding_config = tool_config
            .get("google_maps_grounding")
            .expect("google_maps_grounding missing in toolConfig");

        let lat = grounding_config["retrieval_config"]["lat_lng"]["latitude"]
            .as_f64()
            .unwrap();
        let lng = grounding_config["retrieval_config"]["lat_lng"]["longitude"]
            .as_f64()
            .unwrap();

        assert!((lat - 37.7749).abs() < 0.0001);
        assert!((lng - -122.4194).abs() < 0.0001);
    }

    #[test]
    fn test_google_search_dynamic_threshold() {
        // GOAL: Verify dynamic retrieval config is placed in toolConfig

        // 1. Setup
        let mut config = GeminiToolConfig::default();
        config.google_search = Some(GoogleSearchConfig {
            dynamic_threshold: Some(0.65),
        });

        let messages = mock_messages();

        // 2. Act
        let body = build_request_body(&messages, None, None, &config).unwrap();

        // 3. Assertions

        // Check Definition
        let tools_arr = body["tools"].as_array().unwrap();
        assert!(tools_arr.iter().any(|t| t.get("googleSearch").is_some()));

        // Check Configuration
        let tool_config = body["toolConfig"].as_object().unwrap();
        let search_config = tool_config
            .get("google_search_retrieval")
            .expect("google_search_retrieval missing in toolConfig");

        assert_eq!(
            search_config["dynamic_retrieval_config"]["mode"],
            "MODE_DYNAMIC"
        );
        assert_eq!(
            search_config["dynamic_retrieval_config"]["dynamic_threshold"],
            0.65
        );
    }

    #[test]
    fn test_code_execution_simple() {
        // GOAL: Verify code execution is just a flag in tools, no toolConfig needed

        let mut config = GeminiToolConfig::default();
        config.code_execution = true;

        let body = build_request_body(&mock_messages(), None, None, &config).unwrap();

        let tools_arr = body["tools"].as_array().unwrap();
        assert!(tools_arr.iter().any(|t| t.get("codeExecution").is_some()));

        // Should not create toolConfig if only code_execution is present
        assert!(body.get("toolConfig").is_none());
    }

    #[test]
    fn test_function_calling_config() {
        // GOAL: Verify functionCallingConfig structure (mode and whitelist)

        let mut config = GeminiToolConfig::default();
        config.function_calling = Some(FunctionCallingConfig {
            mode: Some("ANY".to_string()),
            allowed_function_names: Some(vec!["my_func".to_string()]),
        });

        let body = build_request_body(&mock_messages(), None, None, &config).unwrap();

        let tool_config = body["toolConfig"].as_object().unwrap();
        let fc_config = tool_config.get("functionCallingConfig").unwrap();

        assert_eq!(fc_config["mode"], "ANY");
        assert_eq!(fc_config["allowedFunctionNames"][0], "my_func");
    }

    #[test]
    fn test_mixed_custom_and_native_tools() {
        // GOAL: Ensure 'functionDeclarations' (custom) and 'googleSearch' (native) coexist in 'tools'

        let mut config = GeminiToolConfig::default();
        config.google_search = Some(GoogleSearchConfig::default());

        // Mock a custom tool collection
        // Note: In a real test, you'd use a real ToolCollection with 1 tool.
        // Here we rely on build_tools_and_config logic handling the Option.
        // Assuming your ToolCollection::new() works:
        let mut custom_tools = ToolCollection::new();
        // Since we can't easily add a tool without defining definitions,
        // we'll simulate the behavior by inspecting how the code handles `Some`.
        // However, for this specific test file context, let's just ensure
        // the Native tool is added correctly even if custom_tools is passed (even if empty).

        let body =
            build_request_body(&mock_messages(), Some(&custom_tools), None, &config).unwrap();

        let tools_arr = body["tools"].as_array().unwrap();

        // Depending on implementation of ToolCollection::json(), it might return [] if empty.
        // But our code logic: `tools_array.push(json!({ "functionDeclarations": declarations }));`
        // So we expect at least 2 entries: 1 for functions, 1 for googleSearch

        assert!(tools_arr.len() >= 2); // 1 for custom declarations container, 1 for search

        let has_funcs = tools_arr
            .iter()
            .any(|t| t.get("functionDeclarations").is_some());
        let has_search = tools_arr.iter().any(|t| t.get("googleSearch").is_some());

        assert!(has_funcs, "functionDeclarations block missing");
        assert!(has_search, "googleSearch block missing");
    }

    #[test]
    fn test_structured_output_sanitization() {
        // GOAL: Verify schemas are stripped of fields Gemini rejects ($schema, additionalProperties)

        use schemars::JsonSchema;
        use serde::{Deserialize, Serialize};

        #[derive(JsonSchema, Serialize, Deserialize)]
        struct MyData {
            name: String,
            #[serde(rename = "age")]
            age_val: i32,
        }

        let schema = schemars::schema_for!(MyData);
        let config = GeminiToolConfig::default();

        let body = build_request_body(&mock_messages(), None, Some(&schema), &config).unwrap();

        let gen_config = body["generationConfig"].as_object().unwrap();

        assert_eq!(gen_config["responseMimeType"], "application/json");

        let response_schema = &gen_config["responseSchema"];

        // Verify forbidden fields are gone
        assert!(response_schema.get("$schema").is_none());
        assert!(response_schema.get("title").is_none());

        // Verify properties structure remains
        let props = response_schema["properties"].as_object().unwrap();
        assert!(props.contains_key("name"));
        assert!(props.contains_key("age"));

        // Verify nested forbidden fields (additionalProperties inside object)
        assert!(response_schema.get("additionalProperties").is_none());
    }
}
