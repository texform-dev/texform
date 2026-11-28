#![allow(dead_code)]

use texform_transform::FlattenGroupsConfig;

/// FlattenGroups enabled with rendered-spacing protection on.
pub const FLATTEN_STRICT: FlattenGroupsConfig = FlattenGroupsConfig {
    enabled: true,
    preserve_rendered_spacing: true,
};

/// FlattenGroups enabled with only structural guards on.
pub const FLATTEN_STRUCTURAL_ONLY: FlattenGroupsConfig = FlattenGroupsConfig {
    enabled: true,
    preserve_rendered_spacing: false,
};
