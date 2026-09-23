use super::*;
use crate::cross_cone_hir_authority::{
    CrossConeHirNominalAuthorityError, ValidatedNominalProviderView,
};

#[test]
fn callable_access_uses_actual_foreign_declarations_despite_matching_local_names() {
    let dependency = crate::strong_compile_decode::tests::cone_named("callable-access-provider");
    let provider_bytes = support::surface(dependency.clone());
    let mut provider = declaration_front(&provider_bytes);
    let (foreign, _, foreign_type) = support::context(&provider);
    let current_bytes = support::surface(cone());
    let mut current = declaration_front(&current_bytes);
    let (local, origin, ty) = support::context(&current);
    assert_ne!(foreign, local);
    let CallableTemplateOrigin::Function(function) = foreign else {
        panic!("function")
    };
    let callee = DefaultCallableRefV1::try_new(
        DefaultCallableDeclarationV1::Function(function),
        OptionalSignatureType::Present(Box::new(foreign_type)),
        vec![],
    )
    .unwrap();
    let value = support::value(&ty, &origin);
    support::install(
        &mut current,
        local,
        origin,
        ty,
        DefaultExpressionKindV1::MethodCall {
            receiver: Box::new(value.clone()),
            callee: DefaultMethodCalleeV1::Callable(callee.clone()),
            arguments: vec![value],
        },
        ExportDefaultCallableTargetV1::Callable(callee),
    );
    let dependency_record = open_graph(&provider_bytes)
        .decode_cross_cone_hir_front_sections()
        .unwrap()
        .dependency_record();
    let bytes = cross_cone_artifact_for_with_hir_foundation(
        cone(),
        vec![dependency_record],
        current.foundations.hir.as_canonical(),
        encode(&current.hir_interface.index_for_wire().unwrap()).unwrap(),
    );
    let mut decoded = open_graph(&bytes)
        .decode_cross_cone_hir_front_sections()
        .unwrap();
    let identities = decoded
        .validate_foundation_identities([&provider.identities])
        .unwrap();
    let mut front = decoded
        .validate_foundation_structure(identities)
        .unwrap()
        .resolve_hir_interface()
        .unwrap()
        .validate_hir_production()
        .unwrap();
    for mode in 0..3 {
        if mode == 2 {
            restrict(
                &mut provider,
                DefinitionOriginSubject::Function(function),
                DeclaredVisibilityV1::Private,
            );
        }
        let dependencies = if mode != 1 {
            vec![ValidatedNominalProviderView {
                identity: dependency.identity(),
                identities: &provider.identities,
                foundation: &provider.foundations.hir,
                core: &provider.hir_core_production,
                interface: &provider.hir_interface,
            }]
        } else {
            vec![]
        };
        let result = CanonicalCrossConeHirSurfaceAuthority::new(
            front.graph.identity(),
            &front.identities,
            &front.foundations.hir,
            &front.hir_interface,
            dependencies,
            front.graph.envelope.meter_mut(),
        )
        .validate_default_callable_access(&front.hir_core_production);
        if mode == 0 {
            result.unwrap();
        } else {
            let Err(Error::Template { source, .. }) = result else {
                panic!("foreign callable access")
            };
            let Error::Occurrence { source, .. } = *source else {
                panic!("actual callable occurrence")
            };
            if mode == 1 {
                assert!(matches!(*source, Error::Declaration(error)
                    if matches!(*error, CrossConeHirNominalAuthorityError::UnreachableProvider { origin }
                        if origin == dependency.identity())));
            } else {
                assert!(matches!(*source, Error::WitnessDomain));
            }
        }
    }
}
