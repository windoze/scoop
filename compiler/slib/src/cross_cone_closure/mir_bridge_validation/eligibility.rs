//! Maximal provider export derivation from validated HIR and MIR surfaces.

use scoop_hir::{CallableInterfaceRecordV1, NominalExactLeafClassifierV1};
use scoop_identity::DependencyCallableDeclarationId;
use scoop_mir::{
    CrossConeMirBridgeSectionV1, ParamFreeMirCallableExportV1, StrongCallableBridgeSurfaceV1,
};
use scoop_wire::WirePath;

use super::CrossConeMirClosureRelationError;

mod scope;
pub(super) use scope::validate_export_surfaces;

fn validate_export_relation(
    callable_records: &[CallableInterfaceRecordV1],
    strong_bridges: &StrongCallableBridgeSurfaceV1,
    dependency_bridge: &CrossConeMirBridgeSectionV1,
    classifier: &NominalExactLeafClassifierV1,
) -> Result<(), CrossConeMirClosureRelationError> {
    let path = WirePath::root();
    let mut expected = Vec::new();
    scoop_wire::allocation::try_reserve(&mut expected, callable_records.len(), &path)
        .map_err(CrossConeMirClosureRelationError::Resource)?;
    for callable in callable_records {
        let Some(eligible) = classifier
            .classify_callable(callable)
            .map_err(CrossConeMirClosureRelationError::NominalClassification)?
        else {
            continue;
        };
        let declaration = eligible.declaration();
        let implementation = declaration.implementation().callable_owner();

        let Some(strong) = strong_bridges
            .bridges()
            .iter()
            .find(|bridge| bridge.implementation() == implementation)
        else {
            continue;
        };
        if strong.signature() != eligible.signature() {
            return Err(CrossConeMirClosureRelationError::StrongSignatureMismatch { declaration });
        }
        expected.push(eligible);
    }
    let actual = dependency_bridge.exports();

    expected.sort_unstable_by_key(scoop_hir::ParamFreeNominalCallableV1::declaration);
    for eligible in &expected {
        let declaration = eligible.declaration();
        let export = find_export(actual, declaration)
            .ok_or(CrossConeMirClosureRelationError::MissingMaximalExport { declaration })?;
        let expected_gc = match eligible.gc_effect() {
            scoop_identity::GcEffect::Managed => scoop_mir::GcEffect::Managed,
            scoop_identity::GcEffect::NoGc => scoop_mir::GcEffect::NoGc,
        };
        if export.signature() != eligible.signature() || export.gc_effect() != expected_gc {
            return Err(CrossConeMirClosureRelationError::ExportSignatureMismatch { declaration });
        }
    }
    for export in actual {
        if expected
            .binary_search_by_key(&export.declaration(), |eligible| eligible.declaration())
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
