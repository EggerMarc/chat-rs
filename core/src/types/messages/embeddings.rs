use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Embeddings {
    pub content: Vec<f32>,
    pub dimension: usize,
}

impl Embeddings {
    pub fn new(data: Vec<f32>) -> Self {
        let len = data.len();
        Embeddings {
            content: data,
            dimension: len,
        }
    }

    pub fn from_batch(batch: Vec<Vec<f32>>) -> Vec<Self> {
        batch
            .into_iter()
            .map(Embeddings::new)
            .collect::<Vec<Self>>()
    }
}

impl From<Vec<f32>> for Embeddings {
    fn from(data: Vec<f32>) -> Self {
        Self::new(data)
    }
}
