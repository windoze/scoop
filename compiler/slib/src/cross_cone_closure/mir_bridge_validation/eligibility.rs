//! Maximal provider export derivation from validated HIR and MIR surfaces.

use scoop_hir::{
    CallableImplementationV1, CallableInterfaceRecordV1, CoreHirInterfaceBranchV1,
    CoreHirTypeCapabilityV1, CoreTypeDefinitionV1, PublicDeclarationOwnerV1,
};
use scoop_identity::{
    CallableTemplateOrigin, ConeIdentity, DependencyCallableDeclarationId, ExactCallableSignature,
    PersistentExactTypeId, PersistentTypeId, SignatureTypeKey,
};
use scoop_mir::{
    CrossConeMirBridgeSectionV1, ParamFreeMirCallableExportV1, StrongCallableBridgeSurfaceV1,
};

use super::{CrossConeClosureMirBridgeError, CrossConeMirClosureRelationError};
use crate::MirBridgeValidatedCrossConeHirFrontSections;

pub(super) struct CoreClosedExactLeafClassifier {
    leaves: Vec<(PersistentTypeId, PersistentExactTypeId)>,
}

impl CoreClosedExactLeafClassifier {
    pub(super) fn try_new(
        core: &MirBridgeValidatedCrossConeHirFrontSections<'_>,
    ) -> Result<Self, CrossConeClosureMirBridgeError> {
        let CoreHirInterfaceBranchV1::Core(interface) = core.hir_core_production().core_interface()
        else {
            return Err(CrossConeClosureMirBridgeError::MissingTrustedCoreInterface);
        };
        let target_count = interface.type_targets().targets().len();
        let mut leaves = Vec::new();
        leaves.try_reserve_exact(target_count).map_err(|_| {
            CrossConeClosureMirBridgeError::Allocation {
                requested_slots: target_count,
            }
        })?;
        for target in interface.type_targets().targets() {
            let (
                CoreTypeDefinitionV1::Type(source),
                CoreHirTypeCapabilityV1::ParamFreeStrong(exact),
            ) = (target.definition(), target.capability())
            else {
                continue;
            };
            leaves.push((source, exact));
        }
        leaves.sort_unstable_by_key(|(source, _)| *source);
        leaves.dedup_by_key(|(source, _)| *source);
        Ok(Self { leaves })
    }

    fn classify(&self, signature: &SignatureTypeKey) -> Option<PersistentExactTypeId> {
        let SignatureTypeKey::Nominal(source) = signature else {
            return None;
        };
        self.leaves
            .binary_search_by_key(source, |(candidate, _)| *candidate)
            .ok()
            .map(|index| self.leaves[index].1)
    }
}

pub(super) fn validate_export_surface(
    front: &MirBridgeValidatedCrossConeHirFrontSections<'_>,
    classifier: &CoreClosedExactLeafClassifier,
) -> Result<(), CrossConeMirClosureRelationError> {
    validate_export_relation(
        front.identity(),
        front.hir_interface().callable_interfaces().records(),
        front.mir_core_production().strong_callable_bridges(),
        front.mir_cross_cone_bridge(),
        classifier,
    )
}

fn validate_export_relation(
    artifact: ConeIdentity,
    callable_records: &[CallableInterfaceRecordV1],
    strong_bridges: &StrongCallableBridgeSurfaceV1,
    dependency_bridge: &CrossConeMirBridgeSectionV1,
    classifier: &CoreClosedExactLeafClassifier,
) -> Result<(), CrossConeMirClosureRelationError> {
    if artifact == ConeIdentity::CORE {
        return Ok(());
    }

    let mut expected = Vec::new();
    expected
        .try_reserve_exact(callable_records.len())
        .map_err(|_| CrossConeMirClosureRelationError::Allocation {
            requested_slots: callable_records.len(),
        })?;
    for callable in callable_records {
        let Some(declaration) = eligible_declaration(callable) else {
            continue;
        };
        let Some(signature) = exact_signature(callable, classifier)? else {
            continue;
        };
        let implementation = declaration.implementation().callable_owner();
        let Some(strong) = strong_bridges
            .bridges()
            .iter()
            .find(|bridge| bridge.implementation() == implementation)
        else {
            continue;
        };
        if strong.signature() != &signature {
            return Err(CrossConeMirClosureRelationError::StrongSignatureMismatch { declaration });
        }
        expected.push((declaration, signature));
    }
    expected.sort_unstable_by_key(|(declaration, _)| *declaration);

    let actual = dependency_bridge.exports();
    for (declaration, signature) in &expected {
        let export = find_export(actual, *declaration).ok_or(
            CrossConeMirClosureRelationError::MissingMaximalExport {
                declaration: *declaration,
            },
        )?;
        if export.signature() != signature {
            return Err(CrossConeMirClosureRelationError::ExportSignatureMismatch {
                declaration: *declaration,
            });
        }
    }
    for export in actual {
        if expected
            .binary_search_by_key(&export.declaration(), |(declaration, _)| *declaration)
            .is_err()
        {
            return Err(CrossConeMirClosureRelationError::UnexpectedExport {
                declaration: export.declaration(),
            });
        }
    }
    Ok(())
}

fn eligible_declaration(
    callable: &CallableInterfaceRecordV1,
) -> Option<DependencyCallableDeclarationId> {
    if !matches!(
        callable.owner(),
        PublicDeclarationOwnerV1::TopLevel | PublicDeclarationOwnerV1::Extension
    ) || !callable.type_parameters().is_empty()
        || callable.effects().execution() != scoop_identity::Effect::Ordinary
        || callable.effects().implementation() != CallableImplementationV1::Scoop
    {
        return None;
    }
    match callable.declaration() {
        CallableTemplateOrigin::Function(declaration) => {
            Some(DependencyCallableDeclarationId::Function(declaration))
        }
        CallableTemplateOrigin::Accessor(declaration) => Some(
            DependencyCallableDeclarationId::PropertyAccessor(declaration),
        ),
        CallableTemplateOrigin::GenericFunction(_)
        | CallableTemplateOrigin::Constructor(_)
        | CallableTemplateOrigin::VariantConstructor(_) => None,
    }
}

fn exact_signature(
    callable: &CallableInterfaceRecordV1,
    classifier: &CoreClosedExactLeafClassifier,
) -> Result<Option<ExactCallableSignature>, CrossConeMirClosureRelationError> {
    let receiver = match callable.receiver() {
        Some(receiver) => match classifier.classify(receiver) {
            Some(exact) => Some(exact),
            None => return Ok(None),
        },
        None => None,
    };
    let parameter_count = callable.parameters().parameters().len();
    let mut parameters = Vec::new();
    parameters.try_reserve_exact(parameter_count).map_err(|_| {
        CrossConeMirClosureRelationError::Allocation {
            requested_slots: parameter_count,
        }
    })?;
    for parameter in callable.parameters().parameters() {
        let Some(exact) = classifier.classify(parameter.value_type()) else {
            return Ok(None);
        };
        parameters.push(exact);
    }
    let Some(result) = classifier.classify(callable.result()) else {
        return Ok(None);
    };
    Ok(Some(ExactCallableSignature::new(
        callable.effects().execution(),
        receiver,
        parameters,
        result,
    )))
}

fn find_export(
    exports: &[ParamFreeMirCallableExportV1],
    declaration: DependencyCallableDeclarationId,
) -> Option<&ParamFreeMirCallableExportV1> {
    exports
        .binary_search_by_key(&declaration, ParamFreeMirCallableExportV1::declaration)
        .ok()
        .map(|index| &exports[index])
}

#[cfg(test)]
mod tests;
