//! Per-call parser state.
//!
//! [`ParserState`] is the runtime companion to [`ParseContext`] and
//! [`ParseConfig`]: it bundles the immutable knowledge base reference, the
//! user-facing config, and the source string with the *mutable* per-call
//! counters that the parser needs (currently just brace-group depth).
//!
//! The split exists because `ParseContext` is meant to be reused across many
//! parse calls with different configs; it cannot own a mutable depth counter
//! without either re-entrance hazards or interior-mutability footguns.
//!
//! `ParserState` is constructed once at the entry point of every parse call
//! and threaded through the internal `custom` parser closures.

use std::cell::Cell;

use super::{ParseConfig, ParseContext};

pub(crate) struct ParserState<'a> {
    pub(crate) ctx: &'a ParseContext,
    pub(crate) config: &'a ParseConfig,
    pub(crate) src: &'a str,
    group_depth: Cell<usize>,
}

impl<'a> ParserState<'a> {
    pub(crate) fn new(ctx: &'a ParseContext, config: &'a ParseConfig, src: &'a str) -> Self {
        Self {
            ctx,
            config,
            src,
            group_depth: Cell::new(0),
        }
    }

    /// Attempt to enter a brace-group scope.
    ///
    /// Returns `None` if the current depth has already reached
    /// `config.max_group_depth`; callers must then bail out to the depth-limit
    /// fallback path. On success the returned [`GroupGuard`] increments the
    /// depth and restores the previous value on drop, so callers do not need
    /// to remember to decrement on every exit path (success, error, panic).
    pub(crate) fn enter_group(&self) -> Option<GroupGuard<'_>> {
        let prev = self.group_depth.get();
        if prev >= self.config.max_group_depth {
            return None;
        }
        self.group_depth.set(prev + 1);
        Some(GroupGuard { state: self, prev })
    }
}

/// RAII handle returned by [`ParserState::enter_group`]. Restores the
/// previous brace-group depth on drop, including on error returns and panics.
pub(crate) struct GroupGuard<'a> {
    state: &'a ParserState<'a>,
    prev: usize,
}

impl Drop for GroupGuard<'_> {
    fn drop(&mut self) {
        self.state.group_depth.set(self.prev);
    }
}
