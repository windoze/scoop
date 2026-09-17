//! Canonical relation checks for the cross-Cone LIR bridge.

use scoop_identity::{
    CallableBodyKey, ConeIdentity, DependencyCallableDeclarationId, Effect, LinkageClass,
    ObjectDefinitionPlanId, ObjectDefinitionPlanKey, PersistentCallableBodyId, PersistentSymbolKey,
    PersistentSymbolRequest, StrongCallableDefinitionOwner, StrongDefinitionEntity,
    StrongDefinitionRole,
};

use crate::{
    CallingConvention, OdrFreeLirFoundation, StrongObjectSymbolSurfaceV1,
    cross_cone_bridge::{
        CrossConeLirBridgeRelationError, DependencyExternalCallableRootPlanV1,
        ParamFreeLirCallableBuildError, ParamFreeLirCallableExportV1,
        SelectedDependencyLirCallableV1,
    },
};

pub(super) fn build_callable(
    provider: ConeIdentity,
    declaration: DependencyCallableDeclarationId,
    target: StrongCallableDefinitionOwner,
    abi_signature: scoop_identity::CanonicalScoopAbiFunctionSignature,
    calling_convention: CallingConvention,
    root_plan: DependencyExternalCallableRootPlanV1,
) -> Result<ParamFreeLirCallableExportV1, ParamFreeLirCallableBuildError> {
    let expected_target = declaration.implementation();
    if target != expected_target {
        return Err(ParamFreeLirCallableBuildError::TargetMismatch {
            declaration,
            expected: expected_target,
            actual: target,
        });
    }
    if abi_signature.signature().effect() != Effect::Ordinary {
        return Err(ParamFreeLirCallableBuildError::Suspend { declaration });
    }
    if root_plan.gc_effect() != abi_signature.gc_effect() {
        return Err(ParamFreeLirCallableBuildError::RootProtocolMismatch {
            declaration,
            abi: abi_signature.gc_effect(),
            root: root_plan.gc_effect(),
        });
    }
    let (_, expected_symbol, required_definition) =
        derive_link_contract(provider, target).map_err(ParamFreeLirCallableBuildError::Contract)?;
    Ok(ParamFreeLirCallableExportV1 {
        declaration,
        target,
        abi_signature,
        expected_symbol,
        calling_convention,
        root_plan,
        required_definition,
    })
}

pub(super) fn validate_section_relations(
    foundation: &OdrFreeLirFoundation,
    exports: &[ParamFreeLirCallableExportV1],
    selected: &[SelectedDependencyLirCallableV1],
) -> Result<(), CrossConeLirBridgeRelationError> {
    let producer = foundation.producer();
    if producer == ConeIdentity::CORE && !exports.is_empty() {
        return Err(CrossConeLirBridgeRelationError::CoreExportsOrdinaryDependencyCallable);
    }
    let definitions = StrongObjectSymbolSurfaceV1::from_odr_free_foundation(foundation)
        .map_err(CrossConeLirBridgeRelationError::DefinitionSurface)?;
    for (index, export) in exports.iter().enumerate() {
        validate_export(producer, index, export, foundation, &definitions)?;
    }
    for (index, selected) in selected.iter().enumerate() {
        if selected.provider == producer {
            return Err(CrossConeLirBridgeRelationError::SelectedCurrentProvider {
                index,
                provider: selected.provider,
            });
        }
        if selected.provider == ConeIdentity::CORE {
            return Err(CrossConeLirBridgeRelationError::SelectedTrustedCore { index });
        }
        let (_, symbol, definition) =
            derive_link_contract(selected.provider, selected.bridge.target)
                .map_err(CrossConeLirBridgeRelationError::Contract)?;
        if selected.bridge.expected_symbol != symbol
            || selected.bridge.required_definition != definition
        {
            return Err(CrossConeLirBridgeRelationError::SelectedContractMismatch {
                index,
                provider: selected.provider,
                declaration: selected.bridge.declaration,
            });
        }
    }
    Ok(())
}

fn validate_export(
    producer: ConeIdentity,
    index: usize,
    export: &ParamFreeLirCallableExportV1,
    foundation: &OdrFreeLirFoundation,
    definitions: &StrongObjectSymbolSurfaceV1,
) -> Result<(), CrossConeLirBridgeRelationError> {
    let (body, symbol, definition) = derive_link_contract(producer, export.target)
        .map_err(CrossConeLirBridgeRelationError::Contract)?;
    if export.expected_symbol != symbol || export.required_definition != definition {
        return Err(CrossConeLirBridgeRelationError::ExportContractMismatch {
            index,
            declaration: export.declaration,
        });
    }
    if !foundation.contains_callable_body(body) {
        return Err(CrossConeLirBridgeRelationError::MissingExportBody { index, body });
    }
    if !foundation.contains_symbol_request(symbol) {
        return Err(CrossConeLirBridgeRelationError::MissingExportSymbol { index, symbol });
    }
    let Some(plan) = definitions.plan(definition) else {
        return Err(CrossConeLirBridgeRelationError::MissingExportDefinition { index, definition });
    };
    if plan.owner() != StrongDefinitionEntity::callable_body(body)
        || plan.definition_role() != StrongDefinitionRole::CallableBody
        || plan.primary_symbol() != symbol
    {
        return Err(CrossConeLirBridgeRelationError::ExportDefinitionMismatch {
            index,
            definition,
        });
    }
    Ok(())
}

fn derive_link_contract(
    provider: ConeIdentity,
    target: StrongCallableDefinitionOwner,
) -> Result<
    (
        PersistentCallableBodyId,
        PersistentSymbolRequest,
        ObjectDefinitionPlanId,
    ),
    ParamFreeLirCallableContractError,
> {
    let body = PersistentCallableBodyId::from_key(&CallableBodyKey::strong(target))
        .map_err(ParamFreeLirCallableContractError::Identity)?;
    let expected_symbol = PersistentSymbolRequest::new(
        PersistentSymbolKey::CallableBody(body),
        LinkageClass::ConeStrong,
    )
    .map_err(ParamFreeLirCallableContractError::Symbol)?;
    let definition_key = ObjectDefinitionPlanKey::strong(
        provider,
        StrongDefinitionEntity::callable_body(body),
        StrongDefinitionRole::CallableBody,
    )
    .map_err(ParamFreeLirCallableContractError::Definition)?;
    let required_definition = ObjectDefinitionPlanId::from_key(&definition_key)
        .map_err(ParamFreeLirCallableContractError::Identity)?;
    Ok((body, expected_symbol, required_definition))
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

use super::ParamFreeLirCallableContractError;
