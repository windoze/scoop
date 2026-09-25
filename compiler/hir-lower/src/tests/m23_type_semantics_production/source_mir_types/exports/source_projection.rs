use super::*;
use mir::MirTypeBridgeSourceSemanticAuthorityV1;
use scoop_mir_lower::{MirTypeBridgeSourceProjectionError, MirTypeBridgeSourceProjectionV1};

mod records;
mod rejections;

fn project(
    input: MirTypeBridgeExportInputV1<'_>,
    dependencies: &mir::CanonicalParamFreeMirTypeExportsV1,
) -> Result<MirTypeBridgeSourceProjectionV1, MirTypeBridgeSourceProjectionError> {
    MirTypeBridgeSourceProjectionV1::from_input(
        input,
        MirTypeBridgeDependencyTablesV1 {
            types: &[dependencies],
            callables: &[],
            dispatch: &[],
        },
    )
}

#[test]
fn actual_mir_source_projection_replays_independently_produced_export_candidates() {
    for name in ["standalone", "combined"] {
        let (_, source) = fixture(name);
        with_exports(&source, |input, dependencies, candidate| {
            let source = project(input, dependencies).unwrap();
            let checked = candidate
                .validate_sources(input.mir.module().cone, input.identities, &source)
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
fn actual_mir_source_projection_rejects_missing_exports() {
    let (_, source) = fixture("combined");
    with_exports(&source, |input, dependencies, candidate| {
        let source = project(input, dependencies).unwrap();
        rejections::inventories(input, candidate, &source);
    });
}
