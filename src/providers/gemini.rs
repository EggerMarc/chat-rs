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
    /// Creates a `GeminiClient` configured for the given model.
    ///
    /// The function obtains the Gemini API key from the `GEMINI_API_KEY` environment variable and
    /// returns an error if the key is not present or cannot be read.
    ///
    /// # Parameters
    ///
    /// * `model_name` - The identifier of the Gemini model to use (e.g., `"gemini-pro"`).
    ///
    /// # Returns
    ///
    /// `Ok(GeminiClient)` containing the provided model name and the API key from `GEMINI_API_KEY`,
    /// `Err` if the environment variable is missing or cannot be read.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::env;
    /// env::set_var("GEMINI_API_KEY", "test-key");
    /// let client = crate::providers::gemini::GeminiClient::new("gemini-pro").unwrap();
    /// assert_eq!(client.model_name, "gemini-pro");
    /// ```
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
    /// Send the given messages to the Google Gemini generateContent endpoint and return the parsed Content.
    ///
    /// The request body is constructed from `messages` and, if provided, includes tool function declarations from `tools`.
    /// The response is parsed into the crate's `Content` representation.
    ///
    /// # Parameters
    ///
    /// - `messages`: conversation messages to send to the model.
    /// - `tools`: optional collection of tools whose function declarations will be included in the request.
    /// - `_options`: currently unused chat options (kept for API compatibility).
    ///
    /// # Returns
    ///
    /// `Ok(Content)` with the parsed content on success; `Err(ChatError)` on failure.
    /// Returns `ChatError::Provider` for HTTP or response-read errors, and `ChatError::InvalidResponse` for JSON parse or content-parsing errors.
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use crate::providers::gemini::GeminiClient;
    /// # use crate::chat::{Messages, ToolCollection, ChatOptions};
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = GeminiClient::new("models/example")?;
    /// let messages = Messages::default();
    /// let content = client.complete(&messages, None, None).await?;
    /// // inspect content...
    /// # Ok(())
    /// # }
    /// ```
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
    /// Converts the message sequence into a Gemini-formatted JSON array.
    ///
    /// Each message is transformed with its `into_gemini` representation and collected into a JSON array value.
    ///
    /// # Examples
    ///
    /// ```
    /// // Construct a Messages value containing one simple text content.
    /// let msgs = Messages(vec![Content::from_text("hello")]);
    /// let json = msgs.into_gemini();
    /// assert!(json.is_array());
    /// ```
    fn into_gemini(&self) -> Value {
        self.0.iter().map(|content| content.into_gemini()).collect()
    }
}

impl Content {
    /// Convert this Content into the Gemini API JSON shape.
    ///
    /// The returned JSON object has a `"parts"` array where each element is the Gemini-formatted
    /// representation of a single part produced by `PartEnum::into_gemini`.
    ///
    /// # Examples
    ///
    /// ```
    /// // Construct a Content with a single text part and convert it to Gemini JSON.
    /// // (Types shown for illustration; adjust to actual constructors in this crate.)
    /// let content = Content { parts: Parts(vec![PartEnum::Text(Text::new("hello".into()))]) };
    /// let value = content.into_gemini();
    /// assert!(value.get("parts").is_some());
    /// ```
    fn into_gemini(&self) -> Value {
        json!({
            "parts": self.parts.0.iter().map(|part| part.into_gemini()).collect::<Vec<Value>>()
        })
    }
}

impl PartEnum {
    /// Convert this PartEnum into the JSON shape expected by the Gemini API.
    ///
    /// # Examples
    ///
    /// ```
    /// use serde_json::json;
    /// // assume PartEnum::Text is available in scope
    /// let part = PartEnum::Text("hello".into());
    /// let v = part.into_gemini();
    /// assert_eq!(v, json!({"text": "hello"}));
    /// ```
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

/// Parse a Gemini API response candidate into the crate's internal `Content` representation.
///
/// The function extracts the first candidate's `content`, converts its `parts` into `PartEnum` values
/// (currently supports `text` and `functionCall`), maps the content `role` to `RoleEnum`, and
/// maps the candidate `finishReason` to `CompleteReasonEnum`.
///
/// # Parameters
///
/// - `json`: The full JSON response from the Gemini API; the function reads `candidates[0]["content"]`
///   and related fields.
///
/// # Returns
///
/// `Ok(Content)` with populated `parts`, `role`, and `complete_reason` on success, or `Err(ChatError)`
/// if required fields for a function call (name or args) are missing or cannot be serialized.
///
/// # Examples
///
/// ```
/// use serde_json::json;
///
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
/// let content = parse_gemini_content(&resp).unwrap();
/// assert_eq!(content.parts.len(), 1);
/// ```
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
    use serde_json::json;

    #[test]
    fn test_gemini_client_new_success() {
        std::env::set_var("GEMINI_API_KEY", "test_api_key");
        
        let client = GeminiClient::new("gemini-2.5-flash");
        assert!(client.is_ok());
        
        let client = client.unwrap();
        assert_eq!(client.model_name, "gemini-2.5-flash");
        assert_eq!(client.api_key, "test_api_key");
    }

    #[test]
    fn test_gemini_client_new_missing_api_key() {
        std::env::remove_var("GEMINI_API_KEY");
        
        let client = GeminiClient::new("gemini-2.5-flash");
        assert!(client.is_err());
    }

    #[test]
    fn test_gemini_client_new_different_models() {
        std::env::set_var("GEMINI_API_KEY", "test_key");
        
        let client1 = GeminiClient::new("gemini-1.5-pro").unwrap();
        assert_eq!(client1.model_name, "gemini-1.5-pro");
        
        let client2 = GeminiClient::new("gemini-2.0-flash").unwrap();
        assert_eq!(client2.model_name, "gemini-2.0-flash");
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
        messages.push(content::from_user(vec!["Hello"]));
        
        let gemini_value = messages.into_gemini();
        assert!(gemini_value.is_array());
        assert_eq!(gemini_value.as_array().unwrap().len(), 1);
    }

    #[test]
    fn test_messages_into_gemini_multiple_messages() {
        let mut messages = Messages::default();
        messages.push(content::from_user(vec!["User message"]));
        messages.push(content::from_system(vec!["System prompt"]));
        messages.push(content::from_model(vec!["Model response"]));
        
        let gemini_value = messages.into_gemini();
        assert_eq!(gemini_value.as_array().unwrap().len(), 3);
    }

    #[test]
    fn test_content_into_gemini() {
        let content = content::from_user(vec!["Test message"]);
        let gemini_value = content.into_gemini();
        
        assert!(gemini_value.is_object());
        assert!(gemini_value.get("parts").is_some());
        assert!(gemini_value.get("parts").unwrap().is_array());
    }

    #[test]
    fn test_content_into_gemini_with_multiple_parts() {
        let mut content = content::from_user(vec!["First", "Second"]);
        let gemini_value = content.into_gemini();
        
        let parts = gemini_value.get("parts").unwrap().as_array().unwrap();
        assert_eq!(parts.len(), 2);
    }

    #[test]
    fn test_part_enum_text_into_gemini() {
        let part = PartEnum::from_text("Hello, world!");
        let gemini_value = part.into_gemini();
        
        assert!(gemini_value.is_object());
        assert_eq!(gemini_value.get("text").unwrap().as_str().unwrap(), "Hello, world!");
    }

    #[test]
    fn test_part_enum_reasoning_into_gemini() {
        let part = PartEnum::from_reasoning("Thinking...");
        let gemini_value = part.into_gemini();
        
        assert!(gemini_value.is_object());
        assert!(gemini_value.get("reasoning").is_some());
    }

    #[test]
    fn test_part_enum_function_call_into_gemini() {
        let fc = FunctionCall::new("test_function".to_string(), json!({"arg": "value"}));
        let part = PartEnum::from_function_call(fc);
        let gemini_value = part.into_gemini();
        
        assert!(gemini_value.is_object());
        assert!(gemini_value.get("function_call").is_some());
    }

    #[test]
    fn test_part_enum_function_response_into_gemini() {
        let fc = FunctionCall::new("test".to_string(), json!({}));
        let fr = FunctionResponse {
            id: fc.id,
            name: "test".to_string(),
            result: json!({"status": "ok"}),
        };
        let part = PartEnum::from_function_response(fr);
        let gemini_value = part.into_gemini();
        
        assert!(gemini_value.is_object());
        assert!(gemini_value.get("function_response").is_some());
    }

    #[test]
    fn test_parse_gemini_content_with_text() {
        let json = json!({
            "candidates": [{
                "content": {
                    "role": "model",
                    "parts": [
                        {"text": "Hello, this is a response"}
                    ]
                },
                "finishReason": "STOP"
            }]
        });
        
        let content = parse_gemini_content(&json).unwrap();
        assert_eq!(content.role, RoleEnum::Model);
        assert_eq!(content.complete_reason, CompleteReasonEnum::Stop);
        assert_eq!(content.parts.length(), 1);
        assert_eq!(content.parts.text_response().unwrap().as_str(), "Hello, this is a response");
    }

    #[test]
    fn test_parse_gemini_content_with_function_call() {
        let json = json!({
            "candidates": [{
                "content": {
                    "role": "model",
                    "parts": [
                        {
                            "functionCall": {
                                "name": "get_weather",
                                "args": {
                                    "location": "San Francisco"
                                }
                            }
                        }
                    ]
                },
                "finishReason": "STOP"
            }]
        });
        
        let content = parse_gemini_content(&json).unwrap();
        assert_eq!(content.parts.length(), 1);
        
        let fc = content.parts.function_calls().next().unwrap();
        assert_eq!(fc.name, "get_weather");
    }

    #[test]
    fn test_parse_gemini_content_multiple_parts() {
        let json = json!({
            "candidates": [{
                "content": {
                    "role": "model",
                    "parts": [
                        {"text": "First part"},
                        {"text": "Second part"}
                    ]
                },
                "finishReason": "STOP"
            }]
        });
        
        let content = parse_gemini_content(&json).unwrap();
        assert_eq!(content.parts.length(), 2);
    }

    #[test]
    fn test_parse_gemini_content_role_user() {
        let json = json!({
            "candidates": [{
                "content": {
                    "role": "user",
                    "parts": [{"text": "User message"}]
                },
                "finishReason": "STOP"
            }]
        });
        
        let content = parse_gemini_content(&json).unwrap();
        assert_eq!(content.role, RoleEnum::User);
    }

    #[test]
    fn test_parse_gemini_content_role_system() {
        let json = json!({
            "candidates": [{
                "content": {
                    "role": "system",
                    "parts": [{"text": "System message"}]
                },
                "finishReason": "STOP"
            }]
        });
        
        let content = parse_gemini_content(&json).unwrap();
        assert_eq!(content.role, RoleEnum::System);
    }

    #[test]
    fn test_parse_gemini_content_role_unknown_defaults_to_model() {
        let json = json!({
            "candidates": [{
                "content": {
                    "role": "unknown_role",
                    "parts": [{"text": "Message"}]
                },
                "finishReason": "STOP"
            }]
        });
        
        let content = parse_gemini_content(&json).unwrap();
        assert_eq!(content.role, RoleEnum::Model);
    }

    #[test]
    fn test_parse_gemini_content_finish_reason_stop() {
        let json = json!({
            "candidates": [{
                "content": {
                    "role": "model",
                    "parts": [{"text": "Done"}]
                },
                "finishReason": "STOP"
            }]
        });
        
        let content = parse_gemini_content(&json).unwrap();
        assert_eq!(content.complete_reason, CompleteReasonEnum::Stop);
    }

    #[test]
    fn test_parse_gemini_content_finish_reason_max_tokens() {
        let json = json!({
            "candidates": [{
                "content": {
                    "role": "model",
                    "parts": [{"text": "Truncated"}]
                },
                "finishReason": "MAX_TOKENS"
            }]
        });
        
        let content = parse_gemini_content(&json).unwrap();
        assert_eq!(content.complete_reason, CompleteReasonEnum::MaxTokens);
    }

    #[test]
    fn test_parse_gemini_content_finish_reason_safety() {
        let json = json!({
            "candidates": [{
                "content": {
                    "role": "model",
                    "parts": [{"text": "Filtered"}]
                },
                "finishReason": "SAFETY"
            }]
        });
        
        let content = parse_gemini_content(&json).unwrap();
        assert_eq!(content.complete_reason, CompleteReasonEnum::ContentFilter);
    }

    #[test]
    fn test_parse_gemini_content_finish_reason_unknown_defaults_to_none() {
        let json = json!({
            "candidates": [{
                "content": {
                    "role": "model",
                    "parts": [{"text": "Message"}]
                },
                "finishReason": "UNKNOWN"
            }]
        });
        
        let content = parse_gemini_content(&json).unwrap();
        assert_eq!(content.complete_reason, CompleteReasonEnum::None);
    }

    #[test]
    fn test_parse_gemini_content_no_finish_reason() {
        let json = json!({
            "candidates": [{
                "content": {
                    "role": "model",
                    "parts": [{"text": "Message"}]
                }
            }]
        });
        
        let content = parse_gemini_content(&json).unwrap();
        assert_eq!(content.complete_reason, CompleteReasonEnum::None);
    }

    #[test]
    fn test_parse_gemini_content_empty_parts() {
        let json = json!({
            "candidates": [{
                "content": {
                    "role": "model",
                    "parts": []
                },
                "finishReason": "STOP"
            }]
        });
        
        let content = parse_gemini_content(&json).unwrap();
        assert_eq!(content.parts.length(), 0);
    }

    #[test]
    fn test_parse_gemini_content_missing_name_in_function_call() {
        let json = json!({
            "candidates": [{
                "content": {
                    "role": "model",
                    "parts": [
                        {
                            "functionCall": {
                                "args": {"key": "value"}
                            }
                        }
                    ]
                },
                "finishReason": "STOP"
            }]
        });
        
        let result = parse_gemini_content(&json);
        assert!(result.is_err());
        match result.unwrap_err() {
            ChatError::Provider(msg) => {
                assert!(msg.contains("function call name"));
            }
            _ => panic!("Expected Provider error"),
        }
    }

    #[test]
    fn test_parse_gemini_content_missing_args_in_function_call() {
        let json = json!({
            "candidates": [{
                "content": {
                    "role": "model",
                    "parts": [
                        {
                            "functionCall": {
                                "name": "test_func"
                            }
                        }
                    ]
                },
                "finishReason": "STOP"
            }]
        });
        
        let result = parse_gemini_content(&json);
        assert!(result.is_err());
        match result.unwrap_err() {
            ChatError::Provider(msg) => {
                assert!(msg.contains("function call arguments"));
            }
            _ => panic!("Expected Provider error"),
        }
    }

    #[test]
    fn test_parse_gemini_content_mixed_parts() {
        let json = json!({
            "candidates": [{
                "content": {
                    "role": "model",
                    "parts": [
                        {"text": "Let me call a function"},
                        {
                            "functionCall": {
                                "name": "get_data",
                                "args": {"id": 123}
                            }
                        },
                        {"text": "Done"}
                    ]
                },
                "finishReason": "STOP"
            }]
        });
        
        let content = parse_gemini_content(&json).unwrap();
        assert_eq!(content.parts.length(), 3);
        assert_eq!(content.parts.text_parts().count(), 2);
        assert_eq!(content.parts.function_calls().count(), 1);
    }

    #[test]
    fn test_parse_gemini_content_with_unicode() {
        let json = json!({
            "candidates": [{
                "content": {
                    "role": "model",
                    "parts": [
                        {"text": "Hello 世界 🌍"}
                    ]
                },
                "finishReason": "STOP"
            }]
        });
        
        let content = parse_gemini_content(&json).unwrap();
        assert_eq!(content.parts.text_response().unwrap().as_str(), "Hello 世界 🌍");
    }

    #[test]
    fn test_parse_gemini_content_with_special_characters() {
        let json = json!({
            "candidates": [{
                "content": {
                    "role": "model",
                    "parts": [
                        {"text": "Special chars: @#$%^&*()"}
                    ]
                },
                "finishReason": "STOP"
            }]
        });
        
        let content = parse_gemini_content(&json).unwrap();
        assert_eq!(content.parts.text_response().unwrap().as_str(), "Special chars: @#$%^&*()");
    }

    #[test]
    fn test_parse_gemini_content_empty_text() {
        let json = json!({
            "candidates": [{
                "content": {
                    "role": "model",
                    "parts": [
                        {"text": ""}
                    ]
                },
                "finishReason": "STOP"
            }]
        });
        
        let content = parse_gemini_content(&json).unwrap();
        assert_eq!(content.parts.length(), 1);
        assert_eq!(content.parts.text_response().unwrap().as_str(), "");
    }

    #[test]
    fn test_parse_gemini_content_function_call_with_complex_args() {
        let json = json!({
            "candidates": [{
                "content": {
                    "role": "model",
                    "parts": [
                        {
                            "functionCall": {
                                "name": "complex_function",
                                "args": {
                                    "nested": {
                                        "key": "value",
                                        "array": [1, 2, 3]
                                    },
                                    "simple": "string"
                                }
                            }
                        }
                    ]
                },
                "finishReason": "STOP"
            }]
        });
        
        let content = parse_gemini_content(&json).unwrap();
        let fc = content.parts.function_calls().next().unwrap();
        assert_eq!(fc.name, "complex_function");
        assert!(fc.args.get("nested").is_some());
        assert!(fc.args.get("simple").is_some());
    }
}
