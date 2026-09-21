//! Original defaults cannot originate on overrides (language 8.5.1).
use super::*;
use crate::cross_cone_type_semantics::source_authority::binding_keys;
use scoop_identity::{DispatchSlotKey, PersistentDispatchSlotId, SourceDeclarationKind};

mod errors;
pub use errors::DefaultSourceProviderSlotError;
type SlotError = DefaultSourceProviderSlotError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DefaultSourceProviderDispatchV1 {
    Direct,
    RootSlot(PersistentDispatchSlotId),
}

pub(super) fn validate(
    provider: &BoundNominalParameterProtocolsV1<'_, '_, '_, '_>,
    declaration: &DeclarationFacts<'_>,
    direct: &DefaultSourceAccessDomainV1,
    references: &DefaultSourceReferenceClosureV1<'_>,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<DefaultSourceProviderDispatchV1, Error> {
    let dispatch = classify(provider, declaration, meter, path)?;
    for occurrence in references.occurrences() {
        meter.charge_work(1, path)?;
        let valid = match (dispatch, occurrence.source().witness().slot_call_domain()) {
            (
                DefaultSourceProviderDispatchV1::Direct,
                OptionalDefaultSourceSlotDomainV1::Absent,
            ) => true,
            (
                DefaultSourceProviderDispatchV1::RootSlot(_),
                OptionalDefaultSourceSlotDomainV1::Present(actual),
            ) => {
                let expected_bytes =
                    scoop_wire::encoded_length(direct).map_err(SlotError::Encoding)?;
                let actual_bytes =
                    scoop_wire::encoded_length(actual).map_err(SlotError::Encoding)?;
                meter.charge_work(expected_bytes.saturating_add(actual_bytes), path)?;
                actual == direct
            }
            _ => false,
        };
        if !valid {
            return Err(SlotError::Witness {
                kind: occurrence.source().kind(),
                index: occurrence.index(),
            }
            .into());
        }
    }
    Ok(dispatch)
}

fn classify(
    provider: &BoundNominalParameterProtocolsV1<'_, '_, '_, '_>,
    declaration: &DeclarationFacts<'_>,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<DefaultSourceProviderDispatchV1, Error> {
    meter.charge_nodes(1, path)?;
    meter.charge_edges(1, path)?;
    meter.charge_work(1, path)?;
    let source = declaration.provider;
    let slots = source.slot_relations().slots();
    if source.modality() == CallableModalityV1::Final {
        if !slots.is_empty() {
            return Err(SlotError::SlotCount {
                expected: 0,
                actual: slots.len(),
            }
            .into());
        }
        return Ok(DefaultSourceProviderDispatchV1::Direct);
    }
    let [slot] = slots else {
        return Err(SlotError::SlotCount {
            expected: 1,
            actual: slots.len(),
        }
        .into());
    };
    let CallableTemplateOrigin::Function(function) = declaration.definition_root.declaration()
    else {
        return Err(SlotError::ProviderDeclaration.into());
    };
    let foundation = provider.members().nominals.foundation;
    sources::query(
        foundation.source().entries().sources.records().len(),
        meter,
        path,
    )?;
    let owner = foundation
        .nominal_key(source.owner())
        .map_err(SlotError::Foundation)?;
    let expected = match (owner.declaration_kind(), source.modality()) {
        (SourceDeclarationKind::Class, CallableModalityV1::Open | CallableModalityV1::Abstract) => {
            DispatchSlotKey::virtual_method(function)
        }
        (
            SourceDeclarationKind::Interface,
            CallableModalityV1::InterfaceDefault | CallableModalityV1::Abstract,
        ) => DispatchSlotKey::interface_method(function),
        _ => return Err(SlotError::ProviderDeclaration.into()),
    };
    let records = foundation
        .foundation
        .as_canonical()
        .type_source_dispatch_records();
    meter.charge_work((records.len() as u64).saturating_mul(65), path)?;
    let record = records
        .iter()
        .find(|record| record.id() == *slot)
        .ok_or(SlotError::MissingSlot(*slot))?;
    binding_keys::verify(*slot, record.key(), foundation.identities, meter, path)
        .map_err(SlotError::Foundation)?;
    if record.key() != &expected {
        return Err(SlotError::RootSlot(*slot).into());
    }
    Ok(DefaultSourceProviderDispatchV1::RootSlot(*slot))
}
