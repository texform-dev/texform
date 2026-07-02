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
    pub fn load_from_yaml(yaml_path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(yaml_path)
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

pub fn default_repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("CARGO_MANIFEST_DIR should be inside the workspace root")
        .to_path_buf()
}

pub fn default_datasets_yaml() -> PathBuf {
    default_repo_root().join("regression").join("datasets.yaml")
}

pub fn default_results_root(datasets_yaml: &Path) -> PathBuf {
    datasets_yaml
        .parent()
        .expect("datasets yaml should have a parent directory")
        .join("results")
}

pub fn resolve_dataset_file(datasets_yaml: &Path, entry: &DatasetEntry) -> PathBuf {
    datasets_yaml
        .parent()
        .expect("datasets yaml should have a parent directory")
        .join(&entry.file)
}
