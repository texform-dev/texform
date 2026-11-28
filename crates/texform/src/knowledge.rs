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
/// is given, in default import order (`base` first, with `ams` before
/// `mathtools`).
///
/// Package order sets precedence just like LaTeX `\usepackage`: later
/// incompatible definitions override earlier ones. `physics` is excluded
/// because it redefines standard commands; select it explicitly by extending
/// this list (appending `physics` lets it override `braket`, whereas placing
/// `braket` after `physics` matches MathJax).
pub fn default_packages() -> Vec<&'static str> {
    texform_core::parse::default_package_names().to_vec()
}
