use serde_json::{Value, json};

use crate::gemini::lib::GeminiNativeTool;

#[derive(Clone)]
pub struct GoogleMapsTool {
    pub lat_lng: Option<(f32, f32)>,
    pub enable_widget: bool,
}

impl GeminiNativeTool for GoogleMapsTool {
    /// Returns the identifier used for the Google Maps tool in Gemini tool declarations.
    ///
    /// # Returns
    ///
    /// A static string slice with the tool key: `"googleMaps"`.
    ///
    /// # Examples
    ///
    /// ```
    /// let tool = crate::providers::gemini::google_maps::GoogleMapsTool { lat_lng: None, enable_widget: false };
    /// assert_eq!(tool.tool_key(), "googleMaps");
    /// ```
    fn tool_key(&self) -> &'static str {
        "googleMaps"
    }

    /// Constructs the Google Maps tool declaration as a JSON value.
    ///
    /// The returned JSON contains a single key `"googleMaps"` whose value is an object
    /// that includes an `"enableWidget": true` field only when `enable_widget` is set.
    ///
    /// # Examples
    ///
    /// ```
    /// let tool = GoogleMapsTool { lat_lng: None, enable_widget: true };
    /// let decl = tool.to_tool_declaration();
    /// assert!(decl.get("googleMaps").is_some());
    /// assert_eq!(decl["googleMaps"]["enableWidget"], true);
    /// ```
    fn to_tool_declaration(&self) -> Value {
        let mut def = json!({});
        if self.enable_widget {
            def["enableWidget"] = json!(true);
        }
        json!({ "googleMaps": def })
    }

    /// Produces an optional tool configuration containing geographic coordinates.
    ///
    /// # Returns
    ///
    /// `Some((key, value))` where `key` is `"retrievalConfig"` and `value` is a JSON object
    /// with a `latLng` field containing `latitude` and `longitude` when `lat_lng` is set;
    /// `None` if `lat_lng` is `None`.
    ///
    /// # Examples
    ///
    /// ```
    /// use serde_json::json;
    ///
    /// let tool = crate::providers::gemini::google_maps::GoogleMapsTool {
    ///     lat_lng: Some((37.42_f32, -122.08_f32)),
    ///     enable_widget: false,
    /// };
    ///
    /// let cfg = tool.to_tool_config().unwrap();
    /// assert_eq!(cfg.0, "retrievalConfig");
    /// assert_eq!(cfg.1, json!({ "latLng": { "latitude": 37.42_f32, "longitude": -122.08_f32 } }));
    /// ```
    fn to_tool_config(&self) -> Option<(String, Value)> {
        self.lat_lng.map(|(lat, lng)| {
            (
                "retrievalConfig".to_string(),
                json!({
                    "latLng": {
                        "latitude": lat,
                        "longitude": lng
                    }
                }),
            )
        })
    }

    /// Creates a boxed clone of the tool as a `dyn GeminiNativeTool`.
    ///
    /// The returned value is an owned `Box` containing a clone of `self` and can be used where a
    /// trait object implementing `GeminiNativeTool` is required.
    ///
    /// # Examples
    ///
    /// ```
    /// let original = GoogleMapsTool { lat_lng: Some((37.422, -122.084)), enable_widget: true };
    /// let boxed = original.clone_box();
    /// assert_eq!(boxed.tool_key(), "googleMaps");
    /// ```
    fn clone_box(&self) -> Box<dyn GeminiNativeTool> {
        Box::new(self.clone())
    }
}
