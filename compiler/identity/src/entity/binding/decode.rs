use std::fmt;

use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WireErrorKind};

use super::{
    BindingNamespace, BindingRole, BindingTarget, BindingTargetError, ExportBindingKey,
    LocalBindingKey, LocalBindingRole,
};
use crate::{
    CanonicalIdentifierError, ConeIdentity, DecodedCanonicalIdentifier, DecodedPackagePath,
    DecodedPersistentId, DecodedSourceIdentity, PersistentEnumVariantId,
    PersistentExtensionPropertyId, PersistentFunctionId, PersistentGenericFunctionId,
    PersistentGenericTypeId, PersistentIdResolver, PersistentKeyResolver, PersistentObjectValueId,
    PersistentPropertyId, PersistentTypeAliasId, PersistentTypeId, SourceDeclarationKey,
    SourceIdentityResolutionError,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DecodedBindableEntity {
    Type(DecodedPersistentId<PersistentTypeId>),
    GenericType(DecodedPersistentId<PersistentGenericTypeId>),
    ObjectValue(DecodedPersistentId<PersistentObjectValueId>),
    Function(DecodedPersistentId<PersistentFunctionId>),
    GenericFunction(DecodedPersistentId<PersistentGenericFunctionId>),
    Property(DecodedPersistentId<PersistentPropertyId>),
    ExtensionProperty(DecodedPersistentId<PersistentExtensionPropertyId>),
    TypeAlias(DecodedPersistentId<PersistentTypeAliasId>),
    EnumVariant(DecodedPersistentId<PersistentEnumVariantId>),
}

impl DecodedBindableEntity {
    /// Resolves this untrusted target while rechecking the namespace/role
    /// discriminator against the referenced declaration's canonical key.
    pub fn resolve_target<R, E>(
        self,
        namespace: BindingNamespace,
        role: BindingRole,
        resolver: &mut R,
    ) -> Result<BindingTarget, BindingIdentityResolutionError<E>>
    where
        R: BindingResolver<E>,
    {
        resolve_target(namespace, self, role, resolver)
    }
}

impl WireEncode for DecodedBindableEntity {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        match self {
            Self::Type(id) => encode_value_sum(encoder, 1, id),
            Self::GenericType(id) => encode_value_sum(encoder, 2, id),
            Self::ObjectValue(id) => encode_value_sum(encoder, 3, id),
            Self::Function(id) => encode_value_sum(encoder, 4, id),
            Self::GenericFunction(id) => encode_value_sum(encoder, 5, id),
            Self::Property(id) => encode_value_sum(encoder, 6, id),
            Self::ExtensionProperty(id) => encode_value_sum(encoder, 7, id),
            Self::TypeAlias(id) => encode_value_sum(encoder, 8, id),
            Self::EnumVariant(id) => encode_value_sum(encoder, 9, id),
        }
    }
}

impl WireDecode for DecodedBindableEntity {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let (fields, tag) = decode_sum_header(decoder)?;
        expect_sum_length(decoder, fields, 2)?;
        match tag {
            1 => decode_id_variant(decoder, Self::Type),
            2 => decode_id_variant(decoder, Self::GenericType),
            3 => decode_id_variant(decoder, Self::ObjectValue),
            4 => decode_id_variant(decoder, Self::Function),
            5 => decode_id_variant(decoder, Self::GenericFunction),
            6 => decode_id_variant(decoder, Self::Property),
            7 => decode_id_variant(decoder, Self::ExtensionProperty),
            8 => decode_id_variant(decoder, Self::TypeAlias),
            9 => decode_id_variant(decoder, Self::EnumVariant),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

impl WireDecode for BindingNamespace {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        match decoder.unsigned()? {
            1 => Ok(Self::Type),
            2 => Ok(Self::Value),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

impl WireDecode for BindingRole {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        match decoder.unsigned()? {
            1 => Ok(Self::TypeName),
            2 => Ok(Self::ObjectValue),
            3 => Ok(Self::Function),
            4 => Ok(Self::ExtensionFunction),
            5 => Ok(Self::Property),
            6 => Ok(Self::ExtensionProperty),
            7 => Ok(Self::TypeAlias),
            8 => Ok(Self::EnumVariant),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

impl WireDecode for LocalBindingRole {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        match decoder.unsigned()? {
            1 => Ok(Self::Declaration),
            2 => Ok(Self::ExactImport),
            3 => Ok(Self::StarImport),
            4 => Ok(Self::AliasImport),
            tag => Err(unknown_tag(decoder, tag)),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedExportBindingKey {
    exporter: DecodedPersistentId<ConeIdentity>,
    package: DecodedPackagePath,
    namespace: BindingNamespace,
    name: DecodedCanonicalIdentifier,
    target: DecodedBindableEntity,
    role: BindingRole,
}

impl DecodedExportBindingKey {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<ExportBindingKey, BindingIdentityResolutionError<E>>
    where
        R: BindingResolver<E>,
    {
        let exporter = resolver
            .resolve(self.exporter)
            .map_err(BindingIdentityResolutionError::Reference)?;
        let package = self
            .package
            .validate()
            .map_err(BindingIdentityResolutionError::Package)?;
        let name = self
            .name
            .validate()
            .map_err(BindingIdentityResolutionError::Name)?;
        let target = resolve_target(self.namespace, self.target, self.role, resolver)?;
        Ok(ExportBindingKey::new(exporter, package, name, target))
    }
}

impl WireEncode for DecodedExportBindingKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(6)?;
        encoder.field(1)?;
        self.exporter.encode(encoder)?;
        encoder.field(2)?;
        self.package.encode(encoder)?;
        encoder.field(3)?;
        self.namespace.encode(encoder)?;
        encoder.field(4)?;
        self.name.encode(encoder)?;
        encoder.field(5)?;
        self.target.encode(encoder)?;
        encoder.field(6)?;
        self.role.encode(encoder)
    }
}

impl WireDecode for DecodedExportBindingKey {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(6)?;
        Ok(Self {
            exporter: decoder.field(1, DecodedPersistentId::decode)?,
            package: decoder.field(2, DecodedPackagePath::decode)?,
            namespace: decoder.field(3, BindingNamespace::decode)?,
            name: decoder.field(4, DecodedCanonicalIdentifier::decode)?,
            target: decoder.field(5, DecodedBindableEntity::decode)?,
            role: decoder.field(6, BindingRole::decode)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedLocalBindingKey {
    origin: DecodedPersistentId<ConeIdentity>,
    source: DecodedSourceIdentity,
    package: DecodedPackagePath,
    namespace: BindingNamespace,
    local_name: DecodedCanonicalIdentifier,
    target: DecodedBindableEntity,
    binding_role: BindingRole,
    source_role: LocalBindingRole,
}

impl DecodedLocalBindingKey {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<LocalBindingKey, BindingIdentityResolutionError<E>>
    where
        R: BindingResolver<E>,
    {
        let origin = resolver
            .resolve(self.origin)
            .map_err(BindingIdentityResolutionError::Reference)?;
        let source = self
            .source
            .resolve(resolver)
            .map_err(BindingIdentityResolutionError::Source)?;
        if origin != source.cone() {
            return Err(BindingIdentityResolutionError::OriginMismatch);
        }
        let package = self
            .package
            .validate()
            .map_err(BindingIdentityResolutionError::Package)?;
        let local_name = self
            .local_name
            .validate()
            .map_err(BindingIdentityResolutionError::Name)?;
        let target = resolve_target(self.namespace, self.target, self.binding_role, resolver)?;
        Ok(LocalBindingKey::new(
            source,
            package,
            local_name,
            target,
            self.source_role,
        ))
    }
}

impl WireEncode for DecodedLocalBindingKey {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(8)?;
        encoder.field(1)?;
        self.origin.encode(encoder)?;
        encoder.field(2)?;
        self.source.encode(encoder)?;
        encoder.field(3)?;
        self.package.encode(encoder)?;
        encoder.field(4)?;
        self.namespace.encode(encoder)?;
        encoder.field(5)?;
        self.local_name.encode(encoder)?;
        encoder.field(6)?;
        self.target.encode(encoder)?;
        encoder.field(7)?;
        self.binding_role.encode(encoder)?;
        encoder.field(8)?;
        self.source_role.encode(encoder)
    }
}

impl WireDecode for DecodedLocalBindingKey {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(8)?;
        Ok(Self {
            origin: decoder.field(1, DecodedPersistentId::decode)?,
            source: decoder.field(2, DecodedSourceIdentity::decode)?,
            package: decoder.field(3, DecodedPackagePath::decode)?,
            namespace: decoder.field(4, BindingNamespace::decode)?,
            local_name: decoder.field(5, DecodedCanonicalIdentifier::decode)?,
            target: decoder.field(6, DecodedBindableEntity::decode)?,
            binding_role: decoder.field(7, BindingRole::decode)?,
            source_role: decoder.field(8, LocalBindingRole::decode)?,
        })
    }
}

pub trait BindingResolver<E>:
    PersistentIdResolver<ConeIdentity, Error = E>
    + PersistentKeyResolver<PersistentTypeId, SourceDeclarationKey, Error = E>
    + PersistentKeyResolver<PersistentGenericTypeId, SourceDeclarationKey, Error = E>
    + PersistentKeyResolver<PersistentObjectValueId, SourceDeclarationKey, Error = E>
    + PersistentKeyResolver<PersistentFunctionId, SourceDeclarationKey, Error = E>
    + PersistentKeyResolver<PersistentGenericFunctionId, SourceDeclarationKey, Error = E>
    + PersistentKeyResolver<PersistentPropertyId, SourceDeclarationKey, Error = E>
    + PersistentKeyResolver<PersistentExtensionPropertyId, SourceDeclarationKey, Error = E>
    + PersistentKeyResolver<PersistentTypeAliasId, SourceDeclarationKey, Error = E>
    + PersistentIdResolver<PersistentEnumVariantId, Error = E>
{
}

impl<R, E> BindingResolver<E> for R where
    R: PersistentIdResolver<ConeIdentity, Error = E>
        + PersistentKeyResolver<PersistentTypeId, SourceDeclarationKey, Error = E>
        + PersistentKeyResolver<PersistentGenericTypeId, SourceDeclarationKey, Error = E>
        + PersistentKeyResolver<PersistentObjectValueId, SourceDeclarationKey, Error = E>
        + PersistentKeyResolver<PersistentFunctionId, SourceDeclarationKey, Error = E>
        + PersistentKeyResolver<PersistentGenericFunctionId, SourceDeclarationKey, Error = E>
        + PersistentKeyResolver<PersistentPropertyId, SourceDeclarationKey, Error = E>
        + PersistentKeyResolver<PersistentExtensionPropertyId, SourceDeclarationKey, Error = E>
        + PersistentKeyResolver<PersistentTypeAliasId, SourceDeclarationKey, Error = E>
        + PersistentIdResolver<PersistentEnumVariantId, Error = E>
{
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BindingIdentityResolutionError<E> {
    Reference(E),
    Source(SourceIdentityResolutionError<E>),
    Package(CanonicalIdentifierError),
    Name(CanonicalIdentifierError),
    InvalidTarget {
        namespace: BindingNamespace,
        role: BindingRole,
    },
    Target(BindingTargetError),
    OriginMismatch,
}

impl<E: fmt::Display> fmt::Display for BindingIdentityResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reference(error) => error.fmt(formatter),
            Self::Source(error) => error.fmt(formatter),
            Self::Package(error) => write!(formatter, "invalid binding package: {error}"),
            Self::Name(error) => write!(formatter, "invalid binding name: {error}"),
            Self::InvalidTarget { namespace, role } => write!(
                formatter,
                "binding target is invalid for namespace {namespace:?} and role {role:?}"
            ),
            Self::Target(error) => error.fmt(formatter),
            Self::OriginMismatch => {
                formatter.write_str("local binding origin must equal its source Cone")
            }
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for BindingIdentityResolutionError<E> {}

fn resolve_target<R, E>(
    namespace: BindingNamespace,
    target: DecodedBindableEntity,
    role: BindingRole,
    resolver: &mut R,
) -> Result<BindingTarget, BindingIdentityResolutionError<E>>
where
    R: BindingResolver<E>,
{
    match (namespace, role, target) {
        (BindingNamespace::Type, BindingRole::TypeName, DecodedBindableEntity::Type(id)) => {
            resolve_source_key(resolver, id, BindingTarget::type_name)
        }
        (BindingNamespace::Type, BindingRole::TypeName, DecodedBindableEntity::GenericType(id)) => {
            resolve_source_key(resolver, id, BindingTarget::type_name)
        }
        (
            BindingNamespace::Value,
            BindingRole::ObjectValue,
            DecodedBindableEntity::ObjectValue(id),
        ) => resolve_source_key(resolver, id, BindingTarget::object_value),
        (BindingNamespace::Value, BindingRole::Function, DecodedBindableEntity::Function(id)) => {
            resolve_source_key(resolver, id, BindingTarget::function)
        }
        (
            BindingNamespace::Value,
            BindingRole::Function,
            DecodedBindableEntity::GenericFunction(id),
        ) => resolve_source_key(resolver, id, BindingTarget::function),
        (
            BindingNamespace::Value,
            BindingRole::ExtensionFunction,
            DecodedBindableEntity::Function(id),
        ) => resolve_source_key(resolver, id, BindingTarget::extension_function),
        (
            BindingNamespace::Value,
            BindingRole::ExtensionFunction,
            DecodedBindableEntity::GenericFunction(id),
        ) => resolve_source_key(resolver, id, BindingTarget::extension_function),
        (BindingNamespace::Value, BindingRole::Property, DecodedBindableEntity::Property(id)) => {
            resolve_source_key(resolver, id, BindingTarget::property)
        }
        (
            BindingNamespace::Value,
            BindingRole::ExtensionProperty,
            DecodedBindableEntity::ExtensionProperty(id),
        ) => resolve_source_key(resolver, id, BindingTarget::extension_property),
        (BindingNamespace::Type, BindingRole::TypeAlias, DecodedBindableEntity::TypeAlias(id)) => {
            resolve_source_key(resolver, id, BindingTarget::type_alias)
        }
        (
            BindingNamespace::Value,
            BindingRole::EnumVariant,
            DecodedBindableEntity::EnumVariant(id),
        ) => resolver
            .resolve(id)
            .map(BindingTarget::enum_variant)
            .map_err(BindingIdentityResolutionError::Reference),
        (namespace, role, _) => {
            Err(BindingIdentityResolutionError::InvalidTarget { namespace, role })
        }
    }
}

fn resolve_source_key<R, I, E>(
    resolver: &mut R,
    id: DecodedPersistentId<I>,
    build: fn(&SourceDeclarationKey) -> Result<BindingTarget, BindingTargetError>,
) -> Result<BindingTarget, BindingIdentityResolutionError<E>>
where
    I: crate::PersistentId,
    R: PersistentKeyResolver<I, SourceDeclarationKey, Error = E>,
{
    let key = resolver
        .resolve_key(id)
        .map_err(BindingIdentityResolutionError::Reference)?;
    build(&key).map_err(BindingIdentityResolutionError::Target)
}

fn decode_sum_header(decoder: &mut Decoder<'_>) -> Result<(u64, u64), WireError> {
    let fields = decoder.map()?;
    let tag = decoder.field(0, Decoder::unsigned)?;
    Ok((fields, tag))
}

fn decode_id_variant<I, T>(
    decoder: &mut Decoder<'_>,
    build: impl FnOnce(DecodedPersistentId<I>) -> T,
) -> Result<T, WireError>
where
    I: crate::PersistentId,
{
    decoder.field(1, DecodedPersistentId::decode).map(build)
}

fn expect_sum_length(decoder: &Decoder<'_>, actual: u64, expected: u64) -> Result<(), WireError> {
    if actual == expected {
        Ok(())
    } else {
        Err(WireError::new(
            WireErrorKind::InvalidLength { expected, actual },
            decoder.path().clone(),
            Some(decoder.position()),
        ))
    }
}

fn unknown_tag(decoder: &Decoder<'_>, tag: u64) -> WireError {
    WireError::new(
        WireErrorKind::UnknownTag { tag },
        decoder.path().clone(),
        Some(decoder.position()),
    )
}

fn encode_tag(encoder: &mut Encoder, tag: u64) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.field(0)?;
    encoder.unsigned(tag)
}

fn encode_value_sum(
    encoder: &mut Encoder,
    tag: u64,
    value: &impl WireEncode,
) -> Result<(), scoop_wire::cbor::EncodeError> {
    encoder.map(2)?;
    encode_tag(encoder, tag)?;
    encoder.field(1)?;
    value.encode(encoder)
}

#[cfg(test)]
mod tests;
