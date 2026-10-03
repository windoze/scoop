//! Canonical relation checks for the cross-Cone LIR bridge.

use super::{
    CrossConeLirBridgeRelationError, ParamFreeLirCallableBuildError, ParamFreeLirCallableExportV1,
    SelectedDependencyLirCallableV1,
};
use crate::{
    CallableAbiBuildError, CallableAbiRecordV1, CallableAbiValidationError, CallingConvention,
    ConeLirFoundation, ExternalCallableRootPlan,
};
use scoop_identity::{
    ConeIdentity, DependencyCallableDeclarationId, StrongCallableDefinitionOwner,
};

pub(super) fn build_callable(
    provider: ConeIdentity,
    declaration: DependencyCallableDeclarationId,
    target: StrongCallableDefinitionOwner,
    abi_signature: scoop_identity::CanonicalScoopAbiFunctionSignature,
    calling_convention: CallingConvention,
    root_plan: ExternalCallableRootPlan,
) -> Result<ParamFreeLirCallableExportV1, ParamFreeLirCallableBuildError> {
    let callable = CallableAbiRecordV1::new(
        provider,
        target,
        abi_signature,
        calling_convention,
        root_plan,
    )
    .map_err(|error| match error {
        CallableAbiBuildError::Suspend => ParamFreeLirCallableBuildError::Suspend { declaration },
        CallableAbiBuildError::RootProtocolMismatch { abi, root } => {
            ParamFreeLirCallableBuildError::RootProtocolMismatch {
                declaration,
                abi,
                root,
            }
        }
        CallableAbiBuildError::Contract(error) => ParamFreeLirCallableBuildError::Contract(error),
    })?;
    ParamFreeLirCallableExportV1::from_abi(declaration, callable)
}

pub(super) fn validate_section_relations(
    foundation: &ConeLirFoundation,
    exports: &[ParamFreeLirCallableExportV1],
    selected: &[SelectedDependencyLirCallableV1],
) -> Result<(), CrossConeLirBridgeRelationError> {
    let producer = foundation.producer();

    for (index, export) in exports.iter().enumerate() {
        validate_export(index, export, foundation)?;
    }
    validate_selected(producer, selected)
}

pub(super) fn validate_selected(
    producer: ConeIdentity,
    selected: &[SelectedDependencyLirCallableV1],
) -> Result<(), CrossConeLirBridgeRelationError> {
    for (index, selected) in selected.iter().enumerate() {
        if selected.provider == producer {
            return Err(CrossConeLirBridgeRelationError::SelectedCurrentProvider {
                index,
                provider: selected.provider,
            });
        }

        selected
            .bridge
            .callable
            .link_contract(selected.provider)
            .map_err(|error| match error {
                CallableAbiValidationError::Contract(error) => {
                    CrossConeLirBridgeRelationError::Contract(error)
                }
                _ => CrossConeLirBridgeRelationError::SelectedContractMismatch {
                    index,
                    provider: selected.provider,
                    declaration: selected.bridge.declaration,
                },
            })?;
    }
    Ok(())
}

pub(super) fn canonical_order(
    exports: &mut [ParamFreeLirCallableExportV1],
    selected: &mut [SelectedDependencyLirCallableV1],
) -> Result<(), super::CrossConeLirBridgeBuildError> {
    use super::CrossConeLirBridgeBuildError as Error;

    exports.sort_unstable_by_key(ParamFreeLirCallableExportV1::declaration);
    reject_duplicate_exports(exports).map_err(Error::DuplicateExport)?;
    selected.sort_unstable_by_key(SelectedDependencyLirCallableV1::sort_key);
    reject_duplicate_selected(selected).map_err(|(provider, declaration)| {
        Error::DuplicateSelected {
            provider,
            declaration,
        }
    })
}

fn validate_export(
    index: usize,
    export: &ParamFreeLirCallableExportV1,
    foundation: &ConeLirFoundation,
) -> Result<(), CrossConeLirBridgeRelationError> {
    export
        .callable
        .validate_against(foundation)
        .map_err(|error| match error {
            CallableAbiValidationError::Contract(error) => {
                CrossConeLirBridgeRelationError::Contract(error)
            }
            CallableAbiValidationError::ContractMismatch => {
                CrossConeLirBridgeRelationError::ExportContractMismatch {
                    index,
                    declaration: export.declaration,
                }
            }
            CallableAbiValidationError::MissingBody(body) => {
                CrossConeLirBridgeRelationError::MissingExportBody { index, body }
            }
            CallableAbiValidationError::MissingSymbol(symbol) => {
                CrossConeLirBridgeRelationError::MissingExportSymbol { index, symbol }
            }
            CallableAbiValidationError::MissingDefinition(definition) => {
                CrossConeLirBridgeRelationError::MissingExportDefinition { index, definition }
            }
            CallableAbiValidationError::DefinitionMismatch(definition) => {
                CrossConeLirBridgeRelationError::ExportDefinitionMismatch { index, definition }
            }
        })
}

pub(super) fn reject_duplicate_exports(
    exports: &[ParamFreeLirCallableExportV1],
) -> Result<(), DependencyCallableDeclarationId> {
    exports
        .windows(2)
        .find(|pair| pair[0].declaration == pair[1].declaration)
        .map_or(Ok(()), |pair| Err(pair[0].declaration))
}

pub(super) fn reject_duplicate_selected(
    selected: &[SelectedDependencyLirCallableV1],
) -> Result<(), (ConeIdentity, DependencyCallableDeclarationId)> {
    selected
        .windows(2)
        .find(|pair| pair[0].sort_key() == pair[1].sort_key())
        .map_or(Ok(()), |pair| Err(pair[0].sort_key()))
}
