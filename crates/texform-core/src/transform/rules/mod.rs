//! Builtin transform rule registry.
//!
//! Keep module declarations and the concrete rule list in one file so new
//! rules only need one registration site.

use crate::transform::rule::TransformRule;

pub mod over_to_frac;
pub mod quantity_to_qty;
pub mod trace_to_tr;

pub(crate) static ALL_RULES: &[&dyn TransformRule] = &[
    &over_to_frac::OVER_TO_FRAC,
    &quantity_to_qty::QUANTITY_TO_QTY,
    &trace_to_tr::TRACE_TO_TR,
];
