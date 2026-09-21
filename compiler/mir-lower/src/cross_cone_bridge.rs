//! Producer-side projection of the M23-5 cross-Cone MIR bridge.

use std::fmt;

use scoop_hir::{
    CallableInterfaceRecordV1, CoreClosedCallableClassificationError,
    CoreClosedExactLeafClassifierV1, CrossConeHirInterfaceSectionV1,
};
use scoop_mir::{
    CrossConeMirBridgeBuildError, CrossConeMirBridgeSectionV1, OdrFreeMirFoundation,
    ParamFreeMirCallableBuildError, ParamFreeMirCallableExportV1, SelectedDependencyMirSet,
    StrongCallableBridgeSurfaceV1,
};

use scoop_identity::{
    ConeIdentity, DependencyCallableDeclarationId, ExactCallableSignature,
    StrongCallableDefinitionOwner,
};

/// Projects the exact producer export surface and the already-validated
/// consumer selections into the M23-5 MIR bridge section.
///
/// Export eligibility is derived from the HIR interface and trusted-core
/// exact-leaf classifier. A public callable is exported only when the current
/// MIR foundation also contains its strong implementation. The selected side
/// is copied only from the branded request-local selection set; this function
/// never scans MIR bodies or symbols to reconstruct dependency use.
pub fn lower_cross_cone_bridge_section(
    artifact: ConeIdentity,
    hir: &CrossConeHirInterfaceSectionV1,
    classifier: &CoreClosedExactLeafClassifierV1,
    foundation: &OdrFreeMirFoundation,
    selected: &SelectedDependencyMirSet,
) -> Result<CrossConeMirBridgeSectionV1, CrossConeMirBridgeLoweringError> {
    lower_cross_cone_bridge_with_classifier(
        artifact,
        hir.callable_interfaces().records(),
        classifier,
        foundation,
        selected,
    )
}

fn lower_cross_cone_bridge_with_classifier<C>(
    artifact: ConeIdentity,
    callable_records: &[CallableInterfaceRecordV1],
    classifier: &C,
    foundation: &OdrFreeMirFoundation,
    selected: &SelectedDependencyMirSet,
) -> Result<CrossConeMirBridgeSectionV1, CrossConeMirBridgeLoweringError>
where
    C: CoreClosedCallableClassifier,
{
    if selected.consumer() != artifact {
        return Err(CrossConeMirBridgeLoweringError::ForeignSelection {
            expected: artifact,
            actual: selected.consumer(),
        });
    }

    let strong = StrongCallableBridgeSurfaceV1::from_odr_free_foundation(foundation);
    let exports = derive_exports(callable_records, &strong, classifier)?;
    let selected = clone_selected(selected)?;
    CrossConeMirBridgeSectionV1::try_new(artifact, foundation, exports, selected)
        .map_err(CrossConeMirBridgeLoweringError::Bridge)
}

fn clone_selected(
    selected: &SelectedDependencyMirSet,
) -> Result<Vec<scoop_mir::SelectedDependencyMirCallableV1>, CrossConeMirBridgeLoweringError> {
    let mut records = Vec::new();
    records.try_reserve_exact(selected.len()).map_err(|_| {
        CrossConeMirBridgeLoweringError::Allocation {
            requested_slots: selected.len(),
        }
    })?;
    records.extend_from_slice(selected.callables());
    Ok(records)
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
            CoreClosedExactLeafClassifierV1::classify_callable(self, callable)?.map(|classified| {
                ClassifiedCallable {
                    declaration: classified.declaration(),
                    implementation: classified.implementation(),
                    signature: classified.signature().clone(),
                }
            }),
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ClassifiedCallable {
    declaration: DependencyCallableDeclarationId,
    implementation: StrongCallableDefinitionOwner,
    signature: ExactCallableSignature,
}

fn derive_exports<C>(
    callable_records: &[CallableInterfaceRecordV1],
    strong: &StrongCallableBridgeSurfaceV1,
    classifier: &C,
) -> Result<Vec<ParamFreeMirCallableExportV1>, CrossConeMirBridgeLoweringError>
where
    C: CoreClosedCallableClassifier,
{
    let mut exports = Vec::new();
    exports
        .try_reserve_exact(callable_records.len())
        .map_err(|_| CrossConeMirBridgeLoweringError::Allocation {
            requested_slots: callable_records.len(),
        })?;
    for callable in callable_records {
        let declaration = callable.declaration();
        let Some(classified) = classifier.classify_callable(callable).map_err(|source| {
            CrossConeMirBridgeLoweringError::Classification {
                declaration,
                source,
            }
        })?
        else {
            continue;
        };
        let implementation = classified.implementation.callable_owner();
        let Some(strong) = strong
            .bridges()
            .iter()
            .find(|bridge| bridge.implementation() == implementation)
        else {
            continue;
        };
        if strong.signature() != &classified.signature {
            return Err(CrossConeMirBridgeLoweringError::StrongSignatureMismatch {
                declaration: classified.declaration,
            });
        }
        exports.push(
            ParamFreeMirCallableExportV1::try_new(
                classified.declaration,
                classified.implementation,
                classified.signature,
            )
            .map_err(|source| CrossConeMirBridgeLoweringError::Export {
                declaration: classified.declaration,
                source: Box::new(source),
            })?,
        );
    }
    Ok(exports)
}

#[derive(Debug, Eq, PartialEq)]
pub enum CrossConeMirBridgeLoweringError {
    ForeignSelection {
        expected: ConeIdentity,
        actual: ConeIdentity,
    },
    Allocation {
        requested_slots: usize,
    },
    Classification {
        declaration: scoop_identity::CallableTemplateOrigin,
        source: CoreClosedCallableClassificationError,
    },
    StrongSignatureMismatch {
        declaration: DependencyCallableDeclarationId,
    },
    Export {
        declaration: DependencyCallableDeclarationId,
        source: Box<ParamFreeMirCallableBuildError>,
    },
    Bridge(CrossConeMirBridgeBuildError),
}

impl fmt::Display for CrossConeMirBridgeLoweringError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ForeignSelection { expected, actual } => write!(
                formatter,
                "dependency MIR selection belongs to Cone {actual}, expected {expected}"
            ),
            Self::Allocation { requested_slots } => write!(
                formatter,
                "cannot reserve {requested_slots} cross-Cone MIR bridge records"
            ),
            Self::Classification {
                declaration,
                source,
            } => write!(
                formatter,
                "cannot classify cross-Cone MIR callable {declaration:?}: {source}"
            ),
            Self::StrongSignatureMismatch { declaration } => write!(
                formatter,
                "cross-Cone MIR callable {declaration:?} disagrees with its strong MIR signature"
            ),
            Self::Export {
                declaration,
                source,
            } => write!(
                formatter,
                "cannot build cross-Cone MIR export {declaration:?}: {source}"
            ),
            Self::Bridge(source) => source.fmt(formatter),
        }
    }
}

impl std::error::Error for CrossConeMirBridgeLoweringError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Classification { source, .. } => Some(source),
            Self::Export { source, .. } => Some(source),
            Self::Bridge(source) => Some(source),
            Self::ForeignSelection { .. }
            | Self::Allocation { .. }
            | Self::StrongSignatureMismatch { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests;
