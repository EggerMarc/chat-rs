use serde::Serialize;
use serde_json::{Value, json};

use crate::tools::OpenAINativeTool;

#[derive(Serialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum ImageSize {
    #[serde(rename = "1024x1024")]
    Square,
    #[serde(rename = "1024x1536")]
    Portrait,
    #[serde(rename = "1536x1024")]
    Landscape,
    Auto,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum ImageQuality {
    Low,
    Medium,
    High,
    Auto,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum ImageBackground {
    Transparent,
    Opaque,
    Auto,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "lowercase")]
pub enum ImageOutputFormat {
    Png,
    Jpeg,
    Webp,
}

#[derive(Default, Clone)]
pub struct ImageGenerationTool {
    pub size: Option<ImageSize>,
    pub quality: Option<ImageQuality>,
    pub background: Option<ImageBackground>,
    pub output_format: Option<ImageOutputFormat>,
    pub n: Option<u32>,
}

impl OpenAINativeTool for ImageGenerationTool {
    fn tool_key(&self) -> &'static str {
        "image_generation"
    }

    fn to_tool_declaration(&self) -> Value {
        let mut decl = json!({ "type": "image_generation" });
        let map = decl.as_object_mut().unwrap();
        if let Some(s) = &self.size {
            map.insert("size".into(), serde_json::to_value(s).unwrap());
        }
        if let Some(q) = &self.quality {
            map.insert("quality".into(), serde_json::to_value(q).unwrap());
        }
        if let Some(b) = &self.background {
            map.insert("background".into(), serde_json::to_value(b).unwrap());
        }
        if let Some(f) = &self.output_format {
            map.insert("output_format".into(), serde_json::to_value(f).unwrap());
        }
        if let Some(n) = self.n {
            map.insert("n".into(), json!(n));
        }
        decl
    }

    fn to_tool_config(&self) -> Option<(String, Value)> {
        None
    }

    fn clone_box(&self) -> Box<dyn OpenAINativeTool> {
        Box::new(self.clone())
    }
}
