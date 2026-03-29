// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) OwnPulse Contributors

//! Statistical computation.
//!
//! Pearson correlation, Spearman correlation, Welch's t-test, and basic
//! descriptive statistics. Used by the correlation endpoints (Phase 3).
//! Pure functions — no database access.

use statrs::distribution::{ContinuousCDF, StudentsT};

/// Compute the arithmetic mean of a slice.
///
/// Returns `f64::NAN` if the slice is empty.
pub fn mean(data: &[f64]) -> f64 {
    if data.is_empty() {
        return f64::NAN;
    }
    data.iter().sum::<f64>() / data.len() as f64
}

/// Compute the population standard deviation.
///
/// Returns `f64::NAN` if the slice is empty.
pub fn std_dev(data: &[f64]) -> f64 {
    if data.is_empty() {
        return f64::NAN;
    }
    let m = mean(data);
    let variance = data.iter().map(|x| (x - m).powi(2)).sum::<f64>() / data.len() as f64;
    variance.sqrt()
}

/// Sample standard deviation (Bessel-corrected, divides by n-1).
///
/// Returns `f64::NAN` if fewer than 2 values.
fn sample_std_dev(data: &[f64]) -> f64 {
    if data.len() < 2 {
        return f64::NAN;
    }
    let m = mean(data);
    let variance = data.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (data.len() - 1) as f64;
    variance.sqrt()
}

/// Pearson correlation coefficient.
///
/// Returns `(r, p_value, n)`. The p-value is computed using the t-distribution.
/// If fewer than 3 paired observations, returns `(NAN, NAN, n)`.
///
/// # Panics
///
/// Panics if `x.len() != y.len()`.
pub fn pearson(x: &[f64], y: &[f64]) -> (f64, f64, usize) {
    assert_eq!(x.len(), y.len(), "x and y must have the same length");
    let n = x.len();
    if n < 3 {
        return (f64::NAN, f64::NAN, n);
    }

    let mx = mean(x);
    let my = mean(y);

    let mut sum_xy = 0.0;
    let mut sum_xx = 0.0;
    let mut sum_yy = 0.0;

    for i in 0..n {
        let dx = x[i] - mx;
        let dy = y[i] - my;
        sum_xy += dx * dy;
        sum_xx += dx * dx;
        sum_yy += dy * dy;
    }

    let denom = (sum_xx * sum_yy).sqrt();
    if denom == 0.0 {
        // All values in one or both series are identical — no correlation computable.
        return (f64::NAN, f64::NAN, n);
    }

    let r = sum_xy / denom;
    // Clamp to [-1, 1] to handle floating-point imprecision.
    let r = r.clamp(-1.0, 1.0);

    let p = p_value_from_r(r, n);
    (r, p, n)
}

/// Spearman rank correlation coefficient.
///
/// Rank-transforms both series and computes Pearson on the ranks.
/// Ties are assigned the average rank.
///
/// Returns `(rho, p_value, n)`.
///
/// # Panics
///
/// Panics if `x.len() != y.len()`.
pub fn spearman(x: &[f64], y: &[f64]) -> (f64, f64, usize) {
    assert_eq!(x.len(), y.len(), "x and y must have the same length");
    let rx = rank(x);
    let ry = rank(y);
    pearson(&rx, &ry)
}

/// Welch's t-test for two independent samples with unequal variances.
///
/// Returns `(t_statistic, p_value, degrees_of_freedom)`.
/// If either sample has fewer than 2 observations, returns `(NAN, NAN, NAN)`.
pub fn welch_t_test(a: &[f64], b: &[f64]) -> (f64, f64, f64) {
    if a.len() < 2 || b.len() < 2 {
        return (f64::NAN, f64::NAN, f64::NAN);
    }

    let n1 = a.len() as f64;
    let n2 = b.len() as f64;
    let m1 = mean(a);
    let m2 = mean(b);
    let s1 = sample_std_dev(a);
    let s2 = sample_std_dev(b);

    let s1_sq_n1 = s1 * s1 / n1;
    let s2_sq_n2 = s2 * s2 / n2;
    let se = (s1_sq_n1 + s2_sq_n2).sqrt();

    if se == 0.0 {
        // Both samples have zero variance — means are either identical or not.
        if (m1 - m2).abs() < f64::EPSILON {
            return (0.0, 1.0, n1 + n2 - 2.0);
        }
        return (f64::INFINITY, 0.0, n1 + n2 - 2.0);
    }

    let t = (m1 - m2) / se;

    // Welch-Satterthwaite degrees of freedom
    let df_num = (s1_sq_n1 + s2_sq_n2).powi(2);
    let df_den = (s1_sq_n1.powi(2) / (n1 - 1.0)) + (s2_sq_n2.powi(2) / (n2 - 1.0));
    let df = df_num / df_den;

    let p = if df > 0.0 && df.is_finite() {
        match StudentsT::new(0.0, 1.0, df) {
            Ok(dist) => 2.0 * (1.0 - dist.cdf(t.abs())),
            Err(_) => f64::NAN,
        }
    } else {
        f64::NAN
    };

    (t, p, df)
}

/// Interpret the strength and direction of a correlation coefficient.
pub fn interpret_correlation(r: f64) -> &'static str {
    if r.is_nan() {
        return "insufficient data";
    }
    let abs_r = r.abs();
    let strength = if abs_r < 0.3 {
        "weak"
    } else if abs_r <= 0.7 {
        "moderate"
    } else {
        "strong"
    };
    let direction = if r >= 0.0 { "positive" } else { "negative" };
    match (strength, direction) {
        ("weak", "positive") => "weak positive",
        ("weak", "negative") => "weak negative",
        ("moderate", "positive") => "moderate positive",
        ("moderate", "negative") => "moderate negative",
        ("strong", "positive") => "strong positive",
        ("strong", "negative") => "strong negative",
        _ => "insufficient data",
    }
}

// ---- internal helpers ----

/// Compute the two-tailed p-value from Pearson r and sample size n.
fn p_value_from_r(r: f64, n: usize) -> f64 {
    if n < 3 {
        return f64::NAN;
    }
    let df = (n - 2) as f64;
    let r_sq = r * r;
    if r_sq >= 1.0 {
        return 0.0;
    }
    let t = r * (df / (1.0 - r_sq)).sqrt();
    match StudentsT::new(0.0, 1.0, df) {
        Ok(dist) => 2.0 * (1.0 - dist.cdf(t.abs())),
        Err(_) => f64::NAN,
    }
}

/// Assign average ranks to values. Ties receive the mean of their positions.
fn rank(data: &[f64]) -> Vec<f64> {
    let n = data.len();
    let mut indexed: Vec<(usize, f64)> = data.iter().copied().enumerate().collect();
    indexed.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

    let mut ranks = vec![0.0; n];
    let mut i = 0;
    while i < n {
        let mut j = i;
        while j < n && (indexed[j].1 - indexed[i].1).abs() < f64::EPSILON {
            j += 1;
        }
        // Average rank for tied values (1-based)
        let avg_rank = (i + 1..=j).map(|r| r as f64).sum::<f64>() / (j - i) as f64;
        for k in i..j {
            ranks[indexed[k].0] = avg_rank;
        }
        i = j;
    }
    ranks
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f64 = 1e-6;

    #[test]
    fn mean_basic() {
        assert!((mean(&[1.0, 2.0, 3.0, 4.0, 5.0]) - 3.0).abs() < EPSILON);
    }

    #[test]
    fn mean_empty() {
        assert!(mean(&[]).is_nan());
    }

    #[test]
    fn mean_single() {
        assert!((mean(&[42.0]) - 42.0).abs() < EPSILON);
    }

    #[test]
    fn std_dev_basic() {
        // Population std dev of [2, 4, 4, 4, 5, 5, 7, 9] = 2.0
        let data = [2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];
        assert!((std_dev(&data) - 2.0).abs() < EPSILON);
    }

    #[test]
    fn std_dev_empty() {
        assert!(std_dev(&[]).is_nan());
    }

    #[test]
    fn std_dev_all_same() {
        assert!((std_dev(&[5.0, 5.0, 5.0]) - 0.0).abs() < EPSILON);
    }

    #[test]
    fn sample_std_dev_basic() {
        // Sample std dev of [2, 4, 4, 4, 5, 5, 7, 9]
        // Mean = 5.0, sum of sq diffs = 32.0, variance = 32/7, sd ~= 2.1380899
        let data = [2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0];
        let expected = (32.0_f64 / 7.0).sqrt();
        assert!((sample_std_dev(&data) - expected).abs() < EPSILON);
    }

    #[test]
    fn sample_std_dev_too_few() {
        assert!(sample_std_dev(&[1.0]).is_nan());
        assert!(sample_std_dev(&[]).is_nan());
    }

    #[test]
    fn pearson_perfect_positive() {
        let x = [1.0, 2.0, 3.0, 4.0, 5.0];
        let y = [2.0, 4.0, 6.0, 8.0, 10.0];
        let (r, p, n) = pearson(&x, &y);
        assert!((r - 1.0).abs() < EPSILON);
        assert!(p < 0.01);
        assert_eq!(n, 5);
    }

    #[test]
    fn pearson_perfect_negative() {
        let x = [1.0, 2.0, 3.0, 4.0, 5.0];
        let y = [10.0, 8.0, 6.0, 4.0, 2.0];
        let (r, _p, n) = pearson(&x, &y);
        assert!((r - (-1.0)).abs() < EPSILON);
        assert_eq!(n, 5);
    }

    #[test]
    fn pearson_uncorrelated() {
        let x = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let y = [2.0, 8.0, 1.0, 7.0, 3.0, 6.0, 4.0, 5.0];
        let (r, _p, _n) = pearson(&x, &y);
        assert!(r.abs() < 0.5);
    }

    #[test]
    fn pearson_too_few_points() {
        let x = [1.0, 2.0];
        let y = [3.0, 4.0];
        let (r, p, n) = pearson(&x, &y);
        assert!(r.is_nan());
        assert!(p.is_nan());
        assert_eq!(n, 2);
    }

    #[test]
    fn pearson_all_same_values() {
        let x = [5.0, 5.0, 5.0, 5.0];
        let y = [3.0, 3.0, 3.0, 3.0];
        let (r, _p, _n) = pearson(&x, &y);
        assert!(r.is_nan());
    }

    #[test]
    fn spearman_monotonic() {
        let x = [1.0, 2.0, 3.0, 4.0, 5.0];
        let y = [10.0, 20.0, 30.0, 40.0, 50.0];
        let (r, _p, n) = spearman(&x, &y);
        assert!((r - 1.0).abs() < EPSILON);
        assert_eq!(n, 5);
    }

    #[test]
    fn spearman_with_ties() {
        let x = [1.0, 2.0, 2.0, 3.0];
        let y = [10.0, 20.0, 20.0, 30.0];
        let (r, _p, _n) = spearman(&x, &y);
        assert!((r - 1.0).abs() < EPSILON);
    }

    #[test]
    fn spearman_nonlinear_monotonic() {
        // Spearman should give 1.0 for any monotonically increasing relationship
        let x = [1.0, 2.0, 3.0, 4.0, 5.0];
        let y = [1.0, 4.0, 9.0, 16.0, 25.0]; // y = x^2
        let (r, _p, _n) = spearman(&x, &y);
        assert!((r - 1.0).abs() < EPSILON);
    }

    #[test]
    fn welch_significantly_different() {
        let a = [10.0, 11.0, 12.0, 13.0, 14.0, 15.0, 16.0, 17.0, 18.0, 19.0];
        let b = [50.0, 51.0, 52.0, 53.0, 54.0, 55.0, 56.0, 57.0, 58.0, 59.0];
        let (_t, p, _df) = welch_t_test(&a, &b);
        assert!(p < 0.001, "p = {p}, expected < 0.001");
    }

    #[test]
    fn welch_same_distribution() {
        let a = [10.0, 11.0, 12.0, 13.0, 14.0];
        let b = [10.5, 11.5, 12.5, 13.5, 14.5];
        let (_t, p, _df) = welch_t_test(&a, &b);
        assert!(p > 0.05, "p = {p}, expected > 0.05");
    }

    #[test]
    fn welch_too_few_observations() {
        let a = [10.0];
        let b = [50.0, 51.0, 52.0];
        let (t, p, df) = welch_t_test(&a, &b);
        assert!(t.is_nan());
        assert!(p.is_nan());
        assert!(df.is_nan());
    }

    #[test]
    fn welch_identical_means() {
        let a = [5.0, 5.0, 5.0, 5.0, 5.0];
        let b = [5.0, 5.0, 5.0, 5.0, 5.0];
        let (t, p, _df) = welch_t_test(&a, &b);
        assert!((t - 0.0).abs() < EPSILON);
        assert!((p - 1.0).abs() < EPSILON);
    }

    #[test]
    fn welch_unequal_sizes() {
        let a = [1.0, 2.0, 3.0];
        let b = [100.0, 101.0, 102.0, 103.0, 104.0, 105.0, 106.0, 107.0];
        let (_t, p, _df) = welch_t_test(&a, &b);
        assert!(p < 0.001, "p = {p}, expected < 0.001");
    }

    #[test]
    fn interpret_strong_positive() {
        assert_eq!(interpret_correlation(0.85), "strong positive");
    }

    #[test]
    fn interpret_moderate_negative() {
        assert_eq!(interpret_correlation(-0.5), "moderate negative");
    }

    #[test]
    fn interpret_weak_positive() {
        assert_eq!(interpret_correlation(0.1), "weak positive");
    }

    #[test]
    fn interpret_nan() {
        assert_eq!(interpret_correlation(f64::NAN), "insufficient data");
    }

    #[test]
    fn interpret_zero() {
        assert_eq!(interpret_correlation(0.0), "weak positive");
    }

    #[test]
    fn interpret_boundary_03() {
        assert_eq!(interpret_correlation(0.3), "moderate positive");
    }

    #[test]
    fn interpret_boundary_07() {
        assert_eq!(interpret_correlation(0.7), "moderate positive");
    }

    #[test]
    fn interpret_boundary_071() {
        assert_eq!(interpret_correlation(0.71), "strong positive");
    }

    #[test]
    fn rank_basic() {
        let data = [3.0, 1.0, 2.0];
        let r = rank(&data);
        assert!((r[0] - 3.0).abs() < EPSILON); // 3.0 is rank 3
        assert!((r[1] - 1.0).abs() < EPSILON); // 1.0 is rank 1
        assert!((r[2] - 2.0).abs() < EPSILON); // 2.0 is rank 2
    }

    #[test]
    fn rank_ties() {
        let data = [1.0, 2.0, 2.0, 4.0];
        let r = rank(&data);
        assert!((r[0] - 1.0).abs() < EPSILON);
        assert!((r[1] - 2.5).abs() < EPSILON); // tied for ranks 2 and 3
        assert!((r[2] - 2.5).abs() < EPSILON);
        assert!((r[3] - 4.0).abs() < EPSILON);
    }

    #[test]
    fn rank_all_same() {
        let data = [5.0, 5.0, 5.0];
        let r = rank(&data);
        // All tied, average of ranks 1,2,3 = 2.0
        for val in &r {
            assert!((val - 2.0).abs() < EPSILON);
        }
    }

    #[test]
    fn p_value_from_r_edge_cases() {
        // r = 1.0 should give p = 0.0
        assert!((p_value_from_r(1.0, 10) - 0.0).abs() < EPSILON);
        // r = 0.0 with decent n should give p = 1.0
        assert!((p_value_from_r(0.0, 100) - 1.0).abs() < 0.01);
        // too few points
        assert!(p_value_from_r(0.5, 2).is_nan());
    }
}
