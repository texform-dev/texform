//! FlattenGroups phase shell.
//!
//! The phase is reserved in the pipeline, but the redundant-group removal
//! algorithm is intentionally left for the follow-up FlattenGroups work.

use crate::ast::Ast;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FlattenGroupsConfig {
    pub enabled: bool,
}

impl FlattenGroupsConfig {
    pub const ENABLED: Self = Self { enabled: true };
    pub const DISABLED: Self = Self { enabled: false };
    pub const DEFAULTS: Self = Self::ENABLED;
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FlattenGroupsReport {
    pub removed_empty: usize,
    pub replaced_single_child: usize,
    pub spliced: usize,
    pub redirected_slot: usize,
}

pub fn run(_ast: &mut Ast, _config: &FlattenGroupsConfig, _report: &mut FlattenGroupsReport) {}
