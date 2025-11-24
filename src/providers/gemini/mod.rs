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
use crate::messages::content::{CompleteReasonEnum, RoleEnum};
use crate::messages::parts::{PartEnum, Parts};

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
    fn default() -> Self {
        Self::new()
    }
}

impl GeminiBuilder {
    pub fn new() -> Self {
        Self {
            model_name: None,
            api_key: None,
            native_tools: Vec::new(),
            function_config: None,
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
        self.native_tools.push(Box::new(CodeExecutionTool));
        self
    }

    pub fn with_google_search(&mut self) -> &mut Self {
        self.native_tools.push(Box::new(GoogleSearchTool {
            dynamic_threshold: None,
        }));
        self
    }

    pub fn with_google_search_threshold(&mut self, threshold: f32) -> &mut Self {
        self.native_tools.push(Box::new(GoogleSearchTool {
            dynamic_threshold: Some(threshold),
        }));
        self
    }

    pub fn with_google_maps(&mut self, lat_lng: Option<(f32, f32)>, widget: bool) -> &mut Self {
        self.native_tools.push(Box::new(GoogleMapsTool {
            lat_lng,
            enable_widget: widget,
        }));
        self
    }

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
    async fn complete(
        &self,
        messages: &Messages,
        custom_tools: Option<&ToolCollection>,
        _options: Option<&ChatOptions>,
        structured_output: Option<&schemars::Schema>,
    ) -> Result<Content, ChatError> {
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
        )?;

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
                eprintln!("Gemini API Error: {}", err);
                Err(ChatError::Provider(err.without_url().to_string()))
            }
        }
    }
}

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
