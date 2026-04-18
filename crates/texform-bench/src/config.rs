use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
pub struct DatasetsConfig {
    pub datasets: Vec<DatasetEntry>,
}

#[derive(Deserialize, Clone)]
pub struct DatasetEntry {
    pub slug: String,
    pub file: String,
}

impl DatasetsConfig {
    pub fn load(bench_root: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let yaml_path = bench_root.join("datasets.yaml");
        let content = std::fs::read_to_string(&yaml_path)
            .map_err(|error| format!("failed to read {}: {error}", yaml_path.display()))?;
        Ok(serde_yaml::from_str(&content)?)
    }

    pub fn filter_by_slugs(&self, slugs: &[String]) -> Vec<DatasetEntry> {
        if slugs.is_empty() {
            return self.datasets.clone();
        }

        self.datasets
            .iter()
            .filter(|dataset| slugs.contains(&dataset.slug))
            .cloned()
            .collect()
    }
}

pub fn resolve_bench_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("CARGO_MANIFEST_DIR should be inside the workspace root")
        .join("bench")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_by_slugs_empty_returns_all() {
        let yaml =
            "datasets:\n  - slug: a\n    file: a.parquet\n  - slug: b\n    file: b.parquet\n";
        let config: DatasetsConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.filter_by_slugs(&[]).len(), 2);
    }

    #[test]
    fn filter_by_slugs_selects() {
        let yaml =
            "datasets:\n  - slug: a\n    file: a.parquet\n  - slug: b\n    file: b.parquet\n";
        let config: DatasetsConfig = serde_yaml::from_str(yaml).unwrap();
        let filtered = config.filter_by_slugs(&["b".to_string()]);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].slug, "b");
    }
}
