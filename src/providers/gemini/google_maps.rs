use serde_json::{Value, json};

use crate::gemini::lib::GeminiNativeTool;

#[derive(Clone)]
pub struct GoogleMapsTool {
    pub lat_lng: Option<(f32, f32)>,
    pub enable_widget: bool,
}

impl GeminiNativeTool for GoogleMapsTool {
    fn tool_key(&self) -> &'static str {
        "googleMaps"
    }

    fn to_tool_declaration(&self) -> Value {
        let mut def = json!({});
        if self.enable_widget {
            def["enableWidget"] = json!(true);
        }
        json!({ "googleMaps": def })
    }

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

    fn clone_box(&self) -> Box<dyn GeminiNativeTool> {
        Box::new(self.clone())
    }
}
