//! Physical constant contributions owned by a callable's machine body.

use scoop_identity::{
    DefinitionAtomSubkey, StructuralDefinitionPath, StructuralDefinitionSiteRole,
    StructuralPathSegment,
};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CallableConstantKind {
    Relocatable,
    ReadOnly,
}

impl CallableConstantKind {
    pub const ALL: [Self; 2] = [Self::Relocatable, Self::ReadOnly];

    pub fn subkey(self) -> DefinitionAtomSubkey {
        match self {
            Self::Relocatable => DefinitionAtomSubkey::Singleton,
            Self::ReadOnly => {
                DefinitionAtomSubkey::StructuralPath(StructuralDefinitionPath::from_first(
                    StructuralPathSegment::new(StructuralDefinitionSiteRole::SyntheticValue, 0),
                    [],
                ))
            }
        }
    }
}
