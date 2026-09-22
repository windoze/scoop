use super::*;
use scoop_identity::{
    CanonicalIdentifier, CborIdentityRecord, DeclarationScope, DefinitionOwnerChain,
    InitializationUnitKey, PackagePath, PersistentTypeId, SourceDeclarationKey,
    SourceDeclarationSite, SourceNominalKind,
};

#[test]
fn actual_mir_export_assembly_preserves_explicit_initialization_edges_and_checks_local_roots() {
    let (_, source) = fixture("combined");
    with_exports(&source, |input, dependencies, _| {
        let foreign = object(ConeIdentity::CORE, "DependencyRegistry");
        let missing = object(input.mir.module().cone, "UnmaterializedRegistry");
        let units: Vec<mir::InitializationUnitIdentityRecord> = [&foreign, &missing]
            .into_iter()
            .map(|object| {
                CborIdentityRecord::from_key(InitializationUnitKey::Object(object.id())).unwrap()
            })
            .collect();
        let mut foundation = hir::CanonicalHirFoundation::empty();
        foundation.set_types(vec![foreign, missing]).unwrap();
        foundation.set_initialization_units(units.clone()).unwrap();
        let foundation: hir::DecodedHirFoundation = decoded(&foundation);
        let mut pending = PendingIdentityValidation::new();
        pending
            .register_external_graph_authorities(input.identities)
            .unwrap();
        foundation.register_identities(&mut pending).unwrap();
        foundation.resolve_identities(&mut pending).unwrap();
        let graph = pending.finish().unwrap();
        let input = MirTypeBridgeExportInputV1 {
            identities: &graph,
            ..input
        };
        let local = input.mir.materialization().initialization_roots()[0].identity();
        for (local_unit, emitted) in [(local, true), (units[1].id(), false)] {
            let record = mir::SelectedExternalInitializationUseV1::try_new(
                input.mir.module().cone,
                &graph,
                local_unit,
                ConeIdentity::CORE,
                units[0].id(),
                mir::MirExternalInitializationCauseV1::InitializationSupport(units[0].id()),
                &mut meter(),
            )
            .unwrap();
            let uses =
                mir::CanonicalMirExternalInitializationUsesV1::try_new(vec![record], &mut meter())
                    .unwrap();
            let source = scoop_mir_lower::MirTypeBridgeSourceProjectionV1::from_input(
                input,
                MirTypeBridgeDependencyTablesV1 {
                    types: &[dependencies],
                    callables: &[],
                    dispatch: &[],
                },
                uses.clone(),
                &mut meter(),
            );
            let result = lower_type_bridge_exports(
                input,
                MirTypeBridgeDependencyTablesV1 {
                    types: &[dependencies],
                    callables: &[],
                    dispatch: &[],
                },
                uses,
                &mut meter(),
            );
            if emitted {
                let candidate = result.unwrap();
                let source = source.unwrap();
                assert_eq!(candidate.initialization_uses().records(), &[record]);
                candidate
                    .validate_sources(input.mir.module().cone, &graph, &source, &mut meter())
                    .unwrap();
                let stripped = mir::MirTypeBridgeExportConstituentsV1::new(
                    candidate.types().clone(),
                    candidate.callables().clone(),
                    candidate.dispatch().clone(),
                    candidate.objects().clone(),
                    candidate.shapes().clone(),
                    mir::CanonicalMirExternalInitializationUsesV1::try_new(vec![], &mut meter())
                        .unwrap(),
                );
                assert!(matches!(
                    stripped.validate_sources(
                        input.mir.module().cone,
                        &graph,
                        &source,
                        &mut meter()
                    ),
                    Err(mir::MirTypeBridgeSourceJoinError::Record(
                        mir::MirTypeBridgeSourceRecordV1::InitializationUses
                    ))
                ));
            } else {
                assert!(
                    matches!(result, Err(Error::MissingInitializationUnit(actual)) if actual == local_unit)
                );
                assert!(
                    matches!(source, Err(scoop_mir_lower::MirTypeBridgeSourceProjectionError::Production(
                    Error::MissingInitializationUnit(actual)
                )) if actual == local_unit)
                );
            }
        }
    });
}

fn object(
    provider: ConeIdentity,
    name: &str,
) -> CborIdentityRecord<PersistentTypeId, SourceDeclarationKey> {
    CborIdentityRecord::from_key(SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            provider,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(name).unwrap(),
        SourceNominalKind::Object,
        0,
    ))
    .unwrap()
}
