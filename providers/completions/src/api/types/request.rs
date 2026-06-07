use base64::{Engine as _, engine::general_purpose::STANDARD};
use chat_core::{
    error::ChatError,
    types::{
        messages::{
            Messages,
            content::{Content, RoleEnum},
            file::FileSource,
            parts::PartEnum,
        },
        options::ChatOptions,
        tools::ToolDeclarations,
    },
};
use schemars::Schema;
use serde::Serialize;
use serde_json::{Value, json};

#[derive(Debug, Serialize)]
pub struct CompletionsRequest {
    pub model: String,
    pub messages: Vec<Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Value>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<Value>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_options: Option<Value>,
}

pub struct CompletionsRequestConfig<'a> {
    pub model_name: &'a str,
    pub messages: &'a Messages,
    pub tool_declarations: Option<&'a dyn ToolDeclarations>,
    pub options: Option<&'a ChatOptions>,
    pub output_shape: Option<&'a Schema>,
}

impl CompletionsRequest {
    pub fn from_core(config: CompletionsRequestConfig<'_>) -> Result<Self, ChatError> {
        let CompletionsRequestConfig {
            model_name,
            messages,
            tool_declarations,
            options,
            output_shape,
        } = config;

        let mut req = Self {
            model: model_name.to_string(),
            messages: Vec::new(),
            temperature: None,
            top_p: None,
            max_tokens: None,
            tools: None,
            response_format: None,
            stream: None,
            stream_options: None,
        };

        if let Some(opts) = options {
            req.temperature = opts.temperature;
            req.top_p = opts.top_p;
            req.max_tokens = opts.max_tokens;
        }

        if let Some(schema) = output_shape {
            req.response_format = Some(json!({
                "type": "json_schema",
                "json_schema": {
                    "name": "structured_output",
                    "schema": schema,
                    "strict": false,
                }
            }));
        }

        if let Some(decls) = tool_declarations {
            let value = decls.json().map_err(|e| ChatError::Other(e.to_string()))?;
            if let Value::Array(arr) = value {
                let mut tools_list = Vec::with_capacity(arr.len());
                for decl in arr {
                    let name = decl
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string();
                    let description = decl
                        .get("description")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string();
                    let parameters = decl
                        .get("parameters")
                        .cloned()
                        .unwrap_or_else(|| json!({"type": "object"}));

                    tools_list.push(json!({
                        "type": "function",
                        "function": {
                            "name": name,
                            "description": description,
                            "parameters": parameters,
                        }
                    }));
                }
                if !tools_list.is_empty() {
                    req.tools = Some(tools_list);
                }
            }
        }

        for content in &messages.0 {
            push_content(content, &mut req.messages)?;
        }

        Ok(req)
    }
}

fn push_content(content: &Content, out: &mut Vec<Value>) -> Result<(), ChatError> {
    let role = match content.role {
        RoleEnum::User => "user",
        RoleEnum::Model => "assistant",
        RoleEnum::System => "system",
    };

    let mut content_parts: Vec<Value> = Vec::new();
    let mut tool_calls: Vec<Value> = Vec::new();
    let mut tool_results: Vec<Value> = Vec::new();

    for part in &content.parts.0 {
        match part {
            PartEnum::Text(t) => {
                content_parts.push(json!({"type": "text", "text": t.0}));
            }
            PartEnum::Reasoning(r) => {
                content_parts.push(json!({"type": "text", "text": r.text}));
            }
            PartEnum::Tool(tool) => {
                let (fc, maybe_fr) = tool.to_tuple();
                tool_calls.push(json!({
                    "id": fc.id.clone().map(String::from).unwrap_or_default(),
                    "type": "function",
                    "function": {
                        "name": fc.name,
                        "arguments": serde_json::to_string(&fc.arguments).unwrap_or_default(),
                    }
                }));
                if let Some(fr) = maybe_fr {
                    let output = if fr.result.is_string() {
                        fr.result.as_str().unwrap().to_string()
                    } else {
                        fr.result.to_string()
                    };
                    tool_results.push(json!({
                        "role": "tool",
                        "tool_call_id": fr.id.clone().map(String::from).unwrap_or_default(),
                        "content": output,
                    }));
                }
            }
            PartEnum::File(file) if file.is_image() => {
                let url = match &file.source {
                    FileSource::Url(u) => u.to_string(),
                    FileSource::Bytes(bytes) => {
                        let b64 = STANDARD.encode(bytes);
                        format!("data:{};base64,{}", file.mime, b64)
                    }
                };
                content_parts.push(json!({
                    "type": "image_url",
                    "image_url": {"url": url}
                }));
            }
            PartEnum::File(file) if file.is_audio() => {
                // Chat Completions carries audio only as inline base64 via
                // the `input_audio` content part — there is no URL form.
                let FileSource::Bytes(bytes) = &file.source else {
                    return Err(ChatError::Provider(format!(
                        "chat-completions cannot send audio by URL ({}): the Chat \
                         Completions `input_audio` part only accepts inline data — \
                         fetch the bytes and pass them via File::from_bytes",
                        file.mime
                    )));
                };
                content_parts.push(json!({
                    "type": "input_audio",
                    "input_audio": {
                        "data": STANDARD.encode(bytes),
                        "format": audio_format(file.mime.as_str()),
                    }
                }));
            }
            // Video, documents, and any other non-image/non-audio file:
            // the Chat Completions wire has no content part for these.
            // Fail loudly rather than silently dropping the attachment.
            PartEnum::File(file) => {
                return Err(ChatError::Provider(format!(
                    "chat-completions cannot represent a file part with mimetype {} \
                     in a Chat Completions request — only image and audio parts are \
                     supported on this wire",
                    file.mime
                )));
            }
            PartEnum::Structured(_) | PartEnum::Embeddings(_) => {}
        }
    }

    let message_emitted =
        !content_parts.is_empty() || !tool_calls.is_empty() || role == "assistant";

    if message_emitted {
        let mut message = serde_json::Map::new();
        message.insert("role".into(), json!(role));

        if content_parts.is_empty() {
            message.insert("content".into(), Value::Null);
        } else if content_parts.len() == 1
            && let Some(t) = content_parts[0].get("text").and_then(|v| v.as_str())
            && content_parts[0].get("type").and_then(|v| v.as_str()) == Some("text")
        {
            message.insert("content".into(), json!(t));
        } else {
            message.insert("content".into(), Value::Array(content_parts));
        }

        if !tool_calls.is_empty() {
            message.insert("tool_calls".into(), Value::Array(tool_calls));
        }

        out.push(Value::Object(message));
    }

    out.extend(tool_results);
    Ok(())
}

/// Map an `audio/*` mimetype to the bare format string the Chat
/// Completions `input_audio` part expects (e.g. `audio/mpeg` -> `mp3`).
/// Unknown subtypes are passed through verbatim so the server can accept
/// or reject them explicitly — never silently dropped.
fn audio_format(mime: &str) -> &str {
    match mime.strip_prefix("audio/").unwrap_or(mime) {
        "mpeg" | "mp3" => "mp3",
        "wav" | "wave" | "x-wav" => "wav",
        other => other,
    }
}

#[derive(Debug, Serialize)]
pub struct CompletionsEmbeddingRequest {
    pub model: String,
    pub input: Value,
}

impl CompletionsEmbeddingRequest {
    pub fn from_core(model_name: &str, messages: &Messages) -> Result<Self, ChatError> {
        let last_content = messages
            .0
            .last()
            .ok_or_else(|| ChatError::InvalidResponse("Sent empty content to embed".to_string()))?;

        let mut parts = Vec::new();
        for part in &last_content.parts.0 {
            match part {
                PartEnum::Text(t) => parts.push(json!(t.0)),
                PartEnum::Reasoning(r) => parts.push(json!(r.text)),
                _ => {}
            }
        }

        if parts.is_empty() {
            return Err(ChatError::InvalidResponse(
                "Sent empty content to embed".to_string(),
            ));
        }

        let input = if parts.len() == 1 {
            parts.pop().unwrap()
        } else {
            Value::Array(parts)
        };

        Ok(Self {
            model: model_name.to_string(),
            input,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chat_core::types::messages;

    #[test]
    fn user_message_serializes_to_string_content() {
        let msgs = messages::from_user(vec!["hello"]);
        let req = CompletionsRequest::from_core(CompletionsRequestConfig {
            model_name: "llama3",
            messages: &msgs,
            tool_declarations: None,
            options: None,
            output_shape: None,
        })
        .unwrap();

        let val = serde_json::to_value(&req).unwrap();
        assert_eq!(val["model"], "llama3");
        assert_eq!(val["messages"][0]["role"], "user");
        assert_eq!(val["messages"][0]["content"], "hello");
    }

    #[test]
    fn system_message_uses_system_role() {
        let mut msgs = messages::Messages::default();
        msgs.0
            .push(messages::content::from_system(vec!["you are helpful"]));
        msgs.0.push(messages::content::from_user(vec!["hi"]));
        let req = CompletionsRequest::from_core(CompletionsRequestConfig {
            model_name: "m",
            messages: &msgs,
            tool_declarations: None,
            options: None,
            output_shape: None,
        })
        .unwrap();

        let val = serde_json::to_value(&req).unwrap();
        assert_eq!(val["messages"][0]["role"], "system");
        assert_eq!(val["messages"][0]["content"], "you are helpful");
        assert_eq!(val["messages"][1]["role"], "user");
    }

    fn user_with_file(file: messages::file::File) -> messages::Messages {
        use messages::parts::PartEnum;
        let mut msgs = messages::Messages::default();
        let content = messages::content::Content {
            role: messages::content::RoleEnum::User,
            parts: messages::parts::Parts(vec![
                PartEnum::File(file),
                PartEnum::Text(messages::text::Text::new("transcribe this")),
            ]),
            complete_reason: messages::content::CompleteReasonEnum::None,
        };
        msgs.0.push(content);
        msgs
    }

    fn request_for(msgs: &messages::Messages) -> Result<CompletionsRequest, ChatError> {
        CompletionsRequest::from_core(CompletionsRequestConfig {
            model_name: "m",
            messages: msgs,
            tool_declarations: None,
            options: None,
            output_shape: None,
        })
    }

    #[test]
    fn audio_bytes_serialize_as_input_audio() {
        let file = messages::file::File::from_bytes_with_mime(b"RIFF...".to_vec(), "audio/wav");
        let req = request_for(&user_with_file(file)).unwrap();

        let val = serde_json::to_value(&req).unwrap();
        let parts = &val["messages"][0]["content"];
        assert_eq!(parts[0]["type"], "input_audio");
        assert_eq!(parts[0]["input_audio"]["format"], "wav");
        assert!(parts[0]["input_audio"]["data"].as_str().is_some());
        // The text part is still present alongside the audio.
        assert_eq!(parts[1]["type"], "text");
    }

    #[test]
    fn audio_mpeg_format_normalizes_to_mp3() {
        let file = messages::file::File::from_bytes_with_mime(b"ID3".to_vec(), "audio/mpeg");
        let req = request_for(&user_with_file(file)).unwrap();
        let val = serde_json::to_value(&req).unwrap();
        assert_eq!(
            val["messages"][0]["content"][0]["input_audio"]["format"],
            "mp3"
        );
    }

    #[test]
    fn audio_by_url_errors_loudly() {
        let file =
            messages::file::File::from_url("https://example.com/a.wav", Some("audio/wav")).unwrap();
        let err = request_for(&user_with_file(file)).unwrap_err();
        assert!(matches!(err, ChatError::Provider(_)));
        assert!(err.to_string().contains("audio"));
    }

    #[test]
    fn unrepresentable_file_errors_instead_of_dropping() {
        // A document (PDF) has no Chat Completions content part: the bug
        // report's core complaint was that this vanished silently.
        let file = messages::file::File::from_bytes_with_mime(b"%PDF".to_vec(), "application/pdf");
        let err = request_for(&user_with_file(file)).unwrap_err();
        assert!(matches!(err, ChatError::Provider(_)));
        assert!(err.to_string().contains("application/pdf"));
    }
}
