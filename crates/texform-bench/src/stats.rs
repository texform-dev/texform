use std::time::Duration;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TimingStats {
    pub mean: f64,
    pub p50: f64,
    pub p95: f64,
    pub p99: f64,
    pub max: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_formula_id: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ModeStats {
    pub ok: usize,
    pub failed: usize,
    pub failure_rate_pct: f64,
    pub timing_ms: TimingStats,
}

pub fn compute_mode_stats(durations: &[Duration], oks: &[bool]) -> ModeStats {
    debug_assert_eq!(durations.len(), oks.len());

    let total = durations.len();
    let ok = oks.iter().filter(|&&value| value).count();
    let failed = total.saturating_sub(ok);

    let mut timing_ms: Vec<f64> = durations
        .iter()
        .map(|duration| duration.as_secs_f64() * 1_000.0)
        .collect();
    timing_ms.sort_by(|left, right| left.partial_cmp(right).unwrap());

    ModeStats {
        ok,
        failed,
        failure_rate_pct: if total == 0 {
            0.0
        } else {
            failed as f64 / total as f64 * 100.0
        },
        timing_ms: TimingStats {
            mean: if total == 0 {
                0.0
            } else {
                timing_ms.iter().sum::<f64>() / total as f64
            },
            p50: percentile(&timing_ms, 50.0),
            p95: percentile(&timing_ms, 95.0),
            p99: percentile(&timing_ms, 99.0),
            max: timing_ms.last().copied().unwrap_or(0.0),
            max_formula_id: None,
        },
    }
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }

    let rank = ((p.clamp(0.0, 100.0) / 100.0) * sorted.len() as f64).ceil() as usize;
    let index = rank.saturating_sub(1).min(sorted.len() - 1);
    sorted[index]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn percentile_basic() {
        let data: Vec<f64> = (1..=100).map(|value| value as f64).collect();
        assert_eq!(percentile(&data, 50.0), 50.0);
        assert_eq!(percentile(&data, 95.0), 95.0);
        assert_eq!(percentile(&data, 99.0), 99.0);
        assert_eq!(percentile(&data, 100.0), 100.0);
    }

    #[test]
    fn percentile_empty() {
        assert_eq!(percentile(&[], 50.0), 0.0);
    }

    #[test]
    fn compute_mode_stats_basic() {
        let durations = vec![
            Duration::from_micros(10),
            Duration::from_micros(20),
            Duration::from_micros(30),
        ];
        let oks = vec![true, true, false];

        let stats = compute_mode_stats(&durations, &oks);

        assert_eq!(stats.ok, 2);
        assert_eq!(stats.failed, 1);
        assert!((stats.failure_rate_pct - 33.333).abs() < 0.01);
        assert!((stats.timing_ms.mean - 0.02).abs() < 1e-9);
        assert!((stats.timing_ms.p50 - 0.02).abs() < 1e-9);
        assert!((stats.timing_ms.p95 - 0.03).abs() < 1e-9);
        assert!((stats.timing_ms.p99 - 0.03).abs() < 1e-9);
        assert!((stats.timing_ms.max - 0.03).abs() < 1e-9);
        assert_eq!(stats.timing_ms.max_formula_id, None);
    }
}
