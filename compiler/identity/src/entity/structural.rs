use scoop_wire::{Encoder, WireEncode};

use crate::{
    CanonicalIdentifier, PersistentConstructorId, PersistentFunctionId,
    PersistentGeneratedCallableId, PersistentGenericFunctionId, PersistentGenericTypeId,
    PersistentPropertyAccessorId, PersistentPropertyId, PersistentTypeId, SourceIdentity,
};

mod decode;

pub use decode::{
    DecodedDeclarationName, DecodedDeclarationScope, DecodedDefinitionOwnerAtom,
    DecodedDefinitionOwnerChain, DefinitionOwnerResolutionError,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum StructuralDefinitionSiteRole {
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

impl StructuralDefinitionSiteRole {
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

impl WireEncode for StructuralDefinitionSiteRole {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(self.tag())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StructuralPathSegment {
    site_role: StructuralDefinitionSiteRole,
    ordinal: u32,
}

impl StructuralPathSegment {
    pub const fn new(site_role: StructuralDefinitionSiteRole, ordinal: u32) -> Self {
        Self { site_role, ordinal }
    }

    pub const fn site_role(&self) -> StructuralDefinitionSiteRole {
        self.site_role
    }

    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }
}

impl WireEncode for StructuralPathSegment {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(2)?;
        encoder.field(1)?;
        self.site_role.encode(encoder)?;
        encoder.field(2)?;
        encoder.unsigned(u64::from(self.ordinal))
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StructuralDefinitionPath(Vec<StructuralPathSegment>);

impl StructuralDefinitionPath {
    pub fn new(segments: Vec<StructuralPathSegment>) -> Result<Self, crate::NonEmptyVecError> {
        if segments.is_empty() {
            Err(crate::NonEmptyVecError)
        } else {
            Ok(Self(segments))
        }
    }

    pub fn from_first(
        first: StructuralPathSegment,
        rest: impl IntoIterator<Item = StructuralPathSegment>,
    ) -> Self {
        let mut segments = vec![first];
        segments.extend(rest);
        Self(segments)
    }

    pub fn segments(&self) -> &[StructuralPathSegment] {
        &self.0
    }
}

impl WireEncode for StructuralDefinitionPath {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.0.len() as u64)?;
        for segment in &self.0 {
            segment.encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DeclarationName {
    Named(CanonicalIdentifier),
    Constructor,
}

impl WireEncode for DeclarationName {
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
pub enum DefinitionOwnerAtom {
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

impl WireEncode for DefinitionOwnerAtom {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        let (tag, id): (u64, &dyn WireEncode) = match self {
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
pub struct DefinitionOwnerChain(Vec<DefinitionOwnerAtom>);

impl DefinitionOwnerChain {
    pub fn top_level() -> Self {
        Self(Vec::new())
    }

    pub fn from_outer_to_inner(owners: Vec<DefinitionOwnerAtom>) -> Self {
        Self(owners)
    }

    pub fn owners(&self) -> &[DefinitionOwnerAtom] {
        &self.0
    }
}

impl WireEncode for DefinitionOwnerChain {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.0.len() as u64)?;
        for owner in &self.0 {
            owner.encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DeclarationScope {
    ConeWide,
    SourceScoped(SourceIdentity),
    LexicalScoped {
        source: SourceIdentity,
        path: StructuralDefinitionPath,
    },
}

impl DeclarationScope {
    pub fn source(&self) -> Option<&SourceIdentity> {
        match self {
            Self::ConeWide => None,
            Self::SourceScoped(source) | Self::LexicalScoped { source, .. } => Some(source),
        }
    }
}

impl WireEncode for DeclarationScope {
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
        DeclarationName, StructuralDefinitionPath, StructuralDefinitionSiteRole,
        StructuralPathSegment,
    };
    use crate::CanonicalIdentifier;

    #[test]
    fn structural_path_is_non_empty_and_has_fixed_wire() {
        assert!(StructuralDefinitionPath::new(Vec::new()).is_err());
        let path = StructuralDefinitionPath::from_first(
            StructuralPathSegment::new(StructuralDefinitionSiteRole::Lambda, 0),
            [StructuralPathSegment::new(
                StructuralDefinitionSiteRole::LocalDeclaration,
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
            encode(&DeclarationName::Constructor).unwrap(),
            b"\xa1\x00\x02"
        );
        assert_eq!(
            encode(&DeclarationName::Named(
                CanonicalIdentifier::new("init").unwrap()
            ))
            .unwrap(),
            b"\xa2\x00\x01\x01\x64init"
        );
    }
}
