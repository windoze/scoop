use std::fmt;

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::{
    SourceDeclarationKey, SourceDeclarationKeyError, SourceDeclarationKind, SourceDeclarationSite,
    SourceNominalKind,
};
use crate::{
    CanonicalIdentifierError, ConeIdentity, DeclarationName, DecodedDeclarationName,
    DecodedDeclarationScope, DecodedDefinitionOwnerChain, DecodedDuplicateSignatureKey,
    DecodedPackagePath, DecodedPersistentId, DefinitionOwnerResolutionError, DuplicateSignatureKey,
    OptionalSignatureType, PersistentConstructorId, PersistentExtensionPropertyId,
    PersistentFunctionId, PersistentGeneratedCallableId, PersistentGenericFunctionId,
    PersistentGenericTypeId, PersistentIdResolver, PersistentPropertyAccessorId,
    PersistentPropertyId, PersistentTypeId, SourceIdentityResolutionError,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedSourceDeclarationKey {
    origin: DecodedPersistentId<ConeIdentity>,
    package: DecodedPackagePath,
    owners: DecodedDefinitionOwnerChain,
    name: DecodedDeclarationName,
    declaration_kind: SourceDeclarationKind,
    duplicate_signature: DecodedDuplicateSignatureKey,
    scope: DecodedDeclarationScope,
}

impl DecodedSourceDeclarationKey {
    pub(crate) fn is_generic_nominal(&self) -> bool {
        matches!(self.duplicate_signature, DecodedDuplicateSignatureKey::Nominal { type_parameter_count } if type_parameter_count != 0)
    }

    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<SourceDeclarationKey, SourceDeclarationResolutionError<E>>
    where
        R: PersistentIdResolver<ConeIdentity, Error = E>
            + PersistentIdResolver<PersistentTypeId, Error = E>
            + PersistentIdResolver<PersistentGenericTypeId, Error = E>
            + PersistentIdResolver<PersistentFunctionId, Error = E>
            + PersistentIdResolver<PersistentGenericFunctionId, Error = E>
            + PersistentIdResolver<PersistentConstructorId, Error = E>
            + PersistentIdResolver<PersistentPropertyId, Error = E>
            + PersistentIdResolver<PersistentExtensionPropertyId, Error = E>
            + PersistentIdResolver<PersistentGeneratedCallableId, Error = E>
            + PersistentIdResolver<PersistentPropertyAccessorId, Error = E>
            + PersistentIdResolver<crate::PersistentEnumVariantId, Error = E>,
    {
        let origin = resolver
            .resolve(self.origin)
            .map_err(SourceDeclarationResolutionError::Reference)?;
        let package = self
            .package
            .validate()
            .map_err(SourceDeclarationResolutionError::Package)?;
        let owners = self
            .owners
            .resolve(resolver)
            .map_err(SourceDeclarationResolutionError::Owners)?;
        let name = self
            .name
            .validate()
            .map_err(SourceDeclarationResolutionError::Name)?;
        let duplicate_signature = self
            .duplicate_signature
            .resolve(resolver)
            .map_err(SourceDeclarationResolutionError::Reference)?;
        let scope = self
            .scope
            .resolve(resolver)
            .map_err(SourceDeclarationResolutionError::Scope)?;
        let site = SourceDeclarationSite::new(origin, package, owners, scope)
            .map_err(SourceDeclarationResolutionError::Site)?;
        rebuild_source_key(self.declaration_kind, name, duplicate_signature, site)
    }
}

impl WireEncode for DecodedSourceDeclarationKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(7)?;
        encoder.field(1)?;
        self.origin.encode(encoder)?;
        encoder.field(2)?;
        self.package.encode(encoder)?;
        encoder.field(3)?;
        self.owners.encode(encoder)?;
        encoder.field(4)?;
        self.name.encode(encoder)?;
        encoder.field(5)?;
        self.declaration_kind.encode(encoder)?;
        encoder.field(6)?;
        self.duplicate_signature.encode(encoder)?;
        encoder.field(7)?;
        self.scope.encode(encoder)
    }
}

impl WireDecode for DecodedSourceDeclarationKey {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(7)?;
        Ok(Self {
            origin: decoder.field(1, DecodedPersistentId::decode)?,
            package: decoder.field(2, DecodedPackagePath::decode)?,
            owners: decoder.field(3, DecodedDefinitionOwnerChain::decode)?,
            name: decoder.field(4, DecodedDeclarationName::decode)?,
            declaration_kind: decoder.field(5, decode_declaration_kind)?,
            duplicate_signature: decoder.field(6, DecodedDuplicateSignatureKey::decode)?,
            scope: decoder.field(7, DecodedDeclarationScope::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SourceDeclarationResolutionError<E> {
    Reference(E),
    Package(CanonicalIdentifierError),
    Owners(DefinitionOwnerResolutionError<E>),
    Name(CanonicalIdentifierError),
    Scope(SourceIdentityResolutionError<E>),
    Site(SourceDeclarationKeyError),
    Shape(SourceDeclarationKind),
}

impl<E: fmt::Display> fmt::Display for SourceDeclarationResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reference(error) => error.fmt(formatter),
            Self::Package(error) => write!(formatter, "invalid declaration package: {error}"),
            Self::Owners(error) => error.fmt(formatter),
            Self::Name(error) => write!(formatter, "invalid declaration name: {error}"),
            Self::Scope(error) => error.fmt(formatter),
            Self::Site(error) => error.fmt(formatter),
            Self::Shape(kind) => write!(
                formatter,
                "source declaration kind {kind:?} does not match its name and duplicate signature"
            ),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for SourceDeclarationResolutionError<E> {}

fn rebuild_source_key<E>(
    kind: SourceDeclarationKind,
    name: DeclarationName,
    signature: DuplicateSignatureKey,
    site: SourceDeclarationSite,
) -> Result<SourceDeclarationKey, SourceDeclarationResolutionError<E>> {
    match (kind, name, signature) {
        (
            SourceDeclarationKind::Class,
            DeclarationName::Named(name),
            DuplicateSignatureKey::Nominal {
                type_parameter_count,
            },
        ) => Ok(SourceDeclarationKey::nominal(
            site,
            name,
            SourceNominalKind::Class,
            type_parameter_count,
        )),
        (
            SourceDeclarationKind::Interface,
            DeclarationName::Named(name),
            DuplicateSignatureKey::Nominal {
                type_parameter_count,
            },
        ) => Ok(SourceDeclarationKey::nominal(
            site,
            name,
            SourceNominalKind::Interface,
            type_parameter_count,
        )),
        (
            SourceDeclarationKind::Struct,
            DeclarationName::Named(name),
            DuplicateSignatureKey::Nominal {
                type_parameter_count,
            },
        ) => Ok(SourceDeclarationKey::nominal(
            site,
            name,
            SourceNominalKind::Struct,
            type_parameter_count,
        )),
        (
            SourceDeclarationKind::Enum,
            DeclarationName::Named(name),
            DuplicateSignatureKey::Nominal {
                type_parameter_count,
            },
        ) => Ok(SourceDeclarationKey::nominal(
            site,
            name,
            SourceNominalKind::Enum,
            type_parameter_count,
        )),
        (
            SourceDeclarationKind::Object,
            DeclarationName::Named(name),
            DuplicateSignatureKey::Nominal {
                type_parameter_count,
            },
        ) => Ok(SourceDeclarationKey::nominal(
            site,
            name,
            SourceNominalKind::Object,
            type_parameter_count,
        )),
        (
            SourceDeclarationKind::AnnotationClass,
            DeclarationName::Named(name),
            DuplicateSignatureKey::Nominal {
                type_parameter_count,
            },
        ) => Ok(SourceDeclarationKey::nominal(
            site,
            name,
            SourceNominalKind::AnnotationClass,
            type_parameter_count,
        )),
        (
            SourceDeclarationKind::Function,
            DeclarationName::Named(name),
            DuplicateSignatureKey::Function {
                type_parameter_count,
                receiver,
                parameters,
                conditions,
            },
        ) => Ok(SourceDeclarationKey::constrained_function(
            site,
            name,
            type_parameter_count,
            optional_signature(receiver),
            parameters,
            conditions,
        )),
        (
            SourceDeclarationKind::Constructor,
            DeclarationName::Constructor,
            DuplicateSignatureKey::Constructor { parameters },
        ) => Ok(SourceDeclarationKey::constructor(site, parameters)),
        (
            SourceDeclarationKind::Property,
            DeclarationName::Named(name),
            DuplicateSignatureKey::Property {
                type_parameter_count: 0,
                receiver: OptionalSignatureType::Absent,
            },
        ) => Ok(SourceDeclarationKey::property(site, name)),
        (
            SourceDeclarationKind::ExtensionProperty,
            DeclarationName::Named(name),
            DuplicateSignatureKey::Property {
                type_parameter_count,
                receiver: OptionalSignatureType::Present(receiver),
            },
        ) => Ok(SourceDeclarationKey::extension_property(
            site,
            name,
            type_parameter_count,
            *receiver,
        )),
        (
            SourceDeclarationKind::TypeAlias,
            DeclarationName::Named(name),
            DuplicateSignatureKey::TypeAlias,
        ) => Ok(SourceDeclarationKey::type_alias(site, name)),
        (kind, _, _) => Err(SourceDeclarationResolutionError::Shape(kind)),
    }
}

fn optional_signature(value: OptionalSignatureType) -> Option<crate::SignatureTypeKey> {
    match value {
        OptionalSignatureType::Absent => None,
        OptionalSignatureType::Present(value) => Some(*value),
    }
}

fn decode_declaration_kind(decoder: &mut Decoder<'_>) -> Result<SourceDeclarationKind, WireError> {
    match decoder.unsigned()? {
        1 => Ok(SourceDeclarationKind::Class),
        2 => Ok(SourceDeclarationKind::Interface),
        3 => Ok(SourceDeclarationKind::Struct),
        4 => Ok(SourceDeclarationKind::Enum),
        5 => Ok(SourceDeclarationKind::Object),
        6 => Ok(SourceDeclarationKind::AnnotationClass),
        7 => Ok(SourceDeclarationKind::Function),
        8 => Ok(SourceDeclarationKind::Constructor),
        9 => Ok(SourceDeclarationKind::Property),
        10 => Ok(SourceDeclarationKind::ExtensionProperty),
        11 => Ok(SourceDeclarationKind::TypeAlias),
        tag => Err(WireError::new(
            WireErrorKind::UnknownTag { tag },
            decoder.path().clone(),
            Some(decoder.position()),
        )),
    }
}

#[cfg(test)]
mod tests;
