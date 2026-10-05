use scoop_identity::{CborIdentityRecord, InitializationUnitKey, PropertyOwner};

use super::*;
use crate::{CanonicalPropertyInterfacesV1, PublicDeclarationOwnerV1, SourceNominalId};

/// The source unit table is already part of the provider foundation. This
/// lookup does not infer transitive initialization from a callable body.
pub(crate) fn accessor_initialization_unit(
    accessor: PersistentPropertyAccessorId,
    properties: &CanonicalPropertyInterfacesV1,
    units: &[CborIdentityRecord<PersistentInitializationUnitId, InitializationUnitKey>],
) -> Result<Option<PersistentInitializationUnitId>, Error> {
    let mut declarations = properties.all_declarations().filter(|property| {
        property.accessors().getter() == accessor || property.accessors().setter() == Some(accessor)
    });
    let property = declarations
        .next()
        .ok_or(Error::MissingAccessor(accessor))?;
    if declarations.next().is_some() {
        return Err(Error::DuplicateAccessor(accessor));
    }
    let mut matches = units.iter().filter(|record| match record.key() {
        InitializationUnitKey::TopLevelProperty(id) => {
            property.owner() == PublicDeclarationOwnerV1::TopLevel
                && property.declaration() == PropertyOwner::Property(*id)
        }
        InitializationUnitKey::ExtensionProperty(id) => {
            property.owner() == PublicDeclarationOwnerV1::Extension
                && property.declaration() == PropertyOwner::ExtensionProperty(*id)
        }
        InitializationUnitKey::Object(owner) | InitializationUnitKey::Companion(owner) => {
            property.owner() == PublicDeclarationOwnerV1::Nominal(SourceNominalId::Concrete(*owner))
        }
        InitializationUnitKey::GenericCompanionTemplate(owner) => {
            property.owner()
                == PublicDeclarationOwnerV1::Nominal(SourceNominalId::GenericTemplate(*owner))
        }
        InitializationUnitKey::GenericCompanionApplication { .. }
        | InitializationUnitKey::GenericDelegatedExtensionApplication { .. } => false,
    });
    let unit = matches.next().map(CborIdentityRecord::id);
    if matches.next().is_some() {
        return Err(Error::DuplicateUnit(accessor));
    }
    Ok(unit)
}
