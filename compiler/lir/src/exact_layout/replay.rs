use scoop_identity::{ExactTypeKey, RepresentationRole, ScanKey, ScanRole};
use scoop_wire::{BudgetMeter, WirePath};

use super::*;
use crate::{
    ExternalStrongShapeSubjectV1, OdrFreeLirFoundation, RefScan, StrongShapeDefinitionRefV1,
    ValueLayoutConstituentV1, ValueStorageLayoutV1,
};

mod aggregate;
mod enumeration;
mod instance;
mod scalar;

pub use aggregate::NominalLayoutFieldInputV1;
pub use enumeration::{EnumLayoutFieldInputV1, EnumLayoutVariantInputV1};
pub use instance::ClassLayoutBaseV1;

fn nominal(
    key: &ExactTypeKey,
) -> Result<scoop_identity::NominalDeclarationOwner, ExactLayoutReplayError> {
    if is_unit(key) {
        return Err(ExactLayoutReplayError::IdentityKind);
    }
    use scoop_identity::NominalDeclarationOwner as Owner;
    match key {
        ExactTypeKey::Nominal(owner) => Ok(Owner::Concrete(*owner)),
        ExactTypeKey::NominalApplication { origin, .. } => Ok(Owner::GenericTemplate(*origin)),
        _ => Err(ExactLayoutReplayError::IdentityKind),
    }
}

fn is_unit(key: &ExactTypeKey) -> bool {
    key == &ExactTypeKey::Nominal(
        scoop_identity::CoreBuiltinNominal::Unit
            .identity_record()
            .id(),
    )
}

fn scan_binding(
    identity: &ExactLayoutIdentityV1,
    role: ScanRole,
    foundation: &OdrFreeLirFoundation,
    meter: &mut BudgetMeter,
) -> Result<ScanBinding, ExactLayoutReplayError> {
    let path = WirePath::root();
    if identity.physical_definition().provider() != foundation.producer() {
        return Err(ExactLayoutReplayError::ProviderMismatch);
    }
    let key = ScanKey::new(identity.layout(), role);
    meter.charge_work(foundation.scans().len() as u64, &path)?;
    let scan = foundation
        .scans()
        .iter()
        .find(|record| *record.key() == key)
        .ok_or(ExactLayoutReplayError::MissingScan)?;
    let physical = StrongShapeDefinitionRefV1::from_foundation(
        ExternalStrongShapeSubjectV1::Scan(scan.id()),
        foundation,
        meter,
    )?;
    Ok(ScanBinding {
        id: scan.id(),
        physical,
    })
}

fn finish_value(
    identity: ExactLayoutIdentityV1,
    storage: ValueStorageLayoutV1,
    representation: ValueRepresentation,
    foundation: &OdrFreeLirFoundation,
    meter: &mut BudgetMeter,
) -> Result<ExactValueLayoutV1, ExactLayoutReplayError> {
    let scan = scan_binding(&identity, ScanRole::InlineValue, foundation, meter)?;
    let value =
        ValueLayoutConstituentV1::new(identity.target(), identity.layout_key().clone(), storage)?;
    Ok(ExactValueLayoutV1 {
        identity,
        value,
        representation: ExactRepresentationLayoutV1(representation),
        scan,
    })
}

fn require_roles(
    identity: &ExactLayoutIdentityV1,
    roles: &[RepresentationRole],
) -> Result<(), ExactLayoutReplayError> {
    if roles.contains(&identity.layout_key().representation()) {
        Ok(())
    } else {
        Err(ExactLayoutReplayError::RepresentationRole)
    }
}

fn reserve<T>(count: usize, meter: &mut BudgetMeter) -> Result<Vec<T>, ExactLayoutReplayError> {
    let mut values = Vec::new();
    meter.try_reserve_collection_slots(&mut values, count, &WirePath::root())?;
    Ok(values)
}

pub(super) fn storage_scan(storage: &ValueStorageLayoutV1) -> &RefScan {
    static NONE: RefScan = RefScan::None;
    match storage.kind() {
        crate::ValueStorageKindV1::ZeroSized { .. } => &NONE,
        crate::ValueStorageKindV1::Inline { scan, .. } => scan.as_ref_scan(),
    }
}
