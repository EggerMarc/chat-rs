use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Embeddings {
    pub content: Vec<f32>,
    pub dimension: usize,
}

impl Embeddings {
    /// Create an `Embeddings` instance from a flat vector of `f32`.
    ///
    /// The provided vector is stored in `content` and `dimension` is set to the vector's length.
    ///
    /// # Examples
    ///
    /// ```
    /// let e = Embeddings::from(vec![0.1_f32, 0.2, 0.3]);
    /// assert_eq!(e.content, vec![0.1_f32, 0.2, 0.3]);
    /// assert_eq!(e.dimension, 3);
    /// ```
    pub fn from(data: Vec<f32>) -> Self {
        let len = data.len();
        Embeddings {
            content: data,
            dimension: len,
        }
    }

    /// Converts a batch of embedding vectors into a vector of `Embeddings`.
    ///
    /// Each inner `Vec<f32>` is turned into an `Embeddings` where `content` is the vector and `dimension` is its length.
    ///
    /// # Examples
    ///
    /// ```
    /// let batch = vec![vec![0.1f32, 0.2], vec![0.3, 0.4, 0.5]];
    /// let embeddings = Embeddings::from_batch(batch);
    /// assert_eq!(embeddings[0].content, vec![0.1f32, 0.2]);
    /// assert_eq!(embeddings[0].dimension, 2);
    /// assert_eq!(embeddings[1].dimension, 3);
    /// ```
    pub fn from_batch(batch: Vec<Vec<f32>>) -> Vec<Self> {
        batch
            .iter()
            .map(|value| Embeddings::from(value.to_owned()))
            .collect::<Vec<Embeddings>>()
    }
}
