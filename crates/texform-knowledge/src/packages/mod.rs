pub use crate::builtin::ALL_PACKAGES;
pub use crate::builtin::BuiltinPackage as Package;
pub use crate::builtin::MANAGED_PACKAGE_IMPORT_ORDER;
pub use crate::builtin::PackageName;

pub fn all_package_names() -> Vec<&'static str> {
    crate::builtin::all_package_names()
}

pub fn get(name: &str) -> Option<&'static Package> {
    crate::builtin::lookup_package(name)
}
