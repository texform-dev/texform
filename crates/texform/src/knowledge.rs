//! Knowledge-base package metadata.

/// Summary of one built-in knowledge package.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackageInfo {
    /// Package name as accepted by parser `packages` options (e.g. `base`, `ams`).
    pub name: String,
    /// Number of command records in the package.
    pub commands: usize,
    /// Number of environment records in the package.
    pub environments: usize,
}

/// List all built-in knowledge packages with record counts.
pub fn list_packages() -> Vec<PackageInfo> {
    texform_knowledge::builtin::ALL_PACKAGES
        .iter()
        .map(|pkg| PackageInfo {
            name: pkg.name.to_string(),
            commands: pkg.commands.len(),
            environments: pkg.environments.len(),
        })
        .collect()
}
