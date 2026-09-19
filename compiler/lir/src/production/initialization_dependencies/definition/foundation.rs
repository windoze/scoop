//! Resolve unit definitions before constructing their dependency edges.

use super::*;
use crate::{
    OdrFreeLirFoundation, StrongDigestFinalizationPlanV1, StrongRegistrationIdentitySurfaceV1,
};
use scoop_identity::{
    DefinitionAtomRole, DefinitionAtomSubkey, DigestNodeKey, LinkageClass, ObjectDefinitionAtomKey,
    ObjectDefinitionPlanKey, PersistentSymbolKey, StrongDefinitionEntity, StrongDefinitionRole,
};
use scoop_wire::{BudgetMeter, WirePath};

mod error;
pub use error::InitializationDefinitionResolutionErrorV2;
type Error = InitializationDefinitionResolutionErrorV2;

impl StrongInitializationUnitDefinitionRefV2 {
    /// Binds all physical identities without manufacturing a temporary unit
    /// registration with missing dependencies. Full initialization semantics,
    /// schedule, digest inputs, and selected-use authority are checked when the
    /// complete registration/section is committed.
    pub fn from_foundation(
        unit: PersistentInitializationUnitId,
        foundation: &OdrFreeLirFoundation,
        identities: &StrongRegistrationIdentitySurfaceV1,
        digests: &StrongDigestFinalizationPlanV1,
        meter: &mut BudgetMeter,
    ) -> Result<Self, Error> {
        let path = WirePath::root();
        meter.charge_work(identities.initialization_units().len() as u64, &path)?;
        let identity = identities
            .initialization_units()
            .iter()
            .find(|identity| identity.semantic_id() == unit)
            .ok_or(Error::MissingRegistrationIdentity(unit))?;
        let descriptor = artifact(unit, ArtifactRole::Descriptor, foundation, meter)?;
        let cell = artifact(unit, ArtifactRole::Cell, foundation, meter)?;
        let registration = artifact(unit, ArtifactRole::Registration, foundation, meter)?;
        if identity.definition_plan() != registration.plan() {
            return Err(Error::RegistrationDefinition {
                expected: registration.plan(),
                actual: identity.definition_plan(),
            });
        }
        let expected = DigestNodeKey::strong_registration(registration.plan());
        meter.charge_work(digests.nodes().len() as u64, &path)?;
        let fingerprint = digests
            .nodes()
            .iter()
            .find(|node| node.key() == &expected)
            .ok_or(Error::MissingFingerprint(expected))?;
        if identity.fingerprint_node() != fingerprint.id() {
            return Err(Error::RegistrationFingerprint {
                expected: fingerprint.id(),
                actual: identity.fingerprint_node(),
            });
        }
        Ok(Self {
            provider: foundation.producer(),
            unit,
            descriptor,
            cell,
            registration,
            registration_fingerprint: fingerprint.id(),
        })
    }
}

#[derive(Clone, Copy)]
enum ArtifactRole {
    Descriptor,
    Cell,
    Registration,
}

impl ArtifactRole {
    fn definition(self) -> StrongDefinitionRole {
        match self {
            Self::Descriptor => StrongDefinitionRole::InitializationDescriptor,
            Self::Cell => StrongDefinitionRole::InitializationCell,
            Self::Registration => StrongDefinitionRole::InitializationRegistration,
        }
    }
    fn symbol(self, unit: PersistentInitializationUnitId) -> PersistentSymbolKey {
        match self {
            Self::Descriptor => PersistentSymbolKey::InitializationDescriptor(unit),
            Self::Cell => PersistentSymbolKey::InitializationCell(unit),
            Self::Registration => PersistentSymbolKey::InitializationRegistration(unit),
        }
    }
}

fn artifact(
    unit: PersistentInitializationUnitId,
    role: ArtifactRole,
    foundation: &OdrFreeLirFoundation,
    meter: &mut BudgetMeter,
) -> Result<StrongInitializationArtifactRefV2, Error> {
    let path = WirePath::root();
    meter.charge_work(1, &path)?;
    let key = ObjectDefinitionPlanKey::strong(
        foundation.producer(),
        StrongDefinitionEntity::initialization_unit(unit),
        role.definition(),
    )
    .map_err(Error::DefinitionKey)?;
    meter.charge_work(foundation.definition_plans().len() as u64, &path)?;
    let definition = foundation
        .definition_plans()
        .iter()
        .find(|record| record.key() == &key)
        .ok_or(Error::MissingDefinition(key))?;
    let symbol = PersistentSymbolRequest::new(role.symbol(unit), LinkageClass::ConeStrong)
        .map_err(Error::Symbol)?;
    meter.charge_work(foundation.symbol_requests().len() as u64, &path)?;
    if !foundation.contains_symbol_request(symbol) {
        return Err(Error::MissingSymbol(symbol));
    }
    meter.charge_work(foundation.definition_atoms().len() as u64, &path)?;
    let mut primary = foundation.definition_atoms().iter().filter(|record| {
        record.key().plan() == definition.id() && record.key().role() == DefinitionAtomRole::Primary
    });
    let primary = match (primary.next(), primary.next()) {
        (Some(record), None) => record.id(),
        _ => return Err(Error::PrimaryAtoms(definition.id())),
    };
    meter.charge_work(foundation.definition_atoms().len() as u64, &path)?;
    let mut associated = foundation.definition_atoms().iter().filter(|record| {
        record.key().plan() == definition.id() && record.key().role() != DefinitionAtomRole::Primary
    });
    match role {
        ArtifactRole::Descriptor => {
            let expected = ObjectDefinitionAtomKey::new(
                definition.id(),
                DefinitionAtomRole::AddressTakenConstant,
                DefinitionAtomSubkey::InitializationUnit(unit),
            );
            if !matches!((associated.next(), associated.next()), (Some(record), None) if record.key() == &expected)
            {
                return Err(Error::AssociatedAtoms(definition.id()));
            }
        }
        ArtifactRole::Cell | ArtifactRole::Registration => {
            if associated.next().is_some() {
                return Err(Error::AssociatedAtoms(definition.id()));
            }
        }
    }
    Ok(StrongInitializationArtifactRefV2 {
        symbol,
        plan: definition.id(),
        primary,
    })
}
