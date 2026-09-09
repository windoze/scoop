use scoop_wire::{Encoder, WireEncodeV1};

use crate::{
    CanonicalIdentifier, PersistentConstructorId, PersistentFunctionId,
    PersistentGeneratedCallableId, PersistentGenericFunctionId, PersistentGenericTypeId,
    PersistentPropertyAccessorId, PersistentPropertyId, PersistentTypeId, SourceIdentity,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StructuralDefinitionSiteRoleV1 {
    LocalDeclaration,
    Lambda,
    DefaultValue,
    AnonymousObject,
    CoroutineTransform,
    CallbackConversion,
    CallableConversion,
    DispatchAdapter,
    Initializer,
    SynthesizedBridge,
    SyntheticValue,
    StringConstant,
}

impl StructuralDefinitionSiteRoleV1 {
    const fn tag(self) -> u64 {
        match self {
            Self::LocalDeclaration => 1,
            Self::Lambda => 2,
            Self::DefaultValue => 3,
            Self::AnonymousObject => 4,
            Self::CoroutineTransform => 5,
            Self::CallbackConversion => 6,
            Self::CallableConversion => 7,
            Self::DispatchAdapter => 8,
            Self::Initializer => 9,
            Self::SynthesizedBridge => 10,
            Self::SyntheticValue => 11,
            Self::StringConstant => 12,
        }
    }
}

impl WireEncodeV1 for StructuralDefinitionSiteRoleV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(self.tag())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StructuralPathSegmentV1 {
    site_role: StructuralDefinitionSiteRoleV1,
    ordinal: u32,
}

impl StructuralPathSegmentV1 {
    pub const fn new(site_role: StructuralDefinitionSiteRoleV1, ordinal: u32) -> Self {
        Self { site_role, ordinal }
    }

    pub const fn site_role(&self) -> StructuralDefinitionSiteRoleV1 {
        self.site_role
    }

    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }
}

impl WireEncodeV1 for StructuralPathSegmentV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.site_role.encode(encoder)?;
        encoder.field(2)?;
        encoder.unsigned(u64::from(self.ordinal))
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StructuralDefinitionPathV1(Vec<StructuralPathSegmentV1>);

impl StructuralDefinitionPathV1 {
    pub fn new(segments: Vec<StructuralPathSegmentV1>) -> Result<Self, crate::NonEmptyVecError> {
        if segments.is_empty() {
            Err(crate::NonEmptyVecError)
        } else {
            Ok(Self(segments))
        }
    }

    pub fn from_first(
        first: StructuralPathSegmentV1,
        rest: impl IntoIterator<Item = StructuralPathSegmentV1>,
    ) -> Self {
        let mut segments = vec![first];
        segments.extend(rest);
        Self(segments)
    }

    pub fn segments(&self) -> &[StructuralPathSegmentV1] {
        &self.0
    }
}

impl WireEncodeV1 for StructuralDefinitionPathV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.0.len() as u64)?;
        for segment in &self.0 {
            segment.encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DeclarationNameV1 {
    Named(CanonicalIdentifier),
    Constructor,
}

impl WireEncodeV1 for DeclarationNameV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Named(name) => {
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(1)?;
                encoder.field(1)?;
                name.encode(encoder)
            }
            Self::Constructor => {
                encoder.map(1)?;
                encoder.field(0)?;
                encoder.unsigned(2)
            }
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DefinitionOwnerAtomV1 {
    Type(PersistentTypeId),
    GenericType(PersistentGenericTypeId),
    Function(PersistentFunctionId),
    GenericFunction(PersistentGenericFunctionId),
    Constructor(PersistentConstructorId),
    Property(PersistentPropertyId),
    ExtensionProperty(crate::PersistentExtensionPropertyId),
    GeneratedCallable(PersistentGeneratedCallableId),
    PropertyAccessor(PersistentPropertyAccessorId),
}

impl WireEncodeV1 for DefinitionOwnerAtomV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        let (tag, id): (u64, &dyn WireEncodeV1) = match self {
            Self::Type(id) => (1, id),
            Self::GenericType(id) => (2, id),
            Self::Function(id) => (3, id),
            Self::GenericFunction(id) => (4, id),
            Self::Constructor(id) => (5, id),
            Self::Property(id) => (6, id),
            Self::ExtensionProperty(id) => (7, id),
            Self::GeneratedCallable(id) => (8, id),
            Self::PropertyAccessor(id) => (9, id),
        };
        encoder.map(2)?;
        encoder.field(0)?;
        encoder.unsigned(tag)?;
        encoder.field(1)?;
        id.encode(encoder)
    }
}

#[derive(Clone, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DefinitionOwnerChainV1(Vec<DefinitionOwnerAtomV1>);

impl DefinitionOwnerChainV1 {
    pub fn top_level() -> Self {
        Self(Vec::new())
    }

    pub fn from_outer_to_inner(owners: Vec<DefinitionOwnerAtomV1>) -> Self {
        Self(owners)
    }

    pub fn owners(&self) -> &[DefinitionOwnerAtomV1] {
        &self.0
    }
}

impl WireEncodeV1 for DefinitionOwnerChainV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.0.len() as u64)?;
        for owner in &self.0 {
            owner.encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DeclarationScopeV1 {
    ConeWide,
    SourceScoped(SourceIdentity),
    LexicalScoped {
        source: SourceIdentity,
        path: StructuralDefinitionPathV1,
    },
}

impl DeclarationScopeV1 {
    pub fn source(&self) -> Option<&SourceIdentity> {
        match self {
            Self::ConeWide => None,
            Self::SourceScoped(source) | Self::LexicalScoped { source, .. } => Some(source),
        }
    }
}

impl WireEncodeV1 for DeclarationScopeV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::ConeWide => {
                encoder.map(1)?;
                encoder.field(0)?;
                encoder.unsigned(1)
            }
            Self::SourceScoped(source) => {
                encoder.map(2)?;
                encoder.field(0)?;
                encoder.unsigned(2)?;
                encoder.field(1)?;
                source.encode(encoder)
            }
            Self::LexicalScoped { source, path } => {
                encoder.map(3)?;
                encoder.field(0)?;
                encoder.unsigned(3)?;
                encoder.field(1)?;
                source.encode(encoder)?;
                encoder.field(2)?;
                path.encode(encoder)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use scoop_wire::encode;

    use super::{
        DeclarationNameV1, StructuralDefinitionPathV1, StructuralDefinitionSiteRoleV1,
        StructuralPathSegmentV1,
    };
    use crate::CanonicalIdentifier;

    #[test]
    fn structural_path_is_non_empty_and_has_fixed_wire() {
        assert!(StructuralDefinitionPathV1::new(Vec::new()).is_err());
        let path = StructuralDefinitionPathV1::from_first(
            StructuralPathSegmentV1::new(StructuralDefinitionSiteRoleV1::Lambda, 0),
            [StructuralPathSegmentV1::new(
                StructuralDefinitionSiteRoleV1::LocalDeclaration,
                3,
            )],
        );
        assert_eq!(
            encode(&path).unwrap(),
            b"\x82\xa2\x01\x02\x02\x00\xa2\x01\x01\x02\x03"
        );
    }

    #[test]
    fn constructor_name_is_not_a_magic_identifier() {
        assert_eq!(
            encode(&DeclarationNameV1::Constructor).unwrap(),
            b"\xa1\x00\x02"
        );
        assert_eq!(
            encode(&DeclarationNameV1::Named(
                CanonicalIdentifier::new("init").unwrap()
            ))
            .unwrap(),
            b"\xa2\x00\x01\x01\x64init"
        );
    }
}
