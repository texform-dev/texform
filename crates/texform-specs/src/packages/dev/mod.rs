use crate::specs::{PackageSpecs, load_package_specs_from_str};

fn dev_yaml() -> &'static str {
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../resources/specs/dev.yaml"
    ))
}

pub fn load() -> PackageSpecs {
    load_package_specs_from_str(dev_yaml(), "dev")
}
