use serde_json::Value;
use tools_rs::ToolError;

use crate::types::tools::ToolDeclarations;

/// A static tool declarations implementation for testing.
///
/// Wraps a JSON value (expected to be an array of function declarations)
/// and returns it from `json()`. Use this when testing providers that
/// need tool declarations without setting up a full `ToolCollection`.
///
/// ```ignore
/// use serde_json::json;
/// use chat_core::testing::StaticToolDeclarations;
///
/// let tools = StaticToolDeclarations(json!([{
///     "name": "get_weather",
///     "description": "Get the current weather",
///     "parameters": {
///         "type": "object",
///         "properties": {
///             "location": { "type": "string" }
///         },
///         "required": ["location"]
///     }
/// }]));
/// ```
pub struct StaticToolDeclarations(pub Value);

impl ToolDeclarations for StaticToolDeclarations {
    fn json(&self) -> Result<Value, ToolError> {
        Ok(self.0.clone())
    }
}
