pub use texform_transform::{Profile, TransformConfig};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NormalizeConfig {
    pub parse: texform_core::parse::ParseConfig,
    pub transform: TransformConfig,
}
