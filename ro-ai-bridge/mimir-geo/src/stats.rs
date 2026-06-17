//! Spatial statistics in pure Rust (offline, Rust-first). Global Moran's I and
//! mean nearest-neighbour distance. The harder ones (kriging, local LISA) stay for
//! the deferred Python/PySAL sandbox — once wheels can be vendored on the offline box.

use crate::error::{GeoError, Result};
use crate::spatial::geo_distance;

/// Global Moran's I over points with attached values, using **binary distance-band**
/// weights (w_ij = 1 if haversine(i,j) ≤ `threshold_m`, else 0; w_ii = 0).
/// ≈ +1 clustered, ≈ 0 random, ≈ −1 dispersed.
pub fn morans_i(points: &[(f64, f64)], values: &[f64], threshold_m: f64) -> Result<f64> {
    let n = points.len();
    if n < 2 || values.len() != n {
        return Err(GeoError::Stats("need ≥2 points and values.len()==points.len()".into()));
    }
    let mean = values.iter().sum::<f64>() / n as f64;
    let dev: Vec<f64> = values.iter().map(|v| v - mean).collect();
    let denom: f64 = dev.iter().map(|d| d * d).sum();
    if denom == 0.0 {
        return Err(GeoError::Stats("zero variance in values".into()));
    }
    let (mut num, mut w_sum) = (0.0_f64, 0.0_f64);
    for i in 0..n {
        for j in 0..n {
            if i != j && geo_distance(points[i], points[j]) <= threshold_m {
                num += dev[i] * dev[j];
                w_sum += 1.0;
            }
        }
    }
    if w_sum == 0.0 {
        return Err(GeoError::Stats("no neighbours within threshold_m — increase the threshold".into()));
    }
    Ok((n as f64 / w_sum) * (num / denom))
}

/// Mean nearest-neighbour distance (metres) — a point-pattern summary (compare to
/// the expected NN distance for CSR to gauge clustering, Clark–Evans style).
pub fn nn_mean_distance(points: &[(f64, f64)]) -> Result<f64> {
    let n = points.len();
    if n < 2 {
        return Err(GeoError::Stats("need ≥2 points".into()));
    }
    let mut total = 0.0;
    for i in 0..n {
        let mut best = f64::MAX;
        for j in 0..n {
            if i != j {
                best = best.min(geo_distance(points[i], points[j]));
            }
        }
        total += best;
    }
    Ok(total / n as f64)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn morans_i_clustered_is_positive() {
        // two tight clusters; values similar within a cluster, different across.
        // threshold 500m → only intra-cluster pairs are neighbours → perfect +autocorr.
        let pts = [(0.0, 0.0), (0.001, 0.0), (1.0, 1.0), (1.001, 1.0)];
        let vals = [1.0, 1.0, 5.0, 5.0];
        let i = morans_i(&pts, &vals, 500.0).unwrap();
        assert!((i - 1.0).abs() < 1e-6, "perfect clustering → I≈1, got {i}");
    }
    #[test]
    fn morans_i_complete_graph_is_minus_inv_n1() {
        // everyone a neighbour of everyone → I = -1/(n-1)
        let pts = [(0.0, 0.0), (0.0001, 0.0), (0.0, 0.0001), (0.0001, 0.0001)];
        let vals = [1.0, 2.0, 3.0, 4.0];
        let i = morans_i(&pts, &vals, 10_000.0).unwrap();
        assert!((i - (-1.0 / 3.0)).abs() < 1e-6, "complete graph → -1/3, got {i}");
    }
    #[test]
    fn nn_mean_positive() {
        let d = nn_mean_distance(&[(0.0, 0.0), (0.001, 0.0), (0.002, 0.0)]).unwrap();
        assert!(d > 50.0 && d < 200.0, "≈111m spacing, got {d}");
    }
    #[test]
    fn errors_on_bad_input() {
        assert!(morans_i(&[(0.0, 0.0)], &[1.0], 100.0).is_err());
        assert!(nn_mean_distance(&[(0.0, 0.0)]).is_err());
    }
}
