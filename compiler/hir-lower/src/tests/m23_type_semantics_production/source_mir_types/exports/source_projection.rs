use super::*;
use mir::MirTypeBridgeSourceSemanticAuthorityV1;
use scoop_mir_lower::{MirTypeBridgeSourceProjectionError, MirTypeBridgeSourceProjectionV1};

mod records;
mod rejections;

fn project(
    input: MirTypeBridgeExportInputV1<'_>,
    dependencies: &mir::CanonicalParamFreeMirTypeExportsV1,
    meter: &mut BudgetMeter,
) -> Result<MirTypeBridgeSourceProjectionV1, MirTypeBridgeSourceProjectionError> {
    MirTypeBridgeSourceProjectionV1::from_input(
        input,
        MirTypeBridgeDependencyTablesV1 {
            types: &[dependencies],
            callables: &[],
            dispatch: &[],
        },
        meter,
    )
}

#[test]
fn actual_mir_source_projection_replays_independently_produced_export_candidates() {
    for name in ["standalone", "combined"] {
        let (_, source) = fixture(name);
        with_exports(&source, |input, dependencies, candidate| {
            let mut meter = meter();
            let source = project(input, dependencies, &mut meter).unwrap();
            let checked = candidate
                .validate_sources(
                    input.mir.module().cone,
                    input.identities,
                    &source,
                    &mut meter,
                )
                .unwrap();
            assert_eq!(checked.provider(), input.mir.module().cone);
            assert!(std::ptr::eq(checked.exports(), candidate));
            let mut roots = input
                .hir
                .output()
                .local
                .materialization()
                .roots()
                .iter()
                .map(|root| root.source())
                .collect::<Vec<_>>();
            roots.sort_unstable();
            assert_eq!(source.required_source_roots().unwrap(), roots);
            for record in candidate.types().records() {
                assert_eq!(source.type_source(record.exact()).unwrap(), record);
            }
            for record in candidate.callables().entries() {
                assert_eq!(
                    source.callable_source(record.implementation()).unwrap(),
                    record
                );
            }
            for record in candidate.dispatch().records() {
                assert_eq!(source.dispatch_source(record.owner()).unwrap(), record);
            }
            for record in candidate.objects().records() {
                assert_eq!(source.object_source(record.value()).unwrap(), record);
            }
            assert_eq!(
                source.committed_initialization_uses().unwrap(),
                candidate.initialization_uses()
            );
            assert!(matches!(
                source.type_source(dependencies.records()[0].exact()),
                Err(MirTypeBridgeSourceProjectionError::MissingSource(
                    mir::MirTypeBridgeSourceRecordV1::Type(_)
                ))
            ));
        });
    }
}

#[test]
fn actual_mir_source_projection_rejects_candidate_omissions_and_shared_budget_exhaustion() {
    let (_, source) = fixture("combined");
    with_exports(&source, |input, dependencies, candidate| {
        let source = project(input, dependencies, &mut meter()).unwrap();
        rejections::inventories(input, candidate, &source);
        let mut measured = meter();
        let source = project(input, dependencies, &mut measured).unwrap();
        let limits = DecodeLimits {
            validation_work_units: measured.usage().validation_work_units,
            ..DecodeLimits::default()
        };
        let mut shared = BudgetMeter::new(limits);
        project(input, dependencies, &mut shared).unwrap();
        assert!(
            candidate
                .validate_sources(
                    input.mir.module().cone,
                    input.identities,
                    &source,
                    &mut shared
                )
                .is_err()
        );
        for limits in [
            DecodeLimits {
                owned_bytes: 0,
                ..DecodeLimits::default()
            },
            DecodeLimits {
                semantic_table_entries: 0,
                ..DecodeLimits::default()
            },
        ] {
            assert!(project(input, dependencies, &mut BudgetMeter::new(limits)).is_err());
        }
    });
}
