//! Maximal provider export derivation from validated HIR and MIR surfaces.

use scoop_hir::{
    CallableInterfaceRecordV1, CoreClosedCallableClassificationError,
    CoreClosedExactLeafClassifierBuildError, CoreClosedExactLeafClassifierV1,
    CoreHirInterfaceBranchV1,
};
use scoop_identity::{ConeIdentity, DependencyCallableDeclarationId, ExactCallableSignature};
use scoop_mir::{
    CrossConeMirBridgeSectionV1, ParamFreeMirCallableExportV1, StrongCallableBridgeSurfaceV1,
};

use super::{CrossConeClosureMirBridgeError, CrossConeMirClosureRelationError};
use crate::MirBridgeValidatedCrossConeHirFrontSections;

pub(super) fn core_classifier(
    core: &MirBridgeValidatedCrossConeHirFrontSections<'_>,
) -> Result<CoreClosedExactLeafClassifierV1, CrossConeClosureMirBridgeError> {
    let CoreHirInterfaceBranchV1::Core(interface) = core.hir_core_production().core_interface()
    else {
        return Err(CrossConeClosureMirBridgeError::MissingTrustedCoreInterface);
    };
    CoreClosedExactLeafClassifierV1::try_from_core_interface(interface).map_err(|error| match error
    {
        CoreClosedExactLeafClassifierBuildError::Allocation { requested_slots } => {
            CrossConeClosureMirBridgeError::Allocation { requested_slots }
        }
        CoreClosedExactLeafClassifierBuildError::Identity(source) => {
            CrossConeClosureMirBridgeError::CoreExactTypeIdentity(source)
        }
    })
}

trait CoreClosedCallableClassifier {
    fn classify_callable(
        &self,
        callable: &CallableInterfaceRecordV1,
    ) -> Result<Option<ClassifiedCallable>, CoreClosedCallableClassificationError>;
}

impl CoreClosedCallableClassifier for CoreClosedExactLeafClassifierV1 {
    fn classify_callable(
        &self,
        callable: &CallableInterfaceRecordV1,
    ) -> Result<Option<ClassifiedCallable>, CoreClosedCallableClassificationError> {
        Ok(
            CoreClosedExactLeafClassifierV1::classify_callable(self, callable)?.map(|eligible| {
                ClassifiedCallable {
                    declaration: eligible.declaration(),
                    signature: eligible.signature().clone(),
                }
            }),
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ClassifiedCallable {
    declaration: DependencyCallableDeclarationId,
    signature: ExactCallableSignature,
}

pub(super) fn validate_export_surface(
    front: &MirBridgeValidatedCrossConeHirFrontSections<'_>,
    classifier: &CoreClosedExactLeafClassifierV1,
) -> Result<(), CrossConeMirClosureRelationError> {
    validate_export_relation(
        front.identity(),
        front.hir_interface().callable_interfaces().records(),
        front.mir_core_production().strong_callable_bridges(),
        front.mir_cross_cone_bridge(),
        classifier,
    )
}

fn validate_export_relation<C>(
    artifact: ConeIdentity,
    callable_records: &[CallableInterfaceRecordV1],
    strong_bridges: &StrongCallableBridgeSurfaceV1,
    dependency_bridge: &CrossConeMirBridgeSectionV1,
    classifier: &C,
) -> Result<(), CrossConeMirClosureRelationError>
where
    C: CoreClosedCallableClassifier,
{
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
        let Some(eligible) = classifier.classify_callable(callable).map_err(
            |CoreClosedCallableClassificationError::Allocation { requested_slots }| {
                CrossConeMirClosureRelationError::Allocation { requested_slots }
            },
        )?
        else {
            continue;
        };
        let declaration = eligible.declaration;
        let implementation = declaration.implementation().callable_owner();
        let Some(strong) = strong_bridges
            .bridges()
            .iter()
            .find(|bridge| bridge.implementation() == implementation)
        else {
            continue;
        };
        if strong.signature() != &eligible.signature {
            return Err(CrossConeMirClosureRelationError::StrongSignatureMismatch { declaration });
        }
        expected.push((declaration, eligible.signature));
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
