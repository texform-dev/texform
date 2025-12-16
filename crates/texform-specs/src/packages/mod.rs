use crate::specs::PackageSpecs;

pub mod base;

pub struct Package {
    pub name: &'static str,
    pub load: fn() -> PackageSpecs,
}

pub const ALL_PACKAGES: &[Package] = &[Package {
    name: "base",
    load: base::load,
}];

pub fn get(name: &str) -> Option<&'static Package> {
    ALL_PACKAGES.iter().find(|p| p.name == name)
}
