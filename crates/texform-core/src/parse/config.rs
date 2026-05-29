//! User-facing parse configuration.

/// Parse-time configuration knobs.
///
/// `ParseConfig` is plain data describing how a single parse call should
/// behave. It is independent of [`ParseContext`](super::ParseContext), which
/// owns the knowledge base; the same context can be reused across many calls
/// with different configs.
///
/// Two orthogonal axes control parsing, both with **`true` = stricter**:
///
/// - [`reject_unknown`](Self::reject_unknown): unknown command/environment names
///   become diagnostics (`true`) or `known: false` nodes (`false`).
/// - [`abort_on_error`](Self::abort_on_error): stop at the first error (`true`)
///   or continue parsing to collect every diagnostic (`false`, slower).
///   Recovery may return a read-only document containing `Error` nodes; use
///   [`ParseResult::try_into_document`](super::ParseResult::try_into_document)
///   when downstream code requires a complete tree.
///
/// Named extremes [`STRICT`](Self::STRICT) and [`LENIENT`](Self::LENIENT) cover
/// the two corners where both axes agree. For mixed settings, use struct-update
/// syntax, e.g. `ParseConfig { reject_unknown: true, ..Default::default() }`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseConfig {
    /// When `true`, unknown command/environment names become diagnostics
    /// instead of being preserved as `known: false` nodes.
    ///
    /// This controls **unknown-name handling only**, not general error strictness.
    pub reject_unknown: bool,
    /// When `true`, parsing stops at the first error in each content item and
    /// may return no document for that item.
    ///
    /// When `false`, the parser uses recovery fallbacks to collect every
    /// diagnostic (useful for IDEs and playgrounds, but slower on large corpora)
    /// and may attach `Error` nodes to the returned document. A document with
    /// error nodes is read-only and cannot be used by transform entry points.
    pub abort_on_error: bool,
    /// Hard upper bound on nested `{ ... }` brace group depth.
    pub max_group_depth: usize,
}

impl ParseConfig {
    /// Default hard upper bound on nested `{ ... }` brace group depth.
    pub const DEFAULT_MAX_GROUP_DEPTH: usize = 128;

    /// Both axes strict: `reject_unknown = true`, `abort_on_error = true`.
    ///
    /// Used by normalization and other correctness-sensitive paths.
    pub const STRICT: Self = Self {
        reject_unknown: true,
        abort_on_error: true,
        max_group_depth: Self::DEFAULT_MAX_GROUP_DEPTH,
    };

    /// Both axes lenient: `reject_unknown = false`, `abort_on_error = false`.
    ///
    /// Default for standalone parse-only entry points and interactive tools.
    pub const LENIENT: Self = Self {
        reject_unknown: false,
        abort_on_error: false,
        max_group_depth: Self::DEFAULT_MAX_GROUP_DEPTH,
    };
}

impl Default for ParseConfig {
    fn default() -> Self {
        Self::LENIENT
    }
}
