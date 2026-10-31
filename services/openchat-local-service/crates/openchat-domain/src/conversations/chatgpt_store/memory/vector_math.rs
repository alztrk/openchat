//! High-performance unrolled vector math and quantization routines.
//!
//! Designed for semantic memory retrieval over 10,000+ candidate vectors.
//! Uses multiple accumulator registers and unrolled loop bodies (8-wide for f32,
//! 16-wide for i8) to maximize instruction-level parallelism (ILP) and enable
//! automatic SIMD vectorization across AVX2, SSE4.1, and ARM NEON.
#![allow(dead_code)]

/// Quantizes an f32 embedding vector into an i8 vector using max-abs scaling.
///
/// Scans the slice in 8-element unrolled chunks to validate finiteness and find
/// the maximum absolute value in a single pass, then normalizes using a single
/// multiplicative scale factor.
pub fn quantize_embedding_unrolled(
    embedding: &[f32],
    expected_dimensions: usize,
) -> Result<Vec<i8>, String> {
    if embedding.len() != expected_dimensions {
        return Err(format!(
            "expected {expected_dimensions} dimensions, received {}",
            embedding.len()
        ));
    }

    let mut max_abs = 0.0_f32;
    let chunks = embedding.chunks_exact(8);
    let remainder = chunks.remainder();

    for chunk in chunks {
        let v0 = chunk[0];
        let v1 = chunk[1];
        let v2 = chunk[2];
        let v3 = chunk[3];
        let v4 = chunk[4];
        let v5 = chunk[5];
        let v6 = chunk[6];
        let v7 = chunk[7];

        if !v0.is_finite()
            || !v1.is_finite()
            || !v2.is_finite()
            || !v3.is_finite()
            || !v4.is_finite()
            || !v5.is_finite()
            || !v6.is_finite()
            || !v7.is_finite()
        {
            return Err("embedding contains a non-finite value".to_owned());
        }

        let m01 = v0.abs().max(v1.abs());
        let m23 = v2.abs().max(v3.abs());
        let m45 = v4.abs().max(v5.abs());
        let m67 = v6.abs().max(v7.abs());
        let chunk_max = m01.max(m23).max(m45.max(m67));
        max_abs = max_abs.max(chunk_max);
    }

    for &value in remainder {
        if !value.is_finite() {
            return Err("embedding contains a non-finite value".to_owned());
        }
        max_abs = max_abs.max(value.abs());
    }

    if !max_abs.is_finite() || max_abs == 0.0 {
        return Err("embedding is not a finite non-zero vector".to_owned());
    }

    let scale = i8::MAX as f32 / max_abs;
    let mut quantized = Vec::with_capacity(expected_dimensions);

    let chunks = embedding.chunks_exact(8);
    let remainder = chunks.remainder();

    for chunk in chunks {
        quantized.push((chunk[0] * scale).round() as i8);
        quantized.push((chunk[1] * scale).round() as i8);
        quantized.push((chunk[2] * scale).round() as i8);
        quantized.push((chunk[3] * scale).round() as i8);
        quantized.push((chunk[4] * scale).round() as i8);
        quantized.push((chunk[5] * scale).round() as i8);
        quantized.push((chunk[6] * scale).round() as i8);
        quantized.push((chunk[7] * scale).round() as i8);
    }

    for &value in remainder {
        quantized.push((value * scale).round() as i8);
    }

    Ok(quantized)
}

/// Computes the dot product of two i8 slices with 16-way unrolling and 4 independent accumulators.
pub fn dot_product_i8(a: &[i8], b: &[i8]) -> i32 {
    debug_assert_eq!(a.len(), b.len(), "vector dimensions must match");
    let len = a.len().min(b.len());
    let a_slice = &a[..len];
    let b_slice = &b[..len];

    let mut acc0 = 0_i32;
    let mut acc1 = 0_i32;
    let mut acc2 = 0_i32;
    let mut acc3 = 0_i32;

    let chunks_a = a_slice.chunks_exact(16);
    let chunks_b = b_slice.chunks_exact(16);
    let remainder_a = chunks_a.remainder();
    let remainder_b = chunks_b.remainder();

    for (ca, cb) in chunks_a.zip(chunks_b) {
        acc0 += (ca[0] as i32 * cb[0] as i32)
            + (ca[1] as i32 * cb[1] as i32)
            + (ca[2] as i32 * cb[2] as i32)
            + (ca[3] as i32 * cb[3] as i32);

        acc1 += (ca[4] as i32 * cb[4] as i32)
            + (ca[5] as i32 * cb[5] as i32)
            + (ca[6] as i32 * cb[6] as i32)
            + (ca[7] as i32 * cb[7] as i32);

        acc2 += (ca[8] as i32 * cb[8] as i32)
            + (ca[9] as i32 * cb[9] as i32)
            + (ca[10] as i32 * cb[10] as i32)
            + (ca[11] as i32 * cb[11] as i32);

        acc3 += (ca[12] as i32 * cb[12] as i32)
            + (ca[13] as i32 * cb[13] as i32)
            + (ca[14] as i32 * cb[14] as i32)
            + (ca[15] as i32 * cb[15] as i32);
    }

    let mut total = (acc0 + acc1) + (acc2 + acc3);
    for (&va, &vb) in remainder_a.iter().zip(remainder_b) {
        total += va as i32 * vb as i32;
    }

    total
}

/// Computes the dot product of two f32 slices with 8-way unrolling and 4 independent accumulators.
pub fn dot_product_f32(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len(), "vector dimensions must match");
    let len = a.len().min(b.len());
    let a_slice = &a[..len];
    let b_slice = &b[..len];

    let mut acc0 = 0.0_f32;
    let mut acc1 = 0.0_f32;
    let mut acc2 = 0.0_f32;
    let mut acc3 = 0.0_f32;

    let chunks_a = a_slice.chunks_exact(8);
    let chunks_b = b_slice.chunks_exact(8);
    let remainder_a = chunks_a.remainder();
    let remainder_b = chunks_b.remainder();

    for (ca, cb) in chunks_a.zip(chunks_b) {
        acc0 += ca[0] * cb[0] + ca[1] * cb[1];
        acc1 += ca[2] * cb[2] + ca[3] * cb[3];
        acc2 += ca[4] * cb[4] + ca[5] * cb[5];
        acc3 += ca[6] * cb[6] + ca[7] * cb[7];
    }

    let mut total = (acc0 + acc1) + (acc2 + acc3);
    for (&va, &vb) in remainder_a.iter().zip(remainder_b) {
        total += va * vb;
    }

    total
}

/// Computes the cosine similarity of two f32 vectors in a single unrolled pass.
pub fn cosine_similarity_f32(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len(), "vector dimensions must match");
    let len = a.len().min(b.len());
    let a_slice = &a[..len];
    let b_slice = &b[..len];

    let mut dot = 0.0_f32;
    let mut norm_a_sq = 0.0_f32;
    let mut norm_b_sq = 0.0_f32;

    let chunks_a = a_slice.chunks_exact(4);
    let chunks_b = b_slice.chunks_exact(4);
    let remainder_a = chunks_a.remainder();
    let remainder_b = chunks_b.remainder();

    for (ca, cb) in chunks_a.zip(chunks_b) {
        dot += ca[0] * cb[0] + ca[1] * cb[1] + ca[2] * cb[2] + ca[3] * cb[3];
        norm_a_sq += ca[0] * ca[0] + ca[1] * ca[1] + ca[2] * ca[2] + ca[3] * ca[3];
        norm_b_sq += cb[0] * cb[0] + cb[1] * cb[1] + cb[2] * cb[2] + cb[3] * cb[3];
    }

    for (&va, &vb) in remainder_a.iter().zip(remainder_b) {
        dot += va * vb;
        norm_a_sq += va * va;
        norm_b_sq += vb * vb;
    }

    let denominator = (norm_a_sq * norm_b_sq).sqrt();
    if denominator > 0.0 {
        (dot / denominator).clamp(-1.0, 1.0)
    } else {
        0.0
    }
}

/// Computes the cosine similarity of two quantized i8 vectors.
pub fn cosine_similarity_i8(a: &[i8], b: &[i8]) -> f32 {
    let dot = dot_product_i8(a, b);
    let norm_a_sq = dot_product_i8(a, a);
    let norm_b_sq = dot_product_i8(b, b);

    let denominator = (norm_a_sq as f64 * norm_b_sq as f64).sqrt();
    if denominator > 0.0 {
        ((dot as f64) / denominator).clamp(-1.0, 1.0) as f32
    } else {
        0.0
    }
}

/// Computes the squared Euclidean (L2) distance between two f32 slices with 8-way unrolling.
pub fn euclidean_distance_squared_f32(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len(), "vector dimensions must match");
    let len = a.len().min(b.len());
    let a_slice = &a[..len];
    let b_slice = &b[..len];

    let mut dist0 = 0.0_f32;
    let mut dist1 = 0.0_f32;
    let mut dist2 = 0.0_f32;
    let mut dist3 = 0.0_f32;

    let chunks_a = a_slice.chunks_exact(8);
    let chunks_b = b_slice.chunks_exact(8);
    let remainder_a = chunks_a.remainder();
    let remainder_b = chunks_b.remainder();

    for (ca, cb) in chunks_a.zip(chunks_b) {
        let d0 = ca[0] - cb[0];
        let d1 = ca[1] - cb[1];
        let d2 = ca[2] - cb[2];
        let d3 = ca[3] - cb[3];
        let d4 = ca[4] - cb[4];
        let d5 = ca[5] - cb[5];
        let d6 = ca[6] - cb[6];
        let d7 = ca[7] - cb[7];

        dist0 += d0 * d0 + d1 * d1;
        dist1 += d2 * d2 + d3 * d3;
        dist2 += d4 * d4 + d5 * d5;
        dist3 += d6 * d6 + d7 * d7;
    }

    let mut total = (dist0 + dist1) + (dist2 + dist3);
    for (&va, &vb) in remainder_a.iter().zip(remainder_b) {
        let diff = va - vb;
        total += diff * diff;
    }

    total
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quantize_embedding_unrolled_matches_exact_scaling() {
        let mut embedding = vec![0.0_f32; 384];
        embedding[0] = 0.25;
        embedding[1] = -0.125;

        let quantized = quantize_embedding_unrolled(&embedding, 384).expect("quantize");
        assert_eq!(quantized[0], 127);
        assert_eq!(quantized[1], -64);
        assert_eq!(quantized.len(), 384);

        assert!(quantize_embedding_unrolled(&vec![0.0; 384], 384).is_err());

        for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut bad = vec![0.0; 384];
            bad[383] = invalid;
            assert!(quantize_embedding_unrolled(&bad, 384).is_err());
        }
    }

    #[test]
    fn test_dot_product_i8() {
        let a = vec![2_i8; 384];
        let b = vec![3_i8; 384];
        let dot = dot_product_i8(&a, &b);
        assert_eq!(dot, 2 * 3 * 384);
    }

    #[test]
    fn test_dot_product_f32() {
        let a = vec![0.5_f32; 384];
        let b = vec![2.0_f32; 384];
        let dot = dot_product_f32(&a, &b);
        assert!((dot - 384.0).abs() < 1e-4);
    }

    #[test]
    fn test_cosine_similarity_identical_and_orthogonal() {
        let a = vec![1.0_f32; 384];
        let sim = cosine_similarity_f32(&a, &a);
        assert!((sim - 1.0).abs() < 1e-4);

        let mut b = vec![0.0_f32; 384];
        b[0] = 1.0;
        let mut c = vec![0.0_f32; 384];
        c[1] = 1.0;
        let sim_ortho = cosine_similarity_f32(&b, &c);
        assert!((sim_ortho - 0.0).abs() < 1e-4);
    }

    #[test]
    fn test_benchmark_10k_vector_dot_product() {
        let query = vec![1_i8; 384];
        let candidate = vec![2_i8; 384];

        let start = std::time::Instant::now();
        let mut sum = 0_i64;
        for _ in 0..10_000 {
            sum += dot_product_i8(&query, &candidate) as i64;
        }
        let elapsed = start.elapsed();
        assert_eq!(sum, 10_000 * 2 * 384);
        eprintln!("10,000 i8 dot products (384 dimensions) took: {elapsed:?}");
    }
}
