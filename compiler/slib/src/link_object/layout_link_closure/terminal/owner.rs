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
    let (definition, symbol, owner) = expected_definition(import.provider(), import.subject())?;
    if import.required_definition() != definition || import.expected_symbol() != symbol {
        return Err(
            CrossConeLayoutTerminalValidationError::DerivedDefinitionMismatch(import_context(
                consumer.identity(),
                import,
            )),
        );
    }
    let name = terminal
        .section
        .target_profile()
        .contract()
        .native_symbol_normalization()
        .compiler_generated_object_symbol(symbol.symbol().as_str())
        .into_bytes();
    let path = WirePath::root();
    meter.charge_work(terminal.defined_symbols.owners().len() as u64, &path)?;
    let actual = find_owner(terminal.defined_symbols, &name).ok_or(
        CrossConeLayoutTerminalValidationError::MissingStrongDefinition {
            context: import_context(consumer.identity(), import),
            definition,
            symbol,
        },
    )?;
    if actual != owner {
        return Err(
            CrossConeLayoutTerminalValidationError::StrongOwnerMismatch {
                context: import_context(consumer.identity(), import),
                expected: owner,
                actual,
            },
        );
    }
    reject_foreign_owners(consumer, import, artifacts, &name, symbol, meter)
}

fn reject_foreign_owners(
    consumer: &CrossConeLayoutTerminalArtifactV1<'_>,
    import: &ExternalShapeLinkImportV1<'_>,
    artifacts: &[CrossConeLayoutTerminalArtifactV1<'_>],
    name: &[u8],
    symbol: PersistentSymbolRequest,
    meter: &mut BudgetMeter,
) -> Result<(), CrossConeLayoutTerminalValidationError> {
    let path = WirePath::root();
    for artifact in artifacts {
        if artifact.identity() == import.provider() {
            continue;
        }
        meter.charge_work(artifact.defined_symbols.owners().len() as u64, &path)?;
        if find_owner(artifact.defined_symbols, name).is_some() {
            return Err(
                CrossConeLayoutTerminalValidationError::ForeignStrongDefinition {
                    context: import_context(consumer.identity(), import),
                    actual_owner: artifact.identity(),
                    symbol,
                },
            );
        }
    }
    Ok(())
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
