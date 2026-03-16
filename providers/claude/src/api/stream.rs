#[cfg(feature = "stream")]
mod inner {
    use crate::api::types::error::handle_claude_error;
    use crate::api::types::request::ClaudeRequest;
    use crate::api::types::response::{ClaudeContentBlock, ClaudeUsage};
    use crate::client::ClaudeClient;
    use async_stream::stream;
    use chat_core::error::ChatError;
    use chat_core::traits::StreamProvider;
    use chat_core::types::messages::Messages;
    use chat_core::types::messages::content::{CompleteReasonEnum, Content, RoleEnum};
    use chat_core::types::messages::parts::{PartEnum, Parts};
    use chat_core::types::messages::text::Text;
    use chat_core::types::metadata::Metadata;
    use chat_core::types::metadata::usage::Usage;
    use chat_core::types::options::ChatOptions;
    use chat_core::types::response::{ChatResponse, StreamEvent};
    use futures::stream::BoxStream;
    use futures::StreamExt;
    use serde_json::Value;
    use tools_rs::{CallId, FunctionCall, ToolCollection};

    const CLAUDE_API_URL: &str = "https://api.anthropic.com/v1/messages";

    #[async_trait::async_trait]
    impl StreamProvider for ClaudeClient {
        async fn stream(
            &self,
            messages: &mut Messages,
            tools: Option<&ToolCollection>,
            options: Option<&ChatOptions>,
        ) -> Result<BoxStream<'static, Result<StreamEvent, ChatError>>, ChatError> {
            let request_body =
                ClaudeRequest::from_core(&self.model_name, messages, tools, options, None, true)?;

            let res = self
                .http_client
                .post(CLAUDE_API_URL)
                .header("x-api-key", &self.api_key)
                .header("anthropic-version", &self.api_version)
                .header("content-type", "application/json")
                .json(&request_body)
                .send()
                .await
                .map_err(|e| ChatError::Network(e.to_string()))?;

            let res = handle_claude_error(res).await.map_err(|f| f.err)?;

            let mut byte_stream = res.bytes_stream();

            let s = stream! {
                let mut buffer = String::new();
                let mut content_blocks: Vec<ClaudeContentBlock> = Vec::new();
                let mut current_block_index: Option<usize> = None;
                let mut current_text = String::new();
                let mut current_thinking = String::new();
                let mut current_tool_id = String::new();
                let mut current_tool_name = String::new();
                let mut current_tool_input = String::new();
                let mut message_id: Option<String> = None;
                let mut model: Option<String> = None;
                let mut stop_reason: Option<String> = None;
                let mut usage: Option<ClaudeUsage> = None;

                while let Some(chunk_result) = byte_stream.next().await {
                    let chunk_bytes = match chunk_result {
                        Ok(bytes) => bytes,
                        Err(e) => {
                            yield Err::<StreamEvent, ChatError>(ChatError::Network(e.to_string()));
                            return;
                        }
                    };

                    buffer.push_str(&String::from_utf8_lossy(&chunk_bytes));

                    while let Some(boundary) = buffer.find("\n\n") {
                        let event_text = buffer[..boundary].to_string();
                        buffer = buffer[boundary + 2..].to_string();

                        let mut event_type = None;
                        let mut event_data = None;

                        for line in event_text.lines() {
                            if let Some(et) = line.strip_prefix("event: ") {
                                event_type = Some(et.to_string());
                            } else if let Some(ed) = line.strip_prefix("data: ") {
                                event_data = Some(ed.to_string());
                            }
                        }

                        let Some(etype) = event_type else {
                            continue;
                        };
                        let data: Option<Value> = event_data
                            .as_deref()
                            .and_then(|d| serde_json::from_str(d).ok());

                        match etype.as_str() {
                            "message_start" => {
                                if let Some(d) = &data {
                                    if let Some(msg) = d.get("message") {
                                        message_id = msg
                                            .get("id")
                                            .and_then(|v| v.as_str())
                                            .map(|s| s.to_string());
                                        model = msg
                                            .get("model")
                                            .and_then(|v| v.as_str())
                                            .map(|s| s.to_string());
                                        if let Some(u) = msg.get("usage") {
                                            usage = serde_json::from_value(u.clone()).ok();
                                        }
                                    }
                                }
                            }
                            "content_block_start" => {
                                if let Some(d) = &data {
                                    current_block_index = d
                                        .get("index")
                                        .and_then(|v| v.as_u64())
                                        .map(|v| v as usize);
                                    if let Some(cb) = d.get("content_block") {
                                        let block_type = cb
                                            .get("type")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("");
                                        match block_type {
                                            "text" => current_text.clear(),
                                            "thinking" => current_thinking.clear(),
                                            "tool_use" => {
                                                current_tool_id = cb
                                                    .get("id")
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("")
                                                    .to_string();
                                                current_tool_name = cb
                                                    .get("name")
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("")
                                                    .to_string();
                                                current_tool_input.clear();
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                            }
                            "content_block_delta" => {
                                if let Some(d) = &data {
                                    if let Some(delta) = d.get("delta") {
                                        let delta_type = delta
                                            .get("type")
                                            .and_then(|v| v.as_str())
                                            .unwrap_or("");
                                        match delta_type {
                                            "text_delta" => {
                                                if let Some(text) =
                                                    delta.get("text").and_then(|v| v.as_str())
                                                {
                                                    current_text.push_str(text);
                                                    yield Ok(StreamEvent::TextChunk(
                                                        text.to_string(),
                                                    ));
                                                }
                                            }
                                            "thinking_delta" => {
                                                if let Some(thinking) = delta
                                                    .get("thinking")
                                                    .and_then(|v| v.as_str())
                                                {
                                                    current_thinking.push_str(thinking);
                                                }
                                            }
                                            "input_json_delta" => {
                                                if let Some(partial) = delta
                                                    .get("partial_json")
                                                    .and_then(|v| v.as_str())
                                                {
                                                    current_tool_input.push_str(partial);
                                                }
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                            }
                            "content_block_stop" => {
                                if current_block_index.is_some() {
                                    if !current_text.is_empty() {
                                        content_blocks.push(ClaudeContentBlock::Text {
                                            text: std::mem::take(&mut current_text),
                                        });
                                    }
                                    if !current_thinking.is_empty() {
                                        content_blocks.push(ClaudeContentBlock::Thinking {
                                            thinking: std::mem::take(&mut current_thinking),
                                        });
                                    }
                                    if !current_tool_name.is_empty() {
                                        let input: Value =
                                            serde_json::from_str(&current_tool_input)
                                                .unwrap_or(Value::Object(Default::default()));
                                        content_blocks.push(ClaudeContentBlock::ToolUse {
                                            id: std::mem::take(&mut current_tool_id),
                                            name: std::mem::take(&mut current_tool_name),
                                            input,
                                        });
                                        current_tool_input.clear();
                                    }
                                    current_block_index = None;
                                }
                            }
                            "message_delta" => {
                                if let Some(d) = &data {
                                    if let Some(sr) =
                                        d.get("stop_reason").and_then(|v| v.as_str())
                                    {
                                        stop_reason = Some(sr.to_string());
                                    }
                                    if let Some(u) = d.get("usage") {
                                        if let Ok(u) =
                                            serde_json::from_value::<ClaudeUsage>(u.clone())
                                        {
                                            if let Some(ref mut existing) = usage {
                                                if u.output_tokens.is_some() {
                                                    existing.output_tokens = u.output_tokens;
                                                }
                                            } else {
                                                usage = Some(u);
                                            }
                                        }
                                    }
                                }
                            }
                            "message_stop" => {
                                let mut core_parts = Parts::default();
                                for block in &content_blocks {
                                    match block {
                                        ClaudeContentBlock::Text { text } => {
                                            core_parts
                                                .push(PartEnum::Text(Text::new(text)));
                                        }
                                        ClaudeContentBlock::Thinking { thinking } => {
                                            core_parts.push(PartEnum::Reasoning(
                                                Text::new(thinking),
                                            ));
                                        }
                                        ClaudeContentBlock::ToolUse { id, name, input } => {
                                            let call_id = CallId::from(id.clone());
                                            core_parts.push(
                                                PartEnum::from_function_call(FunctionCall {
                                                    id: Some(call_id),
                                                    name: name.clone(),
                                                    arguments: input.clone(),
                                                }),
                                            );
                                        }
                                    }
                                }

                                let complete_reason = match stop_reason.as_deref() {
                                    Some("end_turn") => CompleteReasonEnum::Stop,
                                    Some("max_tokens") => CompleteReasonEnum::MaxTokens,
                                    Some("tool_use") => CompleteReasonEnum::ToolCall,
                                    Some(other) => {
                                        CompleteReasonEnum::Other(other.to_string())
                                    }
                                    None => CompleteReasonEnum::None,
                                };

                                let input_tokens = usage
                                    .as_ref()
                                    .and_then(|u| u.input_tokens)
                                    .unwrap_or(0);
                                let output_tokens = usage
                                    .as_ref()
                                    .and_then(|u| u.output_tokens)
                                    .unwrap_or(0);

                                let metadata = Metadata {
                                    id: message_id.clone(),
                                    model_slug: model.clone(),
                                    usage: Usage {
                                        input_tokens,
                                        output_tokens,
                                        total_tokens: input_tokens + output_tokens,
                                    },
                                    ..Default::default()
                                };

                                yield Ok(StreamEvent::Done(ChatResponse {
                                    content: Content {
                                        parts: core_parts,
                                        role: RoleEnum::Model,
                                        complete_reason,
                                    },
                                    metadata: Some(metadata),
                                }));
                            }
                            "error" => {
                                if let Some(d) = &data {
                                    let msg = d
                                        .get("error")
                                        .and_then(|e| e.get("message"))
                                        .and_then(|m| m.as_str())
                                        .unwrap_or("Unknown stream error");
                                    yield Err(ChatError::Provider(msg.to_string()));
                                    return;
                                }
                            }
                            _ => {}
                        }
                    }
                }
            };

            Ok(Box::pin(s))
        }
    }
}
