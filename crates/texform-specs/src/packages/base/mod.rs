use crate::specs::{PackageSpecs, load_package_specs_from_str};

fn base_yaml() -> &'static str {
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../resources/specs/base.yaml"
    ))
}

pub fn load() -> PackageSpecs {
    load_package_specs_from_str(base_yaml(), "base")
}
