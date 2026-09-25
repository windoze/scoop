use scoop_identity::{
    ConeIdentity, LinkageClass, ObjectDefinitionPlanId, ObjectDefinitionPlanOwner,
    ObjectDefinitionPlanRole, PersistentSymbolRequest,
};
use scoop_lir::{ExternalShapeLinkImportV1, ExternalStrongShapeSubjectV1};
use scoop_wire::{BudgetMeter, WirePath};

use super::{
    CrossConeLayoutTerminalArtifactV1, CrossConeLayoutTerminalValidationError, import_context,
};
use crate::{CanonicalDefinedLinkSymbolOwnerSetV1, LinkDefinitionOwnerV1, StrongDefinitionOwnerV1};

pub(super) fn validate(
    consumer: &CrossConeLayoutTerminalArtifactV1<'_>,
    import: &ExternalShapeLinkImportV1<'_>,
    terminal: &CrossConeLayoutTerminalArtifactV1<'_>,
    artifacts: &[CrossConeLayoutTerminalArtifactV1<'_>],
    meter: &mut BudgetMeter,
) -> Result<(), CrossConeLayoutTerminalValidationError> {
    validate_sets(
        consumer.identity(),
        terminal.section.target_profile(),
        import,
        terminal.defined_symbols,
        artifacts.iter().map(|artifact| artifact.defined_symbols),
        meter,
    )
}

pub(super) fn validate_sets<'a>(
    consumer: ConeIdentity,
    target: scoop_lir::LirTargetProfile,
    import: &ExternalShapeLinkImportV1<'_>,
    terminal: &CanonicalDefinedLinkSymbolOwnerSetV1,
    artifacts: impl Iterator<Item = &'a CanonicalDefinedLinkSymbolOwnerSetV1>,
    meter: &mut BudgetMeter,
) -> Result<(), CrossConeLayoutTerminalValidationError> {
    let path = WirePath::root();
    // A subject carries only fixed-width ids and a closed role. Account for
    // its identity preimage before deriving the expected definition.
    meter.charge_owned_bytes(512, &path)?;
    meter.charge_work(512, &path)?;
    let (definition, symbol, owner) = expected_definition(import.provider(), import.subject())?;
    if import.required_definition() != definition || import.expected_symbol() != symbol {
        return Err(
            CrossConeLayoutTerminalValidationError::DerivedDefinitionMismatch(import_context(
                consumer, import,
            )),
        );
    }
    let name = normalized_symbol(target, symbol, meter)?;
    charge_lookup(terminal, &name, meter)?;
    let actual = find_owner(terminal, &name).ok_or(
        CrossConeLayoutTerminalValidationError::MissingStrongDefinition {
            context: import_context(consumer, import),
            definition,
            symbol,
        },
    )?;
    if actual != owner {
        return Err(
            CrossConeLayoutTerminalValidationError::StrongOwnerMismatch {
                context: import_context(consumer, import),
                expected: owner,
                actual,
            },
        );
    }
    reject_foreign_owners(consumer, import, artifacts, &name, symbol, meter)
}

pub(super) fn reject_foreign_owners<'a>(
    consumer: ConeIdentity,
    import: &ExternalShapeLinkImportV1<'_>,
    artifacts: impl Iterator<Item = &'a CanonicalDefinedLinkSymbolOwnerSetV1>,
    name: &[u8],
    symbol: PersistentSymbolRequest,
    meter: &mut BudgetMeter,
) -> Result<(), CrossConeLayoutTerminalValidationError> {
    for artifact in artifacts {
        meter.charge_work(1, &WirePath::root())?;
        if artifact.producer() == import.provider() {
            continue;
        }
        charge_lookup(artifact, name, meter)?;
        if find_owner(artifact, name).is_some() {
            meter.charge_owned_bytes(
                std::mem::size_of::<super::CrossConeLayoutTerminalImportContextV1>() as u64,
                &WirePath::root(),
            )?;
            return Err(
                CrossConeLayoutTerminalValidationError::ForeignStrongDefinition {
                    context: import_context(consumer, import),
                    actual_owner: artifact.producer(),
                    symbol,
                },
            );
        }
    }
    Ok(())
}

pub(super) fn normalized_symbol(
    target: scoop_lir::LirTargetProfile,
    symbol: PersistentSymbolRequest,
    meter: &mut BudgetMeter,
) -> Result<Vec<u8>, scoop_wire::WireError> {
    let path = WirePath::root();
    let logical = scoop_identity::MangledSymbol::byte_length_for_key(&symbol.key()) as u64;
    meter.charge_owned_bytes(logical.saturating_mul(2).saturating_add(1), &path)?;
    meter.charge_work(logical.saturating_mul(2).saturating_add(1), &path)?;
    Ok(target
        .contract()
        .native_symbol_normalization()
        .compiler_generated_object_symbol(symbol.symbol().as_str())
        .into_bytes())
}

fn charge_lookup(
    owners: &CanonicalDefinedLinkSymbolOwnerSetV1,
    name: &[u8],
    meter: &mut BudgetMeter,
) -> Result<(), scoop_wire::WireError> {
    meter.charge_work(
        (name.len() as u64 + 1).saturating_mul(u64::from(owners.owners().len().max(1).ilog2()) + 1),
        &WirePath::root(),
    )
}

fn expected_definition(
    provider: ConeIdentity,
    subject: ExternalStrongShapeSubjectV1,
) -> Result<
    (
        ObjectDefinitionPlanId,
        PersistentSymbolRequest,
        LinkDefinitionOwnerV1,
    ),
    CrossConeLayoutTerminalValidationError,
> {
    let (key, symbol) = subject.expected_definition(provider)?;
    let definition = ObjectDefinitionPlanId::from_key(&key)?;
    let symbol = PersistentSymbolRequest::new(symbol, LinkageClass::ConeStrong)?;
    let (
        ObjectDefinitionPlanOwner::Strong { producer, entity },
        ObjectDefinitionPlanRole::Strong(role),
    ) = (key.owner(), key.definition_role())
    else {
        return Err(CrossConeLayoutTerminalValidationError::InvalidStrongOwner {
            provider,
            subject,
        });
    };
    if producer != provider {
        return Err(CrossConeLayoutTerminalValidationError::InvalidStrongOwner {
            provider,
            subject,
        });
    }
    let owner = StrongDefinitionOwnerV1::new(entity, role)
        .map(LinkDefinitionOwnerV1::StrongDefinition)
        .map_err(
            |_| CrossConeLayoutTerminalValidationError::InvalidStrongOwner { provider, subject },
        )?;
    Ok((definition, symbol, owner))
}

fn find_owner(
    definitions: &CanonicalDefinedLinkSymbolOwnerSetV1,
    symbol: &[u8],
) -> Option<LinkDefinitionOwnerV1> {
    definitions
        .owners()
        .binary_search_by(|candidate| candidate.symbol().cmp(symbol))
        .ok()
        .map(|position| definitions.owners()[position].owner())
}
