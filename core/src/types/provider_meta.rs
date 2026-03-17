use std::any::Any;
use std::collections::HashMap;
use std::fmt;

pub struct ProviderMeta {
    pub description: Option<String>,
    pub data: HashMap<String, Box<dyn Any + Send + Sync>>,
}

impl Default for ProviderMeta {
    fn default() -> Self {
        Self {
            description: None,
            data: HashMap::new(),
        }
    }
}

impl fmt::Debug for ProviderMeta {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProviderMeta")
            .field("description", &self.description)
            .field("data_keys", &self.data.keys().collect::<Vec<_>>())
            .finish()
    }
}
