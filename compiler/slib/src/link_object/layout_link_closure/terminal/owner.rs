use scoop_identity::{
    ConeIdentity, LinkageClass, ObjectDefinitionPlanId, ObjectDefinitionPlanOwner,
    ObjectDefinitionPlanRole, PersistentSymbolRequest,
};
use scoop_lir::{ExternalShapeLinkImportV1, ExternalStrongShapeSubjectV1};

use super::{
    CrossConeLayoutTerminalArtifactV1, CrossConeLayoutTerminalValidationError, import_context,
};
use crate::{CanonicalDefinedLinkSymbolOwnerSetV1, LinkDefinitionOwnerV1, StrongDefinitionOwnerV1};

pub(super) fn validate(
    consumer: &CrossConeLayoutTerminalArtifactV1<'_>,
    import: &ExternalShapeLinkImportV1<'_>,
    terminal: &CrossConeLayoutTerminalArtifactV1<'_>,
    artifacts: &[CrossConeLayoutTerminalArtifactV1<'_>],
) -> Result<(), CrossConeLayoutTerminalValidationError> {
    validate_sets(
        consumer.identity(),
        terminal.section.target_profile(),
        import,
        terminal.defined_symbols,
        artifacts.iter().map(|artifact| artifact.defined_symbols),
    )
}

pub(super) fn validate_sets<'a>(
    consumer: ConeIdentity,
    target: scoop_lir::LirTargetProfile,
    import: &ExternalShapeLinkImportV1<'_>,
    terminal: &CanonicalDefinedLinkSymbolOwnerSetV1,
    artifacts: impl Iterator<Item = &'a CanonicalDefinedLinkSymbolOwnerSetV1>,
) -> Result<(), CrossConeLayoutTerminalValidationError> {
    // A subject carries only fixed-width ids and a closed role. Account for
    // its identity preimage before deriving the expected definition.

    let (definition, symbol, owner) = expected_definition(import.provider(), import.subject())?;
    if import.required_definition() != definition || import.expected_symbol() != symbol {
        return Err(
            CrossConeLayoutTerminalValidationError::DerivedDefinitionMismatch(import_context(
                consumer, import,
            )),
        );
    }
    let name = normalized_symbol(target, symbol);

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
    reject_foreign_owners(consumer, import, artifacts, &name, symbol)
}

pub(super) fn reject_foreign_owners<'a>(
    consumer: ConeIdentity,
    import: &ExternalShapeLinkImportV1<'_>,
    artifacts: impl Iterator<Item = &'a CanonicalDefinedLinkSymbolOwnerSetV1>,
    name: &[u8],
    symbol: PersistentSymbolRequest,
) -> Result<(), CrossConeLayoutTerminalValidationError> {
    for artifact in artifacts {
        if artifact.producer() == import.provider() {
            continue;
        }

        if find_owner(artifact, name).is_some() {
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
) -> Vec<u8> {
    target
        .contract()
        .native_symbol_normalization()
        .compiler_generated_object_symbol(symbol.symbol().as_str())
        .into_bytes()
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
