use scoop_identity::{
    ConeIdentity, DependencyCallableDeclarationId, ExactCallableSignature,
    StrongCallableDefinitionOwner,
};

use crate::{CallableSignatureSubject, OdrFreeMirFoundation};

use super::errors::{
    CrossConeMirBridgeRelationError, CrossConeMirBridgeValidationError,
    ParamFreeMirCallableBuildError,
};
use super::model::{ParamFreeMirCallableExportV1, SelectedDependencyMirCallableV1};

pub(super) fn validate_callable_shape(
    declaration: DependencyCallableDeclarationId,
    implementation: StrongCallableDefinitionOwner,
    signature: &ExactCallableSignature,
) -> Result<(), ParamFreeMirCallableBuildError> {
    let expected = declaration.implementation();
    if implementation != expected {
        return Err(ParamFreeMirCallableBuildError::ImplementationMismatch {
            declaration,
            expected,
            actual: implementation,
        });
    }
    if signature.effect() != scoop_identity::Effect::Ordinary {
        return Err(ParamFreeMirCallableBuildError::Suspend { declaration });
    }
    Ok(())
}

pub(super) fn validate_section_relations(
    artifact: ConeIdentity,
    foundation: &OdrFreeMirFoundation,
    exports: &[ParamFreeMirCallableExportV1],
    selected: &[SelectedDependencyMirCallableV1],
) -> Result<(), CrossConeMirBridgeRelationError> {
    if artifact == ConeIdentity::CORE && !exports.is_empty() {
        return Err(CrossConeMirBridgeRelationError::CoreExportsOrdinaryDependencyCallable);
    }
    for (index, export) in exports.iter().enumerate() {
        let subject = CallableSignatureSubject::Strong(export.implementation.callable_owner());
        let Some(expected) = foundation
            .as_canonical()
            .callable_signatures()
            .iter()
            .find(|record| record.subject() == subject)
        else {
            return Err(
                CrossConeMirBridgeRelationError::MissingExportImplementation {
                    index,
                    implementation: export.implementation,
                },
            );
        };
        if expected.signature() != &export.signature {
            return Err(CrossConeMirBridgeRelationError::ExportSignatureMismatch {
                index,
                implementation: export.implementation,
            });
        }
    }
    for (index, selected) in selected.iter().enumerate() {
        if selected.provider == artifact {
            return Err(CrossConeMirBridgeRelationError::SelectedCurrentProvider {
                index,
                provider: selected.provider,
            });
        }
        if selected.provider == ConeIdentity::CORE {
            return Err(CrossConeMirBridgeRelationError::SelectedTrustedCore { index });
        }
    }
    Ok(())
}

pub(super) fn reject_duplicate_exports(
    exports: &[ParamFreeMirCallableExportV1],
) -> Result<(), DependencyCallableDeclarationId> {
    exports
        .windows(2)
        .find(|pair| pair[0].declaration == pair[1].declaration)
        .map_or(Ok(()), |pair| Err(pair[0].declaration))
}

pub(super) fn reject_duplicate_selected(
    selected: &[SelectedDependencyMirCallableV1],
) -> Result<(), (ConeIdentity, DependencyCallableDeclarationId)> {
    selected
        .windows(2)
        .find(|pair| pair[0].sort_key() == pair[1].sort_key())
        .map_or(Ok(()), |pair| Err(pair[0].sort_key()))
}

pub(super) fn validate_export_order(
    exports: &[ParamFreeMirCallableExportV1],
    record: &ParamFreeMirCallableExportV1,
    index: usize,
) -> Result<(), CrossConeMirBridgeValidationError> {
    let Some(previous) = exports.last() else {
        return Ok(());
    };
    if previous.declaration == record.declaration {
        return Err(CrossConeMirBridgeValidationError::DuplicateExport {
            index,
            declaration: record.declaration,
        });
    }
    if previous.declaration > record.declaration {
        return Err(CrossConeMirBridgeValidationError::NonCanonicalExportOrder { index });
    }
    Ok(())
}

pub(super) fn validate_selected_order(
    selected: &[SelectedDependencyMirCallableV1],
    record: &SelectedDependencyMirCallableV1,
    index: usize,
) -> Result<(), CrossConeMirBridgeValidationError> {
    let Some(previous) = selected.last() else {
        return Ok(());
    };
    if previous.sort_key() == record.sort_key() {
        return Err(CrossConeMirBridgeValidationError::DuplicateSelected {
            index,
            provider: record.provider,
            declaration: record.declaration,
        });
    }
    if previous.sort_key() > record.sort_key() {
        return Err(CrossConeMirBridgeValidationError::NonCanonicalSelectedOrder { index });
    }
    Ok(())
}
