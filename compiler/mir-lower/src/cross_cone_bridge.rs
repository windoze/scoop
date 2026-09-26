//! Producer-side projection of the M23-5 cross-Cone MIR bridge.

use std::fmt;

use scoop_hir::{
    CrossConeHirInterfaceSectionV1, NominalCallableClassificationError,
    NominalExactLeafClassifierV1,
};
use scoop_mir::{
    CrossConeMirBridgeBuildError, CrossConeMirBridgeSectionV1, OdrFreeMirFoundation,
    ParamFreeMirCallableBuildError, ParamFreeMirCallableExportV1, SelectedExternalMirSet,
};

use scoop_identity::{ConeIdentity, DependencyCallableDeclarationId};

/// Projects the exact producer export surface and the already-validated
/// consumer selections into the M23-5 MIR bridge section.
///
/// Export eligibility is derived from the HIR interface and nominal signatures
/// resolved within the actual provider scope. A callable is exported only when the current
/// MIR foundation also contains its strong implementation. The selected side
/// is copied from the complete request-local selection set; this function
/// never scans MIR bodies or symbols to reconstruct dependency use.
pub fn lower_cross_cone_bridge_section(
    artifact: ConeIdentity,
    hir: &CrossConeHirInterfaceSectionV1,
    classifier: &NominalExactLeafClassifierV1,
    foundation: &OdrFreeMirFoundation,
    selected: &SelectedExternalMirSet,
) -> Result<CrossConeMirBridgeSectionV1, CrossConeMirBridgeLoweringError> {
    if selected.consumer() != artifact {
        return Err(CrossConeMirBridgeLoweringError::ForeignSelection {
            expected: artifact,
            actual: selected.consumer(),
        });
    }

    let exports = derive_exports(hir, foundation, classifier)?;
    let selected = clone_selected(selected)?;
    CrossConeMirBridgeSectionV1::try_new(artifact, foundation, exports, selected)
        .map_err(CrossConeMirBridgeLoweringError::Bridge)
}

fn clone_selected(
    selected: &SelectedExternalMirSet,
) -> Result<Vec<scoop_mir::SelectedDependencyMirCallableV1>, CrossConeMirBridgeLoweringError> {
    let mut records = Vec::new();
    records.try_reserve_exact(selected.len()).map_err(|_| {
        CrossConeMirBridgeLoweringError::Allocation {
            requested_slots: selected.len(),
        }
    })?;
    records.extend(selected.dependency_callables().cloned());
    Ok(records)
}

fn derive_exports(
    hir: &CrossConeHirInterfaceSectionV1,
    foundation: &OdrFreeMirFoundation,
    classifier: &NominalExactLeafClassifierV1,
) -> Result<Vec<ParamFreeMirCallableExportV1>, CrossConeMirBridgeLoweringError> {
    let mut exports = Vec::new();
    for callable in hir.callable_interfaces().all_declarations() {
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
        let Some(declaration) = classified.direct_declaration() else {
            continue;
        };
        let implementation = classified.implementation().callable_owner();
        if foundation
            .as_canonical()
            .callable_signature(scoop_mir::CallableSignatureSubject::Strong(implementation))
            .is_none()
        {
            continue;
        }
        exports.push(
            ParamFreeMirCallableExportV1::try_new(
                declaration,
                classified.implementation(),
                classified.signature().clone(),
                match classified.gc_effect() {
                    scoop_identity::GcEffect::Managed => scoop_mir::GcEffect::Managed,
                    scoop_identity::GcEffect::NoGc => scoop_mir::GcEffect::NoGc,
                },
            )
            .map_err(|source| CrossConeMirBridgeLoweringError::Export {
                declaration,
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
        source: NominalCallableClassificationError,
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
            Self::ForeignSelection { .. } | Self::Allocation { .. } => None,
        }
    }
}
