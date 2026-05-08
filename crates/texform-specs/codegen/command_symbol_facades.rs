// Stable public facade names for exact TeX control-symbol command names.
// These names intentionally start with `_` to distinguish commands like `\;`
// from possible control-word commands like `\semicolon`.
pub const COMMAND_SYMBOL_FACADES: &[(&str, &str)] = &[
    (" ", "_CONTROL_SPACE"),
    (",", "_COMMA"),
    (";", "_SEMICOLON"),
    (":", "_COLON"),
    ("!", "_EXCLAMATION"),
    ("*", "_STAR"),
    ("\\", "_BACKSLASH"),
    (">", "_GREATER_THAN"),
    ("|", "_VERTICAL_BAR"),
    (".", "_PERIOD"),
    ("'", "_APOSTROPHE"),
    ("‘", "_LEFT_SINGLE_QUOTE"),
    ("’", "_RIGHT_SINGLE_QUOTE"),
    ("\"", "_DOUBLE_QUOTE"),
    ("`", "_GRAVE_ACCENT"),
    ("^", "_CARET"),
    ("=", "_EQUALS"),
    ("~", "_TILDE"),
];
