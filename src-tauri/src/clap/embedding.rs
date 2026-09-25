//! Embedding storage and similarity utilities

/// Convert embedding Vec<f32> to BLOB bytes for SQLite storage
pub fn embedding_to_blob(embedding: &[f32]) -> Vec<u8> {
    embedding.iter().flat_map(|f| f.to_le_bytes()).collect()
}

/// Convert BLOB bytes back to Vec<f32>
pub fn blob_to_embedding(blob: &[u8]) -> Vec<f32> {
    blob.chunks_exact(4)
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .collect()
}

/// Cosine similarity between two embeddings
/// Note: CLAP embeddings are L2-normalized, so dot product = cosine similarity
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

/// Score an embedding against several query embeddings (alternatives, OR): the best
/// similarity and which query produced it. `queries` must not be empty.
pub fn best_similarity(queries: &[Vec<f32>], emb: &[f32]) -> (usize, f32) {
    queries
        .iter()
        .map(|q| cosine_similarity(q, emb))
        .enumerate()
        .max_by(|(_, a), (_, b)| a.total_cmp(b))
        .expect("at least one query embedding")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedding_roundtrip() {
        let embedding: Vec<f32> = vec![0.1, 0.2, -0.3, 0.4];
        let blob = embedding_to_blob(&embedding);
        let restored = blob_to_embedding(&blob);

        assert_eq!(embedding.len(), restored.len());
        for (a, b) in embedding.iter().zip(restored.iter()) {
            assert!((a - b).abs() < 1e-6);
        }
    }

    #[test]
    fn test_best_similarity_takes_the_closest_alternative() {
        let queries = vec![
            vec![1.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0],
            vec![0.0, 0.0, 1.0],
        ];
        let (index, sim) = best_similarity(&queries, &[0.1, 0.8, 0.6]);
        assert_eq!(index, 1);
        assert!((sim - 0.8).abs() < 1e-6);
        // A single query is plain cosine similarity
        let (index, sim) = best_similarity(&queries[..1], &[0.6, 0.8, 0.0]);
        assert_eq!(index, 0);
        assert!((sim - 0.6).abs() < 1e-6);
    }

    #[test]
    fn test_cosine_similarity() {
        // Identical normalized vectors should have similarity 1.0
        let a: Vec<f32> = vec![1.0, 0.0, 0.0];
        let b: Vec<f32> = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 1e-6);

        // Orthogonal vectors should have similarity 0.0
        let c: Vec<f32> = vec![0.0, 1.0, 0.0];
        assert!(cosine_similarity(&a, &c).abs() < 1e-6);
    }
}
