// The single place that lists built-in packages. `build.rs` includes this file,
// so it must not contain inner attributes or depend on other crate items.

/// Every built-in package, in import order. Loaded packages are merged in this
/// order regardless of request order, so when two packages define the same name
/// with different specs, the later package wins.
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
/// commands. Listed in import order.
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
