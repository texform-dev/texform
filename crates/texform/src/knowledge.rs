//! Shared immutable knowledge bases and built-in package metadata.

pub use texform_core::knowledge::{KnowledgeBase, KnowledgeBaseBuildError, KnowledgeBaseBuilder};

/// Summary of one built-in knowledge package.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PackageInfo {
    /// Package name as accepted by knowledge-base `packages` options (e.g. `base`, `ams`).
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

/// Names of the built-in packages a knowledge base loads when no package list
/// is given, in canonical import order.
///
/// `physics` is excluded because it redefines standard commands; select it
/// explicitly, for example by extending this list.
pub fn default_packages() -> Vec<&'static str> {
    texform_core::parse::default_package_names().to_vec()
}
