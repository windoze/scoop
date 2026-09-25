use std::fmt;

use scoop_identity::{ConeIdentity, PersistentExportBindingId, PersistentIdResolver};
use scoop_wire::{Decoder, Encoder, WireDecode, WireEncode, WireError, WirePath};

use super::{
    CanonicalDependencyBindingWitnessesV1, CanonicalExternalHirReferenceRolesV1,
    CanonicalHirDependencyCallSitesV1, CanonicalHirDependencyTypeSitesV1,
    DecodedCanonicalDependencyBindingWitnessesV1, DecodedCanonicalExternalHirReferenceRolesV1,
    DecodedCanonicalHirDependencyCallSitesV1, DecodedCanonicalHirDependencyTypeSitesV1,
    DecodedExternalHirTargetV1, DependencyBindingWitnessSetValidationError,
    ExternalHirReferenceRoleSetValidationError, ExternalHirTargetResolutionError,
    ExternalHirTargetResolver, ExternalHirTargetV1, HirDependencyCallSiteResolutionError,
    HirDependencyTypeSiteResolutionError, HirDependencyTypeSiteResolver,
};

mod call_sites;
mod semantics;
use call_sites::validate_call_sites;

pub use semantics::{
    ExternalHirReferenceSemanticAuthority, ExternalHirReferenceSemanticValidationError,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalHirReferenceV1 {
    origin: ConeIdentity,
    target: ExternalHirTargetV1,
    roles: CanonicalExternalHirReferenceRolesV1,
    witnesses: CanonicalDependencyBindingWitnessesV1,
    call_sites: CanonicalHirDependencyCallSitesV1,
    type_sites: CanonicalHirDependencyTypeSitesV1,
}

impl ExternalHirReferenceV1 {
    pub fn try_new(
        origin: ConeIdentity,
        target: ExternalHirTargetV1,
        roles: CanonicalExternalHirReferenceRolesV1,
        witnesses: CanonicalDependencyBindingWitnessesV1,
        call_sites: CanonicalHirDependencyCallSitesV1,
        type_sites: CanonicalHirDependencyTypeSitesV1,
    ) -> Result<Self, ExternalHirReferenceBuildError> {
        validate_witness_presence(target, &roles, &witnesses)?;
        validate_call_sites(target, &roles, &witnesses, &call_sites)?;
        let typed = roles.contains(super::ExternalHirReferenceRoleV1::ExecutableTypeDependency);
        if typed && type_sites.is_empty() {
            return Err(ExternalHirReferenceBuildError::MissingTypeSites);
        }
        if !type_sites.is_empty() && (!typed || !matches!(target, ExternalHirTargetV1::Nominal(_)))
        {
            return Err(ExternalHirReferenceBuildError::UnexpectedTypeSites);
        }
        Ok(Self {
            origin,
            target,
            roles,
            witnesses,
            call_sites,
            type_sites,
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

    pub const fn call_sites(&self) -> &CanonicalHirDependencyCallSitesV1 {
        &self.call_sites
    }

    pub const fn type_sites(&self) -> &CanonicalHirDependencyTypeSitesV1 {
        &self.type_sites
    }
}

impl WireEncode for ExternalHirReferenceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(6)?;
        encoder.field(1)?;
        self.origin.encode(encoder)?;
        encoder.field(2)?;
        self.target.encode(encoder)?;
        encoder.field(3)?;
        self.roles.encode(encoder)?;
        encoder.field(4)?;
        self.witnesses.encode(encoder)?;
        encoder.field(5)?;
        self.call_sites.encode(encoder)?;
        encoder.field(6)?;
        self.type_sites.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedExternalHirReferenceV1 {
    origin: scoop_identity::DecodedPersistentId<ConeIdentity>,
    target: DecodedExternalHirTargetV1,
    roles: DecodedCanonicalExternalHirReferenceRolesV1,
    witnesses: DecodedCanonicalDependencyBindingWitnessesV1,
    call_sites: DecodedCanonicalHirDependencyCallSitesV1,
    type_sites: DecodedCanonicalHirDependencyTypeSitesV1,
}

impl DecodedExternalHirReferenceV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<ExternalHirReferenceV1, ExternalHirReferenceResolutionError<E>>
    where
        R: ExternalHirReferenceResolver<E>,
    {
        self.resolve_at(resolver, &WirePath::root())
    }

    pub fn resolve_at<R: ExternalHirReferenceResolver<E>, E>(
        self,
        resolver: &mut R,

        path: &WirePath,
    ) -> Result<ExternalHirReferenceV1, ExternalHirReferenceResolutionError<E>> {
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
        let call_sites = self
            .call_sites
            .resolve(resolver, &path.clone().field(5))
            .map_err(|source| ExternalHirReferenceResolutionError::CallSites(Box::new(source)))?;
        let type_sites = self
            .type_sites
            .resolve(resolver, &path.clone().field(6))
            .map_err(|source| ExternalHirReferenceResolutionError::TypeSites(Box::new(source)))?;
        ExternalHirReferenceV1::try_new(origin, target, roles, witnesses, call_sites, type_sites)
            .map_err(ExternalHirReferenceResolutionError::Shape)
    }
}

impl WireEncode for DecodedExternalHirReferenceV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(6)?;
        encoder.field(1)?;
        self.origin.encode(encoder)?;
        encoder.field(2)?;
        self.target.encode(encoder)?;
        encoder.field(3)?;
        self.roles.encode(encoder)?;
        encoder.field(4)?;
        self.witnesses.encode(encoder)?;
        encoder.field(5)?;
        self.call_sites.encode(encoder)?;
        encoder.field(6)?;
        self.type_sites.encode(encoder)
    }
}

impl WireDecode for DecodedExternalHirReferenceV1 {
    fn decode(decoder: &mut Decoder<'_>) -> Result<Self, WireError> {
        decoder.expect_map(6)?;
        Ok(Self {
            origin: decoder.field(1, scoop_identity::DecodedPersistentId::decode)?,
            target: decoder.field(2, DecodedExternalHirTargetV1::decode)?,
            roles: decoder.field(3, DecodedCanonicalExternalHirReferenceRolesV1::decode)?,
            witnesses: decoder.field(4, DecodedCanonicalDependencyBindingWitnessesV1::decode)?,
            call_sites: decoder.field(5, DecodedCanonicalHirDependencyCallSitesV1::decode)?,
            type_sites: decoder.field(6, DecodedCanonicalHirDependencyTypeSitesV1::decode)?,
        })
    }
}

pub trait ExternalHirReferenceResolver<E>:
    ExternalHirTargetResolver<E>
    + PersistentIdResolver<ConeIdentity, Error = E>
    + PersistentIdResolver<PersistentExportBindingId, Error = E>
    + HirDependencyTypeSiteResolver<E>
{
}

impl<R, E> ExternalHirReferenceResolver<E> for R where
    R: ExternalHirTargetResolver<E>
        + PersistentIdResolver<ConeIdentity, Error = E>
        + PersistentIdResolver<PersistentExportBindingId, Error = E>
        + HirDependencyTypeSiteResolver<E>
{
}

fn validate_witness_presence(
    target: ExternalHirTargetV1,
    roles: &CanonicalExternalHirReferenceRolesV1,
    witnesses: &CanonicalDependencyBindingWitnessesV1,
) -> Result<(), ExternalHirReferenceBuildError> {
    let requires_witness = roles
        .roles()
        .iter()
        .any(|role| role.requires_source_name_witness(target));
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
    MissingCallSites,
    UnexpectedCallSites,
    MissingTypeSites,
    UnexpectedTypeSites,
    RuntimeTarget,
    CallReason { site: usize },
    CallWitnessIndex { site: usize, index: u32 },
}

impl fmt::Display for ExternalHirReferenceBuildError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingWitness => formatter
                .write_str("external HIR reference role requires a dependency binding witness"),
            Self::UnexpectedWitness => formatter.write_str(
                "external HIR reference has no source-name role for its binding witness",
            ),
            Self::MissingCallSites => {
                formatter.write_str("selected external callable has no actual HIR call sites")
            }
            Self::UnexpectedCallSites => formatter
                .write_str("external reference has call sites without a concrete callable use"),
            Self::MissingTypeSites => {
                formatter.write_str("executable type dependency has no actual HIR type sites")
            }
            Self::UnexpectedTypeSites => {
                formatter.write_str("type sites require an executable nominal type dependency")
            }
            Self::RuntimeTarget => {
                formatter.write_str("runtime cast failures require a constructor target")
            }
            Self::CallReason { site } => {
                write!(formatter, "call site {site} has the wrong source reason")
            }
            Self::CallWitnessIndex { site, index } => write!(
                formatter,
                "call site {site} names missing binding witness {index}"
            ),
        }
    }
}

impl std::error::Error for ExternalHirReferenceBuildError {}

#[derive(Debug)]
pub enum ExternalHirReferenceResolutionError<E> {
    Origin(E),
    Target(ExternalHirTargetResolutionError<E>),
    Roles(ExternalHirReferenceRoleSetValidationError),
    Witnesses(DependencyBindingWitnessSetValidationError<E>),
    CallSites(Box<HirDependencyCallSiteResolutionError<E>>),
    TypeSites(Box<HirDependencyTypeSiteResolutionError<E>>),
    Shape(ExternalHirReferenceBuildError),
}

impl<E: fmt::Display> fmt::Display for ExternalHirReferenceResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Origin(error) => write!(formatter, "invalid external HIR origin: {error}"),
            Self::Target(error) => error.fmt(formatter),
            Self::Roles(error) => error.fmt(formatter),
            Self::Witnesses(error) => error.fmt(formatter),
            Self::CallSites(error) => error.fmt(formatter),
            Self::TypeSites(error) => error.fmt(formatter),
            Self::Shape(error) => error.fmt(formatter),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for ExternalHirReferenceResolutionError<E> {}

#[cfg(test)]
mod tests;
