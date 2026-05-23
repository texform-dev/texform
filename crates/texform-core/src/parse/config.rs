//! User-facing parse configuration.

/// Parse-time configuration knobs.
///
/// `ParseConfig` is plain data describing how a single parse call should
/// behave. It is independent of [`ParseContext`](super::ParseContext), which
/// owns the knowledge base; the same context can be reused across many calls
/// with different configs.
///
/// Four named presets cover every legal `strict` × `recover` combination at
/// the default `max_group_depth`. Prefer these at call sites to avoid
/// re-spelling the field set, and fall back to a struct literal only when
/// `max_group_depth` needs to deviate from the default (e.g.
/// `ParseConfig { max_group_depth: 256, ..ParseConfig::STRICT_NO_RECOVER }`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseConfig {
    /// When `true`, unknown command/environment names become diagnostics
    /// instead of being preserved as `known: false` nodes.
    pub strict: bool,
    /// When `true`, the parser wraps content items with a recovery fallback
    /// so one malformed item does not abort the rest of the parse.
    pub recover: bool,
    /// Hard upper bound on nested `{ ... }` brace group depth.
    pub max_group_depth: usize,
}

impl ParseConfig {
    /// Default hard upper bound on nested `{ ... }` brace group depth.
    pub const DEFAULT_MAX_GROUP_DEPTH: usize = 128;

    /// `strict = true`, `recover = true` — strict mode that still collects
    /// every diagnostic via recovery. Rarely useful; intended for dev tools
    /// that want both unknown-as-error and a full error list.
    pub const STRICT_RECOVER: Self = Self {
        strict: true,
        recover: true,
        max_group_depth: Self::DEFAULT_MAX_GROUP_DEPTH,
    };

    /// `strict = true`, `recover = false` — strict mode that aborts at the
    /// first error in each item. Used by transform / dev / tests where
    /// unknown commands must fail and the extra diagnostics from recovery
    /// are unused.
    pub const STRICT_NO_RECOVER: Self = Self {
        strict: true,
        recover: false,
        max_group_depth: Self::DEFAULT_MAX_GROUP_DEPTH,
    };

    /// `strict = false`, `recover = true` — preserve unknown commands as
    /// `known: false` nodes and collect every diagnostic via recovery.
    /// Default for interactive tools (playground, IDE error highlighting).
    pub const NONSTRICT_RECOVER: Self = Self {
        strict: false,
        recover: true,
        max_group_depth: Self::DEFAULT_MAX_GROUP_DEPTH,
    };

    /// `strict = false`, `recover = false` — preserve unknown commands but
    /// abort at the first error. Used by bench / batch pipelines that only
    /// need a pass/fail signal and pay no cost for recovery.
    pub const NONSTRICT_NO_RECOVER: Self = Self {
        strict: false,
        recover: false,
        max_group_depth: Self::DEFAULT_MAX_GROUP_DEPTH,
    };
}

impl Default for ParseConfig {
    fn default() -> Self {
        Self::NONSTRICT_RECOVER
    }
}
