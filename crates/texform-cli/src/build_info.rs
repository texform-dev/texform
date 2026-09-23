//! Build identity embedded by `build.rs`.
//!
//! Distinguishes binaries built from different source trees: the crate
//! version alone cannot tell two worktrees or an uncommitted edit apart.

/// Crate version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Text printed by `--version` after the binary name, for example
/// `0.5.0 (0123456789ab 2026-09-22, dirty)`. The parenthesized part is
/// omitted when the build had no git information.
pub const VERSION_TEXT: &str = env!("TEXFORM_BUILD_VERSION_TEXT");

/// Full hash of the commit the binary was built from.
pub fn commit() -> Option<&'static str> {
    non_empty(env!("TEXFORM_BUILD_COMMIT"))
}

/// Committer date (`YYYY-MM-DD`) of [`commit`].
pub fn commit_date() -> Option<&'static str> {
    non_empty(env!("TEXFORM_BUILD_COMMIT_DATE"))
}

/// Whether build inputs differed from [`commit`] at build time.
pub fn dirty() -> bool {
    env!("TEXFORM_BUILD_DIRTY") == "true"
}

fn non_empty(value: &'static str) -> Option<&'static str> {
    (!value.is_empty()).then_some(value)
}
