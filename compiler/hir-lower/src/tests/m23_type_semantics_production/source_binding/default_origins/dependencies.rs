use super::*;
use scoop_identity::ConeIdentity;

fn empty_source(provider: ConeIdentity) -> hir::TypeFoundationSourceAuthorityV1 {
    hir::TypeFoundationSourceAuthorityV1::try_new(hir::TypeFoundationSourceEntriesV1 {
        provider,
        exact_keys: hir::CanonicalPersistentIdsV1::try_new(vec![]).unwrap(),
        sources: Default::default(),
        representations: Default::default(),
        generated_nominals: hir::CanonicalPersistentIdsV1::try_new(vec![]).unwrap(),
        accessor_keys: hir::CanonicalPersistentIdsV1::try_new(vec![]).unwrap(),
        definition_sources: Default::default(),
        source_roots: Default::default(),
        local_exact_facts: hir::CanonicalPersistentIdsV1::try_new(vec![]).unwrap(),
        dependency_facts: Default::default(),
        local_inheritance_edges: Default::default(),
        fact_shapes: hir::CanonicalExactTypeFactShapesV1::try_new(vec![]).unwrap(),
        representation_owners: hir::CanonicalPersistentIdsV1::try_new(vec![]).unwrap(),
    })
    .unwrap()
}

#[test]
fn foreign_body_locations_route_to_their_explicit_artifact_provider() {
    super::super::super::source_dispatch::with_hir_source(SOURCE, |output, core| {
        let mut fixture = Fixture::from_output(output);
        let sources = Sources::from_output(output, &mut fixture);
        let table = templates(output);
        let t = &table.records()[0];
        let hir::CoreProtocolCallableDefinitionV1::Function(function) = core
            .interface
            .compiler_protocols()
            .initialization_cycle_thrower()
            .definition()
        else {
            panic!("core cycle thrower source function")
        };
        let foreign = hir::ExportDefinitionSourceV1::new(
            core.source_foundation
                .definition_origin(scoop_identity::DefinitionOriginSubject::Function(function))
                .unwrap()
                .origin()
                .clone(),
        );
        assert_eq!(foreign.origin().source().cone(), ConeIdentity::CORE);
        // This pass proves only artifact locations. The later body/source
        // relation pass must still decide whether this origin is appropriate.
        let value = hir::DefaultExpressionV1::try_new(
            t.body().value().kind().clone(),
            t.body().value().result_type().clone(),
            foreign,
        )
        .unwrap();
        let body =
            hir::ExportDefaultBodyV1::try_new(t.body().statements().to_vec(), value).unwrap();
        let routed = replace(
            &table,
            rebuild(t, t.definition_root(), body, t.definition_origin().clone()),
        );
        let source = empty_source(ConeIdentity::CORE);
        let dependency = source
            .bind_to_foundation(&core.source_foundation, &fixture.identities)
            .unwrap();
        let foundation = fixture.bind().unwrap();
        let inputs = core.foundation.import_core_inputs(&core.interface).unwrap();
        sources.with_bound(
            &foundation,
            inputs.protocols().fundamental_types(),
            |members, constructors| {
                let parameters = members
                    .bind_parameter_protocols(constructors, &sources.protocols)
                    .unwrap();
                assert!(matches!(
                    parameters.bind_default_origins(&routed, &[]),
                    Err(Error::MissingProvider(ConeIdentity::CORE))
                ));
                parameters
                    .bind_default_origins(&routed, &[&dependency])
                    .unwrap();
                assert!(matches!(
                    parameters.bind_default_origins(&routed, &[&dependency, &dependency]),
                    Err(Error::DependencyOrder(ConeIdentity::CORE))
                ));
                assert!(matches!(
                    parameters.bind_default_origins(&routed, &[&foundation]),
                    Err(Error::DependencyOrder(_))
                ));
                let other_graph = identity_closure(output);
                let wrong_graph = source
                    .bind_to_foundation(&core.source_foundation, &other_graph)
                    .unwrap();
                assert!(matches!(
                    parameters.bind_default_origins(&routed, &[&wrong_graph]),
                    Err(Error::IdentityGraph(ConeIdentity::CORE))
                ));
            },
        );
    });
}
