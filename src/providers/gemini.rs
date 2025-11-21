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

/// Recursively removes fields that Gemini's API rejects (like $schema, title, etc)
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

fn content_to_gemini_with_parts(content: &Content, parts: Vec<Value>) -> Value {
    match content.role {
        RoleEnum::User => json!({ "role": "user", "parts": parts }),
        RoleEnum::Model => json!({ "role": "model", "parts": parts }),
        // System handled separately, Function handled separately
        _ => json!({ "role": "user", "parts": parts }),
    }
}

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
        "function" => RoleEnum::Model, // Function output comes from user/tool, but responses are model
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
