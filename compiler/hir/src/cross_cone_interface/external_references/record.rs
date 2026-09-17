use std::fmt;

use scoop_identity::{ConeIdentity, PersistentExportBindingId, PersistentIdResolver};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError};

use super::{
    CanonicalDependencyBindingWitnessesV1, CanonicalExternalHirReferenceRolesV1,
    DecodedCanonicalDependencyBindingWitnessesV1, DecodedCanonicalExternalHirReferenceRolesV1,
    DecodedExternalHirTargetV1, DependencyBindingWitnessSetValidationError,
    ExternalHirReferenceRoleSetValidationError, ExternalHirReferenceRoleV1,
    ExternalHirTargetResolutionError, ExternalHirTargetResolver, ExternalHirTargetV1,
};

mod semantics;

pub use semantics::{
    ExternalHirReferenceSemanticAuthority, ExternalHirReferenceSemanticValidationError,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalHirReferenceV1 {
    origin: ConeIdentity,
    target: ExternalHirTargetV1,
    roles: CanonicalExternalHirReferenceRolesV1,
    witnesses: CanonicalDependencyBindingWitnessesV1,
}

impl ExternalHirReferenceV1 {
    pub fn try_new(
        origin: ConeIdentity,
        target: ExternalHirTargetV1,
        roles: CanonicalExternalHirReferenceRolesV1,
        witnesses: CanonicalDependencyBindingWitnessesV1,
    ) -> Result<Self, ExternalHirReferenceBuildError> {
        validate_witness_presence(&roles, &witnesses)?;
        Ok(Self {
            origin,
            target,
            roles,
            witnesses,
        })
    }

    pub const fn origin(&self) -> ConeIdentity {
        self.origin
    }

    pub const fn target(&self) -> ExternalHirTargetV1 {
        self.target
    }

    pub const fn roles(&self) -> &CanonicalExternalHirReferenceRolesV1 {
        &self.roles
    }

    pub const fn witnesses(&self) -> &CanonicalDependencyBindingWitnessesV1 {
        &self.witnesses
    }
}

impl WireEncode for ExternalHirReferenceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.origin.encode(encoder)?;
        encoder.field(2)?;
        self.target.encode(encoder)?;
        encoder.field(3)?;
        self.roles.encode(encoder)?;
        encoder.field(4)?;
        self.witnesses.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedExternalHirReferenceV1 {
    origin: scoop_identity::DecodedPersistentId<ConeIdentity>,
    target: DecodedExternalHirTargetV1,
    roles: DecodedCanonicalExternalHirReferenceRolesV1,
    witnesses: DecodedCanonicalDependencyBindingWitnessesV1,
}

impl DecodedExternalHirReferenceV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<ExternalHirReferenceV1, ExternalHirReferenceResolutionError<E>>
    where
        R: ExternalHirReferenceResolver<E>,
    {
        let origin = <R as PersistentIdResolver<ConeIdentity>>::resolve(resolver, self.origin)
            .map_err(ExternalHirReferenceResolutionError::Origin)?;
        let target = self
            .target
            .resolve(resolver)
            .map_err(ExternalHirReferenceResolutionError::Target)?;
        let roles = self
            .roles
            .validate()
            .map_err(ExternalHirReferenceResolutionError::Roles)?;
        let witnesses = self
            .witnesses
            .resolve(resolver)
            .map_err(ExternalHirReferenceResolutionError::Witnesses)?;
        ExternalHirReferenceV1::try_new(origin, target, roles, witnesses)
            .map_err(ExternalHirReferenceResolutionError::Shape)
    }
}

impl WireEncode for DecodedExternalHirReferenceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(1)?;
        self.origin.encode(encoder)?;
        encoder.field(2)?;
        self.target.encode(encoder)?;
        encoder.field(3)?;
        self.roles.encode(encoder)?;
        encoder.field(4)?;
        self.witnesses.encode(encoder)
    }
}

impl WireDecode for DecodedExternalHirReferenceV1 {
    fn decode(decoder: &mut Decoder<'_, '_>) -> Result<Self, WireError> {
        decoder.expect_map(4)?;
        Ok(Self {
            origin: decoder.field(1, scoop_identity::DecodedPersistentId::decode)?,
            target: decoder.field(2, DecodedExternalHirTargetV1::decode)?,
            roles: decoder.field(3, DecodedCanonicalExternalHirReferenceRolesV1::decode)?,
            witnesses: decoder.field(4, DecodedCanonicalDependencyBindingWitnessesV1::decode)?,
        })
    }
}

pub trait ExternalHirReferenceResolver<E>:
    ExternalHirTargetResolver<E>
    + PersistentIdResolver<ConeIdentity, Error = E>
    + PersistentIdResolver<PersistentExportBindingId, Error = E>
{
}

impl<R, E> ExternalHirReferenceResolver<E> for R where
    R: ExternalHirTargetResolver<E>
        + PersistentIdResolver<ConeIdentity, Error = E>
        + PersistentIdResolver<PersistentExportBindingId, Error = E>
{
}

fn validate_witness_presence(
    roles: &CanonicalExternalHirReferenceRolesV1,
    witnesses: &CanonicalDependencyBindingWitnessesV1,
) -> Result<(), ExternalHirReferenceBuildError> {
    let requires_witness = roles.roles().iter().any(|role| {
        matches!(
            role,
            ExternalHirReferenceRoleV1::ReexportTarget
                | ExternalHirReferenceRoleV1::AliasTarget
                | ExternalHirReferenceRoleV1::DefaultDependency
                | ExternalHirReferenceRoleV1::ConcreteSelectedUse
        )
    });
    match (requires_witness, witnesses.is_empty()) {
        (true, true) => Err(ExternalHirReferenceBuildError::MissingWitness),
        (false, false) => Err(ExternalHirReferenceBuildError::UnexpectedWitness),
        _ => Ok(()),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExternalHirReferenceBuildError {
    MissingWitness,
    UnexpectedWitness,
}

impl fmt::Display for ExternalHirReferenceBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::MissingWitness => {
                "external HIR reference role requires a dependency binding witness"
            }
            Self::UnexpectedWitness => {
                "external HIR reference has no source-name role for its binding witness"
            }
        })
    }
}

impl std::error::Error for ExternalHirReferenceBuildError {}

#[derive(Debug)]
pub enum ExternalHirReferenceResolutionError<E> {
    Origin(E),
    Target(ExternalHirTargetResolutionError<E>),
    Roles(ExternalHirReferenceRoleSetValidationError),
    Witnesses(DependencyBindingWitnessSetValidationError<E>),
    Shape(ExternalHirReferenceBuildError),
}

impl<E: fmt::Display> fmt::Display for ExternalHirReferenceResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Origin(error) => write!(formatter, "invalid external HIR origin: {error}"),
            Self::Target(error) => error.fmt(formatter),
            Self::Roles(error) => error.fmt(formatter),
            Self::Witnesses(error) => error.fmt(formatter),
            Self::Shape(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for ExternalHirReferenceResolutionError<E> {}

#[cfg(test)]
mod tests;
