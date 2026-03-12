use serde_json::{json, Value};

use crate::tools::GeminiNativeTool;

#[derive(Clone)]
pub struct GoogleSearchTool {
    pub dynamic_threshold: Option<f32>,
}

impl GeminiNativeTool for GoogleSearchTool {
    /// The tool's unique key used to identify this Gemini native tool.
    ///
    /// # Examples
    ///
    /// ```
    /// let t = GoogleSearchTool { dynamic_threshold: None };
    /// assert_eq!(t.tool_key(), "googleSearch");
    /// ```
    fn tool_key(&self) -> &'static str {
        "googleSearch"
    }

    /// Indicates that this tool provides search capabilities.
    ///
    /// Returns `true`.
    ///
    /// # Examples
    ///
    /// ```
    /// let tool = GoogleSearchTool { dynamic_threshold: None };
    /// assert!(tool.is_search());
    /// ```
    fn is_search(&self) -> bool {
        true
    }

    /// Creates the Gemini tool declaration for the Google Search tool.
    ///
    /// Produces a JSON object mapping the tool key `"googleSearch"` to an empty object.
    ///
    /// # Examples
    ///
    /// ```
    /// use serde_json::json;
    /// let tool = GoogleSearchTool { dynamic_threshold: None };
    /// let decl = tool.to_tool_declaration();
    /// assert_eq!(decl, json!({ "googleSearch": {} }));
    /// ```
    fn to_tool_declaration(&self) -> Value {
        json!({ "googleSearch": {} })
    }

    /// Provide an optional retrieval configuration for Google Search when a dynamic threshold is set.
    ///
    /// If `dynamic_threshold` is `Some(t)`, returns `Some((name, value))` where `name` is
    /// `"googleSearchRetrieval"` and `value` is a JSON object containing a `dynamicRetrievalConfig`
    /// with `"mode": "MODE_DYNAMIC"` and `"dynamicThreshold": t`. Returns `None` if
    /// `dynamic_threshold` is `None`.
    ///
    /// # Examples
    ///
    /// ```
    /// use serde_json::json;
    /// use serde_json::Value;
    ///
    /// let tool = GoogleSearchTool { dynamic_threshold: Some(0.75) };
    /// let cfg = tool.to_tool_config().unwrap();
    /// assert_eq!(cfg.0, "googleSearchRetrieval");
    /// assert_eq!(cfg.1["dynamicRetrievalConfig"]["mode"], "MODE_DYNAMIC");
    /// assert_eq!(cfg.1["dynamicRetrievalConfig"]["dynamicThreshold"], json!(0.75));
    /// ```
    fn to_tool_config(&self) -> Option<(String, Value)> {
        self.dynamic_threshold.map(|t| {
            (
                "googleSearchRetrieval".to_string(),
                json!({
                    "dynamicRetrievalConfig": {
                        "mode": "MODE_DYNAMIC",
                        "dynamicThreshold": t
                    }
                }),
            )
        })
    }

    /// Create a boxed clone of the tool as a `Box<dyn GeminiNativeTool>`.

    ///

    /// # Examples

    ///

    /// ```

    /// let orig = GoogleSearchTool { dynamic_threshold: Some(0.5) };

    /// let boxed: Box<dyn GeminiNativeTool> = orig.clone_box();

    /// ```
    fn clone_box(&self) -> Box<dyn GeminiNativeTool> {
        Box::new(self.clone())
    }
}

