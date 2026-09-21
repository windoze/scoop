use super::*;
use scoop_wire::{ResourceKind, WireErrorKind};
#[test]
fn source_reference_resolution_shares_one_monotonic_budget() {
    with_hir_source(SOURCE, |output, _| {
        let body = Body::from_dependency_hir(
            output,
            function(output.output().export.module(), "combined"),
            0,
            &mut meter(),
        )
        .unwrap();
        let decoded: Decoded =
            decode_canonical(&encode(body.references()).unwrap(), DecodeLimits::default()).unwrap();
        let mut measured = meter();
        decoded
            .clone()
            .resolve(&mut identity_closure(output), &mut measured)
            .unwrap();
        let work = measured.usage().validation_work_units;
        let mut shared = BudgetMeter::new(DecodeLimits {
            validation_work_units: work * 2 - 1,
            ..DecodeLimits::default()
        });
        decoded
            .clone()
            .resolve(&mut identity_closure(output), &mut shared)
            .unwrap();
        let error = decoded
            .resolve(&mut identity_closure(output), &mut shared)
            .unwrap_err();
        assert!(matches!(
            error.resource_error().unwrap().kind(),
            WireErrorKind::LimitExceeded {
                resource: ResourceKind::ValidationWorkUnits,
                ..
            }
        ));
    });
}
#[test]
fn source_reference_resolution_charges_every_resource_dimension() {
    with_hir_source(SOURCE, |output, _| {
        let body = Body::from_dependency_hir(
            output,
            function(output.output().export.module(), "combined"),
            0,
            &mut meter(),
        )
        .unwrap();
        let bytes = encode(body.references()).unwrap();
        for (limits, expected) in [
            (
                DecodeLimits {
                    decoded_nodes: 0,
                    ..DecodeLimits::default()
                },
                ResourceKind::DecodedNodes,
            ),
            (
                DecodeLimits {
                    owned_bytes: 0,
                    ..DecodeLimits::default()
                },
                ResourceKind::OwnedBytes,
            ),
            (
                DecodeLimits {
                    logical_heap_bytes: 0,
                    ..DecodeLimits::default()
                },
                ResourceKind::LogicalHeapBytes,
            ),
            (
                DecodeLimits {
                    semantic_recursion: 0,
                    ..DecodeLimits::default()
                },
                ResourceKind::SemanticRecursion,
            ),
            (
                DecodeLimits {
                    semantic_table_entries: 0,
                    ..DecodeLimits::default()
                },
                ResourceKind::SemanticTableEntries,
            ),
            (
                DecodeLimits {
                    validation_work_units: 0,
                    ..DecodeLimits::default()
                },
                ResourceKind::ValidationWorkUnits,
            ),
            (
                DecodeLimits {
                    semantic_leaf_bytes: 0,
                    ..DecodeLimits::default()
                },
                ResourceKind::SemanticLeafBytes,
            ),
        ] {
            let decoded: Decoded = decode_canonical(&bytes, DecodeLimits::default()).unwrap();
            let error = decoded
                .resolve(&mut identity_closure(output), &mut BudgetMeter::new(limits))
                .unwrap_err();
            assert!(
                matches!(error.resource_error().unwrap().kind(), WireErrorKind::LimitExceeded { resource, .. } if *resource == expected),
                "{error:?}"
            );
        }
    });
}

#[test]
fn source_producer_checks_occurrence_allocation_in_the_body_budget() {
    with_hir_source(SOURCE, |output, _| {
        let mut export = output.output().export.module().clone();
        let owner = function(&export, "callable");
        let limits = DecodeLimits {
            semantic_table_entries: 64,
            ..DecodeLimits::default()
        };
        Body::from_export_hir(&export, owner, 0, &mut BudgetMeter::new(limits)).unwrap();
        let template = export
            .export_default_exprs
            .iter_mut()
            .find(|(_, expr)| {
                expr.references
                    .callables
                    .iter()
                    .any(|r| r.witness.owner == owner)
            })
            .unwrap()
            .1;
        template.references.callables = vec![template.references.callables[0].clone(); 65];
        let error =
            Body::from_export_hir(&export, owner, 0, &mut BudgetMeter::new(limits)).unwrap_err();
        assert!(
            matches!(error, hir::DefaultSourceBodyProductionError::References(_)),
            "{error:?}"
        );
        assert!(matches!(
            error.resource_error().unwrap().kind(),
            WireErrorKind::LimitExceeded {
                resource: ResourceKind::SemanticTableEntries,
                ..
            }
        ));
    });
}

#[test]
fn source_reference_vector_allocation_is_charged_before_any_target_lookup() {
    with_hir_source(SOURCE, |output, _| {
        let body = Body::from_dependency_hir(
            output,
            function(output.output().export.module(), "callable"),
            0,
            &mut meter(),
        )
        .unwrap();
        let input: Decoded =
            decode_canonical(&encode(body.references()).unwrap(), DecodeLimits::default()).unwrap();
        let mut empty = scoop_identity::PendingIdentityValidation::new()
            .finish()
            .unwrap();
        let error = input
            .resolve(
                &mut empty,
                &mut BudgetMeter::new(DecodeLimits {
                    owned_bytes: 0,
                    ..DecodeLimits::default()
                }),
            )
            .unwrap_err();
        assert!(matches!(
            error,
            hir::DefaultSourceReferencesResolutionError::Resource(_)
        ));
        assert!(matches!(
            error.resource_error().unwrap().kind(),
            WireErrorKind::LimitExceeded {
                resource: ResourceKind::OwnedBytes,
                ..
            }
        ));
    });
}
