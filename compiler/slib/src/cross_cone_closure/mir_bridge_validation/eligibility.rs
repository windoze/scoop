//! Maximal provider export derivation from validated HIR and MIR surfaces.

use scoop_hir::{CallableInterfaceRecordV1, NominalExactLeafClassifierV1};
use scoop_identity::DependencyCallableDeclarationId;
use scoop_mir::{
    CrossConeMirBridgeSectionV1, ParamFreeMirCallableExportV1, StrongCallableBridgeSurfaceV1,
};
use scoop_wire::{BudgetMeter, WirePath};

use super::CrossConeMirClosureRelationError;

mod scope;
pub(super) use scope::validate_export_surfaces;

fn validate_export_relation(
    callable_records: &[CallableInterfaceRecordV1],
    strong_bridges: &StrongCallableBridgeSurfaceV1,
    dependency_bridge: &CrossConeMirBridgeSectionV1,
    classifier: &NominalExactLeafClassifierV1,
    meter: &mut BudgetMeter,
) -> Result<(), CrossConeMirClosureRelationError> {
    let path = WirePath::root();
    let mut expected = Vec::new();
    meter
        .try_reserve_collection_slots(&mut expected, callable_records.len(), &path)
        .map_err(CrossConeMirClosureRelationError::Resource)?;
    for callable in callable_records {
        let Some(eligible) = classifier
            .classify_callable_metered(callable, meter, &path)
            .map_err(CrossConeMirClosureRelationError::NominalClassification)?
        else {
            continue;
        };
        let declaration = eligible.declaration();
        let implementation = declaration.implementation().callable_owner();
        meter
            .charge_work(strong_bridges.bridges().len() as u64, &path)
            .map_err(CrossConeMirClosureRelationError::Resource)?;
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
    let expected_len = expected.len() as u64;
    let actual_len = actual.len() as u64;
    let expected_depth = 1 + u64::from(expected_len.max(1).ilog2());
    let actual_depth = 1 + u64::from(actual_len.max(1).ilog2());
    meter
        .charge_work(
            expected_len
                .saturating_mul(expected_depth + actual_depth)
                .saturating_add(actual_len.saturating_mul(expected_depth)),
            &path,
        )
        .map_err(CrossConeMirClosureRelationError::Resource)?;
    expected.sort_unstable_by_key(scoop_hir::ParamFreeNominalCallableV1::declaration);
    for eligible in &expected {
        let declaration = eligible.declaration();
        let export = find_export(actual, declaration)
            .ok_or(CrossConeMirClosureRelationError::MissingMaximalExport { declaration })?;
        if export.signature() != eligible.signature() {
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
