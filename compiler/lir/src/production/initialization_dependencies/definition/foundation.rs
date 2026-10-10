//! Resolve unit definitions before constructing their dependency edges.

use super::*;
use crate::{ConeLirFoundation, RegistrationIdentitySurfaceV1};
use scoop_identity::{
    DefinitionAtomRole, DefinitionAtomSubkey, LinkageClass, ObjectDefinitionAtomKey,
    ObjectDefinitionPlanOwner, PersistentSymbolKey, StrongDefinitionEntity, StrongDefinitionRole,
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
        foundation: &ConeLirFoundation,
        identities: &RegistrationIdentitySurfaceV1,
    ) -> Result<Self, Error> {
        let identity = identities
            .initialization_units()
            .iter()
            .find(|identity| identity.semantic_id() == unit)
            .ok_or(Error::MissingRegistrationIdentity(unit))?;
        let cell = artifact(unit, ArtifactRole::Cell, foundation)?;
        let registration = artifact(unit, ArtifactRole::Registration, foundation)?;
        if identity.definition_plan() != registration.plan() {
            return Err(Error::RegistrationDefinition {
                expected: registration.plan(),
                actual: identity.definition_plan(),
            });
        }
        Ok(Self {
            provider: foundation.producer(),
            unit,
            cell,
            registration,
        })
    }
}

#[derive(Clone, Copy)]
enum ArtifactRole {
    Cell,
    Registration,
}

impl ArtifactRole {
    fn definition(self) -> StrongDefinitionRole {
        match self {
            Self::Cell => StrongDefinitionRole::InitializationCell,
            Self::Registration => StrongDefinitionRole::InitializationRegistration,
        }
    }
    fn symbol(self, unit: PersistentInitializationUnitId) -> PersistentSymbolKey {
        match self {
            Self::Cell => PersistentSymbolKey::InitializationCell(unit),
            Self::Registration => PersistentSymbolKey::InitializationRegistration(unit),
        }
    }
}

fn artifact(
    unit: PersistentInitializationUnitId,
    role: ArtifactRole,
    foundation: &ConeLirFoundation,
) -> Result<StrongInitializationArtifactRefV2, Error> {
    let entity = StrongDefinitionEntity::initialization_unit(unit);
    let definition_role = role.definition();
    let definition =
        foundation
            .definition_for(entity, definition_role)
            .ok_or(Error::MissingDefinition {
                entity,
                role: definition_role,
            })?;
    let linkage = match definition.key().owner() {
        ObjectDefinitionPlanOwner::Strong { .. } => LinkageClass::ConeStrong,
        ObjectDefinitionPlanOwner::Odr { .. } => LinkageClass::OdrWeak,
    };
    let symbol = PersistentSymbolRequest::new(role.symbol(unit), linkage).map_err(Error::Symbol)?;

    if !foundation.contains_symbol_request(symbol) {
        return Err(Error::MissingSymbol(symbol));
    }

    let mut primary = foundation
        .definition_atoms_for_plan(definition.id())
        .filter(|record| record.key().role() == DefinitionAtomRole::Primary);
    let primary = match (primary.next(), primary.next()) {
        (Some(record), None) => record.id(),
        _ => return Err(Error::PrimaryAtoms(definition.id())),
    };

    let mut associated = foundation
        .definition_atoms_for_plan(definition.id())
        .filter(|record| record.key().role() != DefinitionAtomRole::Primary);
    match role {
        ArtifactRole::Registration => {
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
        ArtifactRole::Cell => {
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
