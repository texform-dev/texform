use crate::specs::PackageSpecs;

pub mod dev;

pub struct Package {
    pub name: &'static str,
    pub load: fn() -> PackageSpecs,
}

pub const ALL_PACKAGES: &[Package] = &[Package {
    name: "dev",
    load: dev::load,
}];

pub const DEFAULT_EXCLUDED_PACKAGES: &[&str] = &["dev"];

pub fn runtime_default_packages() -> &'static [&'static str] {
    &[]
}

pub fn get(name: &str) -> Option<&'static Package> {
    ALL_PACKAGES.iter().find(|p| p.name == name)
}
