use super::*;
use crate::tests::m23_type_semantics_production::source_binding::nominal_parameters::support::Sources;

pub(in crate::tests::m23_type_semantics_production::source_binding::default_operation_signatures) fn with_core_source(
    text: &str,
    run: impl FnOnce(
        &hir::DependencyHirOutput,
        &hir::BoundNominalParameterProtocolsV1<'_, '_, '_, '_>,
        &hir::ImportedCoreFundamentalTypeProtocol,
    ),
) {
    with_core_sources(text, |output, fixture, sources, core| {
        let foundation = fixture.bind().unwrap();
        sources.with_bound(&foundation, core, |members, constructors| {
            let protocols = members
                .bind_parameter_protocols(constructors, &sources.protocols)
                .unwrap();
            run(output, &protocols, core);
        });
    });
}

pub(in crate::tests::m23_type_semantics_production::source_binding::default_operation_signatures) fn with_core_sources(
    text: &str,
    run: impl FnOnce(
        &hir::DependencyHirOutput,
        &Fixture,
        &Sources,
        &hir::ImportedCoreFundamentalTypeProtocol,
    ),
) {
    use crate::tests::m23_type_semantics_production::source_binding::core_foundation::support::{
        import, lower_extra,
    };
    let output = lower_extra(text);
    let protocols = hir::CompilerProtocolDefinitionsV1::from_export(&output.export).unwrap();
    let output = hir::DependencyHirOutput::try_new(
        output,
        hir::ImportedDependencySelectionPlan::empty(ConeIdentity::CORE).finish(),
        vec![],
    )
    .unwrap();
    let source = hir::CrossConeTypeSemanticsFoundationV1::from_dependency_hir(&output)
        .unwrap()
        .source_transcript()
        .unwrap();
    let foundation = hir::CanonicalHirFoundation::from_type_semantics_output(&output).unwrap();
    let decoded: hir::DecodedHirFoundation =
        decode_canonical(&encode(&foundation).unwrap()).unwrap();
    let mut pending = scoop_identity::PendingIdentityValidation::new();
    pending.register_authority(ConeIdentity::CORE).unwrap();
    decoded.register_identities(&mut pending).unwrap();
    decoded.resolve_identities(&mut pending).unwrap();
    let mut identities = pending.finish().unwrap();
    let foundation = hir::OdrFreeHirFoundation::from_validated(
        decoded
            .validate(&ConeCoordinate::reserved_core(), &mut identities)
            .unwrap(),
    )
    .unwrap_or_else(|error| {
        if let hir::OdrFreeHirFoundationError::CallableApplication(id) = error {
            let key = identities
                .canonical_key::<_, scoop_identity::CallableApplicationKey>(id)
                .unwrap();
            let declaration = match key.origin() {
                scoop_identity::CallableTemplateOrigin::Function(id) => identities
                    .canonical_key::<_, scoop_identity::SourceDeclarationKey>(id)
                    .unwrap(),
                scoop_identity::CallableTemplateOrigin::GenericFunction(id) => identities
                    .canonical_key::<_, scoop_identity::SourceDeclarationKey>(id)
                    .unwrap(),
                scoop_identity::CallableTemplateOrigin::Constructor(id) => identities
                    .canonical_key::<_, scoop_identity::SourceDeclarationKey>(id)
                    .unwrap(),
                other => panic!("unexpected fixture application: {key:?}, {other:?}"),
            };
            panic!("fixture materialized {:?}: {key:?}", declaration.name());
        }
        panic!("fixture must not materialize ODR definitions: {error:?}");
    });
    let decoded: hir::DecodedTypeFoundationSourceAuthorityV1 =
        decode_canonical(&encode(&source).unwrap()).unwrap();
    let source = decoded.resolve(&mut identities).unwrap();
    let mut fixture = Fixture {
        source,
        foundation,
        identities,
    };
    let sources = Sources::from_output(&output, &mut fixture);
    let imported = import(&fixture.foundation, &fixture.identities);
    let core = imported.import_core_inputs(&protocols).unwrap();
    run(
        &output,
        &fixture,
        &sources,
        core.protocols().fundamental_types(),
    );
}
