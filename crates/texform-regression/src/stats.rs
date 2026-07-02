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
