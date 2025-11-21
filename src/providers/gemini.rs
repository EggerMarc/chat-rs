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
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent",
            self.model_name
        );

        let mut body = json!({});

        if let Some(contents) = messages.into_gemini_messages() {
            body["contents"] = contents["contents"].clone();
        }
        if let Some(system_instructions) = messages.into_gemini_system() {
            body["system_instruction"] = system_instructions["system_instruction"].clone();
        }
        if let Some(t) = tools {
            body["tools"] = json!({
             "functionDeclarations": t.json().map_err(|err| {
                ChatError::Other(format!("Tools-rs Serialization error: {}", err))
                })?
            });
        }

        let req = reqwest::Client::new()
            .post(url)
            .header("Content-Type", "application/json")
            .header("x-goog-api-key", &self.api_key)
            .body(body.to_string());
        //.body(body.to_string());

        println!("Body: {:#?}", &body);
        let res = req
            .send()
            .await
            .map_err(|e| ChatError::Provider(e.to_string()))?;

        println!("Response: {:#?}", res);

        match res.error_for_status() {
            Ok(data) => {
                let text = data
                    .text()
                    .await
                    .map_err(|e| ChatError::Provider(e.to_string()))?;

                let json: Value = serde_json::from_str(&text)
                    .map_err(|e| ChatError::InvalidResponse(e.to_string()))?;

                println!("Content response JSON: {:?}", json);
                let content = parse_gemini_content(&json).map_err(|e| {
                    ChatError::InvalidResponse(format!("Failed to parse gemini content: {}", e))
                })?;

                println!("Content response: {:?}", content);

                //.map_err(|e| ChatError::InvalidResponse(e.to_string()))?;
                return Ok(content);
            }
            Err(err) => {
                println!("Error requesting completion: {}", err);
                return Err(ChatError::Provider(err.without_url().to_string()));
            }
        }
    }
}

impl Messages {
    pub fn into_gemini_system(&self) -> Option<Value> {
        let mut sys_parts: Parts = Parts::default();
        self.0
            .iter()
            .filter(|content| content.role == RoleEnum::System)
            .for_each(|content| {
                sys_parts.extend(content.parts.clone());
            });
        let parts = sys_parts
            .0
            .iter()
            .map(|part| part.into_gemini())
            .collect::<Vec<Value>>();

        if sys_parts.is_empty() {
            None
        } else {
            Some(json!({
                "system_instruction": {
                "parts": parts
                }
            }))
        }
    }

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
    ///
    pub fn into_gemini_messages(&self) -> Option<Value> {
        let contents: Vec<Value> = self
            .0
            .iter()
            .filter(|content| content.role != RoleEnum::System)
            .map(|content| content.into_gemini())
            .collect();

        if contents.is_empty() {
            None
        } else {
            Some(json!({ "contents": contents }))
        }
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
        let parts = self
            .parts
            .0
            .iter()
            // Skip function responses
            .map(|part| part.into_gemini())
            .collect::<Vec<Value>>();

        match self.role {
            RoleEnum::User => json!({
                "parts": parts,
                "role": "user"
            }),
            RoleEnum::Model => json!({
                "parts": parts,
                "role": "model"
            }),
            RoleEnum::System => serde_json::Value::Array(parts),
        }
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
            PartEnum::FunctionCall(fc) => json!({"functionCall": {
                "id": fc.id,
                "name": fc.name,
                "args": fc.arguments
            }}),
            PartEnum::FunctionResponse(fr) => json!({"functionResponse": {
                "id": fr.id,
                "name": fr.name,
                "response": {
                    "content": fr.result
                }
            }}),
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
