use std::fmt;

use scoop_identity::{
    ConeIdentity, DecodedPersistentId, PersistentIdResolver, PersistentKeyResolver,
    PersistentSourceContextId, PersistentTypeAliasId, SourceContextKey,
    SourceOriginResolutionError,
};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use super::{DecodedTypeAliasTargetV1, TypeAliasTargetResolutionError, TypeAliasTargetV1};
use crate::{
    CanonicalBinderListV1, DecodedCanonicalBinderListV1, DecodedExportDefinitionSourceV1,
    ExportDefinitionSourceV1, PublicLookupAccessV1, SignatureTypeReferenceResolver,
};

mod semantics;

pub use semantics::{
    TypeAliasDeclarationSourceV1, TypeAliasInterfaceSemanticAuthority,
    TypeAliasInterfaceSemanticValidationError,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeAliasInterfaceRecordV1 {
    alias: PersistentTypeAliasId,
    target: TypeAliasTargetV1,
    access: PublicLookupAccessV1,
    definition_origin: ExportDefinitionSourceV1,
    type_parameters: CanonicalBinderListV1,
    owner_parameter_count: u32,
}

impl TypeAliasInterfaceRecordV1 {
    pub fn try_new(
        alias: PersistentTypeAliasId,
        target: TypeAliasTargetV1,
        access: PublicLookupAccessV1,
        definition_origin: ExportDefinitionSourceV1,
    ) -> Result<Self, TypeAliasInterfaceRecordBuildError> {
        if access != PublicLookupAccessV1::DirectOnly {
            return Err(TypeAliasInterfaceRecordBuildError::DirectAccessRequired {
                alias,
                actual: access,
            });
        }
        if target == TypeAliasTargetV1::Alias(alias) {
            return Err(TypeAliasInterfaceRecordBuildError::DirectSelfReference(
                alias,
            ));
        }
        Ok(Self {
            alias,
            target,
            access,
            definition_origin,
            type_parameters: CanonicalBinderListV1::try_new(Vec::new())
                .expect("empty binders are valid"),
            owner_parameter_count: 0,
        })
    }

    pub fn with_type_parameters(
        mut self,
        parameters: CanonicalBinderListV1,
        owner_count: u32,
    ) -> Result<Self, TypeAliasInterfaceRecordBuildError> {
        if parameters.is_empty()
            || owner_count > parameters.len_u32()
            || matches!(self.target, TypeAliasTargetV1::Alias(_))
        {
            return Err(TypeAliasInterfaceRecordBuildError::InvalidParameters(
                self.alias,
            ));
        }
        self.type_parameters = parameters;
        self.owner_parameter_count = owner_count;
        Ok(self)
    }

    pub const fn type_parameters(&self) -> &CanonicalBinderListV1 {
        &self.type_parameters
    }

    pub const fn owner_parameter_count(&self) -> u32 {
        self.owner_parameter_count
    }

    pub fn bound_types(&self) -> impl Iterator<Item = &scoop_identity::SignatureTypeKey> {
        self.type_parameters
            .binders()
            .iter()
            .filter_map(|parameter| {
                if let crate::TypeParameterBoundsV1::Nominal(bounds) = parameter.bounds() {
                    Some(bounds)
                } else {
                    None
                }
            })
            .flat_map(|bounds| {
                bounds
                    .class()
                    .into_iter()
                    .chain(bounds.interfaces().values())
            })
    }

    pub const fn alias(&self) -> PersistentTypeAliasId {
        self.alias
    }

    pub const fn target(&self) -> &TypeAliasTargetV1 {
        &self.target
    }

    pub const fn access(&self) -> PublicLookupAccessV1 {
        self.access
    }

    pub const fn definition_origin(&self) -> &ExportDefinitionSourceV1 {
        &self.definition_origin
    }
}

impl WireEncode for TypeAliasInterfaceRecordV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(
            4 + u64::from(!self.type_parameters.is_empty())
                + u64::from(self.owner_parameter_count != 0),
        )?;
        encoder.field(1)?;
        self.alias.encode(encoder)?;
        encoder.field(2)?;
        self.target.encode(encoder)?;
        encoder.field(3)?;
        self.access.encode(encoder)?;
        encoder.field(4)?;
        self.definition_origin.encode(encoder)?;
        if !self.type_parameters.is_empty() {
            encoder.field(5)?;
            self.type_parameters.encode(encoder)?;
        }
        if self.owner_parameter_count != 0 {
            encoder.field(6)?;
            encoder.unsigned(u64::from(self.owner_parameter_count))?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedTypeAliasInterfaceRecordV1 {
    alias: DecodedPersistentId<PersistentTypeAliasId>,
    target: DecodedTypeAliasTargetV1,
    access: PublicLookupAccessV1,
    definition_origin: DecodedExportDefinitionSourceV1,
    type_parameters: Option<DecodedCanonicalBinderListV1>,
    owner_parameter_count: u32,
}

impl DecodedTypeAliasInterfaceRecordV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<TypeAliasInterfaceRecordV1, TypeAliasInterfaceRecordResolutionError<E>>
    where
        R: TypeAliasInterfaceRecordResolver<E>,
    {
        let alias = resolver
            .resolve(self.alias)
            .map_err(TypeAliasInterfaceRecordResolutionError::Alias)?;
        let target = self
            .target
            .resolve(resolver)
            .map_err(TypeAliasInterfaceRecordResolutionError::Target)?;
        let definition_origin = self
            .definition_origin
            .resolve(resolver)
            .map_err(TypeAliasInterfaceRecordResolutionError::DefinitionOrigin)?;
        let record =
            TypeAliasInterfaceRecordV1::try_new(alias, target, self.access, definition_origin)
                .map_err(TypeAliasInterfaceRecordResolutionError::Record)?;
        match self.type_parameters {
            Some(parameters) => record
                .with_type_parameters(
                    parameters
                        .resolve(resolver)
                        .map_err(TypeAliasInterfaceRecordResolutionError::Parameters)?,
                    self.owner_parameter_count,
                )
                .map_err(TypeAliasInterfaceRecordResolutionError::Record),
            None => Ok(record),
        }
    }
}

impl WireEncode for DecodedTypeAliasInterfaceRecordV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(
            4 + u64::from(self.type_parameters.is_some())
                + u64::from(self.owner_parameter_count != 0),
        )?;
        encoder.field(1)?;
        self.alias.encode(encoder)?;
        encoder.field(2)?;
        self.target.encode(encoder)?;
        encoder.field(3)?;
        self.access.encode(encoder)?;
        encoder.field(4)?;
        self.definition_origin.encode(encoder)?;
        if let Some(parameters) = &self.type_parameters {
            encoder.field(5)?;
            parameters.encode(encoder)?;
        }
        if self.owner_parameter_count != 0 {
            encoder.field(6)?;
            encoder.unsigned(u64::from(self.owner_parameter_count))?;
        }
        Ok(())
    }
}

impl WireDecode for DecodedTypeAliasInterfaceRecordV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        let fields = decoder.map()?;
        if !(4..=6).contains(&fields) {
            return Err(WireError::new(
                scoop_wire::WireErrorKind::InvalidLength {
                    expected: 4,
                    actual: fields,
                },
                decoder.path().clone(),
                Some(decoder.position()),
            ));
        }
        Ok(Self {
            alias: decoder.field(1, DecodedPersistentId::decode)?,
            target: decoder.field(2, DecodedTypeAliasTargetV1::decode)?,
            access: decoder.field(3, PublicLookupAccessV1::decode)?,
            definition_origin: decoder.field(4, DecodedExportDefinitionSourceV1::decode)?,
            type_parameters: if fields >= 5 {
                Some(decoder.field(5, DecodedCanonicalBinderListV1::decode)?)
            } else {
                None
            },
            owner_parameter_count: if fields == 6 {
                decoder.field(6, |decoder| {
                    let count = decoder.u32()?;
                    if count == 0 {
                        return Err(WireError::new(
                            scoop_wire::WireErrorKind::NonCanonicalCbor,
                            decoder.path().clone(),
                            Some(decoder.position()),
                        ));
                    }
                    Ok(count)
                })?
            } else {
                0
            },
        })
    }
}

pub trait TypeAliasInterfaceRecordResolver<E>:
    SignatureTypeReferenceResolver<E>
    + PersistentIdResolver<PersistentTypeAliasId, Error = E>
    + PersistentIdResolver<ConeIdentity, Error = E>
    + PersistentKeyResolver<PersistentSourceContextId, SourceContextKey, Error = E>
{
}

impl<R, E> TypeAliasInterfaceRecordResolver<E> for R where
    R: SignatureTypeReferenceResolver<E>
        + PersistentIdResolver<PersistentTypeAliasId, Error = E>
        + PersistentIdResolver<ConeIdentity, Error = E>
        + PersistentKeyResolver<PersistentSourceContextId, SourceContextKey, Error = E>
{
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypeAliasInterfaceRecordBuildError {
    DirectAccessRequired {
        alias: PersistentTypeAliasId,
        actual: PublicLookupAccessV1,
    },
    DirectSelfReference(PersistentTypeAliasId),
    InvalidParameters(PersistentTypeAliasId),
}

impl fmt::Display for TypeAliasInterfaceRecordBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DirectAccessRequired { alias, actual } => write!(
                formatter,
                "type alias {alias:?} requires direct-only access, found {actual:?}"
            ),
            Self::DirectSelfReference(alias) => {
                write!(formatter, "type alias {alias:?} directly references itself")
            }
            Self::InvalidParameters(alias) => write!(
                formatter,
                "type alias {alias} has invalid parameter bindings or an unexpanded generic target"
            ),
        }
    }
}

impl std::error::Error for TypeAliasInterfaceRecordBuildError {}

#[derive(Debug)]
pub enum TypeAliasInterfaceRecordResolutionError<E> {
    Alias(E),
    Target(TypeAliasTargetResolutionError<E>),
    DefinitionOrigin(SourceOriginResolutionError<E>),
    Record(TypeAliasInterfaceRecordBuildError),
    Parameters(crate::BinderListValidationError<E>),
}

impl<E: fmt::Display> fmt::Display for TypeAliasInterfaceRecordResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Alias(error) => write!(formatter, "invalid type-alias declaration: {error}"),
            Self::Target(error) => error.fmt(formatter),
            Self::DefinitionOrigin(error) => {
                write!(formatter, "invalid type-alias definition origin: {error}")
            }
            Self::Record(error) => error.fmt(formatter),
            Self::Parameters(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error
    for TypeAliasInterfaceRecordResolutionError<E>
{
}

#[cfg(test)]
mod tests;
