// The single place that lists built-in packages. `build.rs` includes this file,
// so it must not contain inner attributes or depend on other crate items.

/// Every built-in package in canonical order.
///
/// This order is used for the default selection, stable `from_packages` lists
/// on merged records, and deterministic rule metadata. Actual package loading
/// follows the caller's request order (with `base` always first), so a later
/// package overrides an earlier incompatible definition.
pub const IMPORT_ORDER: [&str; 13] = [
    "base",
    "ams",
    "braket",
    "physics",
    "textmacros",
    "bboldx",
    "boldsymbol",
    "color",
    "upgreek",
    "mathtools",
    "cancel",
    "extpfeil",
    "textcomp",
];

/// Packages loaded when no package list is given: every built-in package except
/// `physics`, which must be loaded explicitly because it redefines standard
/// commands. Listed in default import order (`ams` before `mathtools`).
pub const DEFAULTS: [&str; 12] = [
    "base",
    "ams",
    "braket",
    "textmacros",
    "bboldx",
    "boldsymbol",
    "color",
    "upgreek",
    "mathtools",
    "cancel",
    "extpfeil",
    "textcomp",
];
