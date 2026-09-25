//! Resolve unit definitions before constructing their dependency edges.

use super::*;
use crate::{
    OdrFreeLirFoundation, StrongDigestFinalizationPlanV1, StrongRegistrationIdentitySurfaceV1,
};
use scoop_identity::{
    DefinitionAtomRole, DefinitionAtomSubkey, DigestNodeKey, LinkageClass, ObjectDefinitionAtomKey,
    ObjectDefinitionPlanKey, PersistentSymbolKey, StrongDefinitionEntity, StrongDefinitionRole,
};

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
    ) -> Result<Self, Error> {
        let identity = identities
            .initialization_units()
            .iter()
            .find(|identity| identity.semantic_id() == unit)
            .ok_or(Error::MissingRegistrationIdentity(unit))?;
        let descriptor = artifact(unit, ArtifactRole::Descriptor, foundation)?;
        let cell = artifact(unit, ArtifactRole::Cell, foundation)?;
        let registration = artifact(unit, ArtifactRole::Registration, foundation)?;
        if identity.definition_plan() != registration.plan() {
            return Err(Error::RegistrationDefinition {
                expected: registration.plan(),
                actual: identity.definition_plan(),
            });
        }
        let expected = DigestNodeKey::strong_registration(registration.plan());

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
) -> Result<StrongInitializationArtifactRefV2, Error> {
    let key = ObjectDefinitionPlanKey::strong(
        foundation.producer(),
        StrongDefinitionEntity::initialization_unit(unit),
        role.definition(),
    )
    .map_err(Error::DefinitionKey)?;

    let definition = foundation
        .definition_plans()
        .iter()
        .find(|record| record.key() == &key)
        .ok_or(Error::MissingDefinition(key))?;
    let symbol = PersistentSymbolRequest::new(role.symbol(unit), LinkageClass::ConeStrong)
        .map_err(Error::Symbol)?;

    if !foundation.contains_symbol_request(symbol) {
        return Err(Error::MissingSymbol(symbol));
    }

    let mut primary = foundation.definition_atoms().iter().filter(|record| {
        record.key().plan() == definition.id() && record.key().role() == DefinitionAtomRole::Primary
    });
    let primary = match (primary.next(), primary.next()) {
        (Some(record), None) => record.id(),
        _ => return Err(Error::PrimaryAtoms(definition.id())),
    };

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
