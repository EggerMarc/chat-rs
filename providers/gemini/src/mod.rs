mod code_execution;
mod google_maps;
mod google_search;
pub mod lib;

use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_json::{Map, Value, json};
use std::env;
use tools_rs::{FunctionCall, ToolCollection};

use crate::error::ChatError;
use crate::gemini::code_execution::CodeExecutionTool;
use crate::gemini::google_maps::GoogleMapsTool;
use crate::gemini::google_search::GoogleSearchTool;
use crate::gemini::lib::GeminiNativeTool;
use crate::traits::CompletionProvider;
use crate::types::messages::content::{CompleteReasonEnum, Content, RoleEnum};
use crate::types::messages::embeddings::Embeddings;
use crate::types::messages::file::File;
use crate::types::messages::parts::{PartEnum, Parts};
use crate::types::messages::text::Text;
use crate::types::metadata::Metadata;
use crate::types::metadata::usage::Usage;
use crate::types::{
    failure::ChatFailure, messages::Messages, options::ChatOptions, response::ChatResponse,
};

#[derive(Clone, Default)]
pub struct FunctionCallingConfig {
    pub mode: Option<String>,
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
            embeddings_config: None,
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
impl CompletionProvider for GeminiClient {
    async fn complete(
        &self,
        messages: &mut Messages,
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
    let role_str = match content.role {
        RoleEnum::User => "user",
        RoleEnum::Model => "model",
        RoleEnum::System => "user",
    };
    json!({ "role": role_str, "parts": parts })
}

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

            parts.push(PartEnum::from_embeddings(Embeddings::from(vector)));
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

        parts.push(PartEnum::from_embeddings(Embeddings::from(vector)));
    }
    Ok(parts)
}
