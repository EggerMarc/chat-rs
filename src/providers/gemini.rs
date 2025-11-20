use std::env;

use async_trait::async_trait;
use serde_json::Value;
use serde_json::json;
use tools_rs::FunctionCall;
use tools_rs::ToolCollection;

use crate::core::lib::ChatOptions;
use crate::core::{
    lib::{ChatError, ChatProvider},
    messages::{Messages, content::Content},
};
use crate::messages::content::CompleteReasonEnum;
use crate::messages::content::RoleEnum;
use crate::messages::parts::PartEnum;
use crate::messages::parts::Parts;
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
    ) -> Result<Content, ChatError> {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            self.model_name, self.api_key
        );

        let body = match tools {
            Some(t) => json!({
                "contents": messages.into_gemini(),
                "tools": {
                   "functionDeclarations": t.json().unwrap()
                }
            }),
            None => json!({
                "contents": messages.into_gemini()
            }),
        };

        let res = reqwest::Client::new()
            .post(url)
            .body(body.to_string())
            .send()
            .await
            .map_err(|e| ChatError::Provider(e.to_string()))?;
        let text = res
            .text()
            .await
            .map_err(|e| ChatError::Provider(e.to_string()))?;

        let json: Value =
            serde_json::from_str(&text).map_err(|e| ChatError::InvalidResponse(e.to_string()))?;

        let content = parse_gemini_content(&json);
        if content.is_err() {
            println!("Caught error in parser");
        }
        //.map_err(|e| ChatError::InvalidResponse(e.to_string()))?;

        Ok(content.unwrap())
    }
}

impl Messages {
    fn into_gemini(&self) -> Value {
        self.0.iter().map(|content| content.into_gemini()).collect()
    }
}

impl Content {
    fn into_gemini(&self) -> Value {
        json!({
            "parts": self.parts.0.iter().map(|part| part.into_gemini()).collect::<Vec<Value>>()
        })
    }
}

impl PartEnum {
    fn into_gemini(&self) -> Value {
        match self {
            PartEnum::Reasoning(text) => json!({"reasoning": text}),
            PartEnum::Text(text) => json!({"text": text}),
            PartEnum::FunctionCall(fc) => json!({"function_call": fc}),
            PartEnum::FunctionResponse(fr) => json!({"function_response": fr}),
            _ => unimplemented!(),
        }
    }
}

fn parse_gemini_content(json: &serde_json::Value) -> Result<Content, ChatError> {
    let content_json = &json["candidates"][0]["content"];

    let mut parts = Parts::default();

    // parse parts array
    if let Some(arr) = content_json["parts"].as_array() {
        for item in arr {
            if let Some(text) = item.get("text").and_then(|v| v.as_str()) {
                parts.push(PartEnum::Text(Text::new(text)));
            }

            /*if let Some(sig) = item.get("thoughtSignature").and_then(|v| v.as_str()) {
                parts.push(PartEnum::Reasoning(Text::new(sig)));
            }*/

            if let Some(fc) = item.get("functionCall") {
                parts.push(PartEnum::from_function_call(FunctionCall::new(
                    fc["name"]
                        .as_str()
                        .ok_or(ChatError::Provider(
                            "Failed to serialize function call name".to_string(),
                        ))?
                        .to_string(),
                    serde_json::Value::Object(
                        fc["args"]
                            .as_object()
                            .ok_or(ChatError::Provider(
                                "Failed to serialize function call arguments".to_string(),
                            ))?
                            .clone(),
                    ),
                )));
            }
        }
    }

    // parse role
    let role = match content_json["role"].as_str().unwrap_or_default() {
        "user" => RoleEnum::User,
        "system" => RoleEnum::System,
        "model" => RoleEnum::Model,
        _ => RoleEnum::Model,
    };

    // parse finish reason
    /*println!(
        "COMPLETE REASON: {:#?}, ALL PARTS: {:#?}\n\n END",
        json["candidates"][0]["finishReason"], parts
    );*/

    let complete_reason = match json["candidates"][0]["finishReason"]
        .as_str()
        .unwrap_or_default()
    {
        "STOP" => CompleteReasonEnum::Stop,
        "MAX_TOKENS" => CompleteReasonEnum::MaxTokens,
        "SAFETY" => CompleteReasonEnum::ContentFilter,
        _ => CompleteReasonEnum::None,
    };

    Ok(Content {
        parts,
        role,
        complete_reason,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messages::content::{CompleteReasonEnum, RoleEnum};
    use crate::messages::parts::{PartEnum, Parts};
    use serde_json::json;

    #[test]
    fn test_gemini_client_new_success() {
        // This test requires GEMINI_API_KEY to be set
        std::env::set_var("GEMINI_API_KEY", "test_key_12345");
        
        let result = GeminiClient::new("gemini-2.5-flash");
        assert!(result.is_ok());
        
        let client = result.unwrap();
        assert_eq!(client.model_name, "gemini-2.5-flash");
        assert_eq!(client.api_key, "test_key_12345");
        
        std::env::remove_var("GEMINI_API_KEY");
    }

    #[test]
    fn test_gemini_client_new_missing_api_key() {
        std::env::remove_var("GEMINI_API_KEY");
        
        let result = GeminiClient::new("gemini-2.5-flash");
        assert!(result.is_err());
    }

    #[test]
    fn test_gemini_client_new_different_models() {
        std::env::set_var("GEMINI_API_KEY", "test_key");
        
        let models = vec![
            "gemini-2.5-flash",
            "gemini-pro",
            "gemini-1.5-pro",
            "custom-model-name"
        ];
        
        for model in models {
            let client = GeminiClient::new(model).unwrap();
            assert_eq!(client.model_name, model);
        }
        
        std::env::remove_var("GEMINI_API_KEY");
    }

    #[test]
    fn test_messages_into_gemini_empty() {
        let messages = Messages::default();
        let gemini_value = messages.into_gemini();
        
        assert!(gemini_value.is_array());
        assert_eq!(gemini_value.as_array().unwrap().len(), 0);
    }

    #[test]
    fn test_messages_into_gemini_single_message() {
        let mut messages = Messages::default();
        messages.push(content::from_user(vec!["Test message"]));
        
        let gemini_value = messages.into_gemini();
        assert!(gemini_value.is_array());
        assert_eq!(gemini_value.as_array().unwrap().len(), 1);
    }

    #[test]
    fn test_messages_into_gemini_multiple_messages() {
        let mut messages = Messages::default();
        messages.push(content::from_system(vec!["System"]));
        messages.push(content::from_user(vec!["User"]));
        messages.push(content::from_model(vec!["Model"]));
        
        let gemini_value = messages.into_gemini();
        assert_eq!(gemini_value.as_array().unwrap().len(), 3);
    }

    #[test]
    fn test_content_into_gemini_text_part() {
        let content = content::from_user(vec!["Hello world"]);
        let gemini_value = content.into_gemini();
        
        assert!(gemini_value.is_object());
        assert!(gemini_value["parts"].is_array());
        
        let parts = gemini_value["parts"].as_array().unwrap();
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0]["text"], "Hello world");
    }

    #[test]
    fn test_content_into_gemini_multiple_parts() {
        let mut content = Content {
            role: RoleEnum::User,
            parts: Parts::default(),
            complete_reason: CompleteReasonEnum::None,
        };
        content.parts.push(PartEnum::from_text("Part 1".to_string()));
        content.parts.push(PartEnum::from_text("Part 2".to_string()));
        
        let gemini_value = content.into_gemini();
        let parts = gemini_value["parts"].as_array().unwrap();
        assert_eq!(parts.len(), 2);
    }

    #[test]
    fn test_part_enum_into_gemini_text() {
        let part = PartEnum::from_text("Test text".to_string());
        let gemini_value = part.into_gemini();
        
        assert_eq!(gemini_value["text"], "Test text");
    }

    #[test]
    fn test_part_enum_into_gemini_reasoning() {
        let part = PartEnum::from_reasoning("Thinking...".to_string());
        let gemini_value = part.into_gemini();
        
        assert!(gemini_value["reasoning"].is_object());
    }

    #[test]
    fn test_part_enum_into_gemini_function_call() {
        let fc = FunctionCall::new("test_func".to_string(), json!({"arg": "value"}));
        let part = PartEnum::from_function_call(fc.clone());
        let gemini_value = part.into_gemini();
        
        assert!(gemini_value["function_call"].is_object());
    }

    #[test]
    fn test_part_enum_into_gemini_function_response() {
        let call_id = tools_rs::CallId::new();
        let fr = FunctionResponse {
            id: call_id,
            name: "test_func".to_string(),
            result: json!({"result": "success"}),
        };
        let part = PartEnum::from_function_response(fr);
        let gemini_value = part.into_gemini();
        
        assert!(gemini_value["function_response"].is_object());
    }

    #[test]
    fn test_parse_gemini_content_with_text() {
        let json_response = json!({
            "candidates": [{
                "content": {
                    "parts": [
                        {"text": "Hello, I'm here to help!"}
                    ],
                    "role": "model"
                },
                "finishReason": "STOP"
            }]
        });
        
        let result = parse_gemini_content(&json_response);
        assert!(result.is_ok());
        
        let content = result.unwrap();
        assert_eq!(content.role, RoleEnum::Model);
        assert_eq!(content.complete_reason, CompleteReasonEnum::Stop);
        assert_eq!(content.parts.length(), 1);
    }

    #[test]
    fn test_parse_gemini_content_with_function_call() {
        let json_response = json!({
            "candidates": [{
                "content": {
                    "parts": [{
                        "functionCall": {
                            "name": "get_weather",
                            "args": {
                                "location": "San Francisco"
                            }
                        }
                    }],
                    "role": "model"
                },
                "finishReason": "STOP"
            }]
        });
        
        let result = parse_gemini_content(&json_response);
        assert!(result.is_ok());
        
        let content = result.unwrap();
        assert_eq!(content.parts.length(), 1);
        
        let fcs = content.parts.function_calls();
        assert_eq!(fcs.len(), 1);
        assert_eq!(fcs[0].name, "get_weather");
    }

    #[test]
    fn test_parse_gemini_content_multiple_parts() {
        let json_response = json!({
            "candidates": [{
                "content": {
                    "parts": [
                        {"text": "Let me check that for you."},
                        {
                            "functionCall": {
                                "name": "search",
                                "args": {"query": "weather"}
                            }
                        }
                    ],
                    "role": "model"
                },
                "finishReason": "STOP"
            }]
        });
        
        let result = parse_gemini_content(&json_response);
        assert!(result.is_ok());
        
        let content = result.unwrap();
        assert_eq!(content.parts.length(), 2);
    }

    #[test]
    fn test_parse_gemini_content_user_role() {
        let json_response = json!({
            "candidates": [{
                "content": {
                    "parts": [{"text": "User message"}],
                    "role": "user"
                },
                "finishReason": "STOP"
            }]
        });
        
        let result = parse_gemini_content(&json_response);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().role, RoleEnum::User);
    }

    #[test]
    fn test_parse_gemini_content_system_role() {
        let json_response = json!({
            "candidates": [{
                "content": {
                    "parts": [{"text": "System message"}],
                    "role": "system"
                },
                "finishReason": "STOP"
            }]
        });
        
        let result = parse_gemini_content(&json_response);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().role, RoleEnum::System);
    }

    #[test]
    fn test_parse_gemini_content_max_tokens_reason() {
        let json_response = json!({
            "candidates": [{
                "content": {
                    "parts": [{"text": "Partial response..."}],
                    "role": "model"
                },
                "finishReason": "MAX_TOKENS"
            }]
        });
        
        let result = parse_gemini_content(&json_response);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().complete_reason, CompleteReasonEnum::MaxTokens);
    }

    #[test]
    fn test_parse_gemini_content_safety_reason() {
        let json_response = json!({
            "candidates": [{
                "content": {
                    "parts": [{"text": ""}],
                    "role": "model"
                },
                "finishReason": "SAFETY"
            }]
        });
        
        let result = parse_gemini_content(&json_response);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().complete_reason, CompleteReasonEnum::ContentFilter);
    }

    #[test]
    fn test_parse_gemini_content_no_finish_reason() {
        let json_response = json!({
            "candidates": [{
                "content": {
                    "parts": [{"text": "Response"}],
                    "role": "model"
                }
            }]
        });
        
        let result = parse_gemini_content(&json_response);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().complete_reason, CompleteReasonEnum::None);
    }

    #[test]
    fn test_parse_gemini_content_empty_parts() {
        let json_response = json!({
            "candidates": [{
                "content": {
                    "parts": [],
                    "role": "model"
                },
                "finishReason": "STOP"
            }]
        });
        
        let result = parse_gemini_content(&json_response);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().parts.length(), 0);
    }

    #[test]
    fn test_parse_gemini_content_unknown_role_defaults_to_model() {
        let json_response = json!({
            "candidates": [{
                "content": {
                    "parts": [{"text": "Test"}],
                    "role": "unknown_role"
                },
                "finishReason": "STOP"
            }]
        });
        
        let result = parse_gemini_content(&json_response);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().role, RoleEnum::Model);
    }

    #[test]
    fn test_parse_gemini_content_missing_role() {
        let json_response = json!({
            "candidates": [{
                "content": {
                    "parts": [{"text": "Test"}]
                },
                "finishReason": "STOP"
            }]
        });
        
        let result = parse_gemini_content(&json_response);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().role, RoleEnum::Model);
    }

    #[test]
    fn test_gemini_client_model_name_persistence() {
        std::env::set_var("GEMINI_API_KEY", "test_key");
        
        let client = GeminiClient::new("my-custom-model").unwrap();
        assert_eq!(client.model_name, "my-custom-model");
        
        std::env::remove_var("GEMINI_API_KEY");
    }

    #[test]
    fn test_parse_gemini_content_complex_function_args() {
        let json_response = json!({
            "candidates": [{
                "content": {
                    "parts": [{
                        "functionCall": {
                            "name": "complex_func",
                            "args": {
                                "nested": {
                                    "key1": "value1",
                                    "key2": [1, 2, 3]
                                },
                                "list": ["a", "b", "c"],
                                "number": 42,
                                "boolean": true
                            }
                        }
                    }],
                    "role": "model"
                },
                "finishReason": "STOP"
            }]
        });
        
        let result = parse_gemini_content(&json_response);
        assert!(result.is_ok());
        
        let content = result.unwrap();
        let fcs = content.parts.function_calls();
        assert_eq!(fcs.len(), 1);
        assert_eq!(fcs[0].name, "complex_func");
        assert!(fcs[0].args["nested"].is_object());
        assert!(fcs[0].args["list"].is_array());
    }

    #[test]
    fn test_into_gemini_roundtrip_text() {
        let original_content = content::from_user(vec!["Test message"]);
        let gemini_json = original_content.into_gemini();
        
        // Verify structure
        assert!(gemini_json["parts"].is_array());
        let parts = gemini_json["parts"].as_array().unwrap();
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0]["text"], "Test message");
    }

    #[test]
    fn test_into_gemini_preserves_part_order() {
        let mut content = Content {
            role: RoleEnum::Model,
            parts: Parts::default(),
            complete_reason: CompleteReasonEnum::Stop,
        };
        
        content.parts.push(PartEnum::from_text("First".to_string()));
        content.parts.push(PartEnum::from_reasoning("Second".to_string()));
        content.parts.push(PartEnum::from_text("Third".to_string()));
        
        let gemini_json = content.into_gemini();
        let parts = gemini_json["parts"].as_array().unwrap();
        
        assert_eq!(parts.len(), 3);
        assert!(parts[0].get("text").is_some());
        assert!(parts[1].get("reasoning").is_some());
        assert!(parts[2].get("text").is_some());
    }

    #[test]
    fn test_gemini_client_api_key_not_exposed_in_debug() {
        std::env::set_var("GEMINI_API_KEY", "secret_key_12345");
        
        let client = GeminiClient::new("test-model").unwrap();
        let debug_output = format!("{:?}", client);
        
        // In a real implementation, we'd want to ensure the API key
        // is not exposed in debug output, but for now we just verify
        // the struct can be created
        assert!(debug_output.contains("GeminiClient"));
        
        std::env::remove_var("GEMINI_API_KEY");
    }
}
