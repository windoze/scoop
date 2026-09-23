use super::*;
use crate::cross_cone_hir_authority::{
    CanonicalCrossConeHirSurfaceAuthority, CrossConeHirNominalAuthorityError,
    ValidatedNominalProviderView,
};

#[test]
fn type_access_resolves_the_actual_ordinary_provider_without_name_or_layout_fallback() {
    let dependency = CallableSourceSurface::for_cone(
        SourceInterfaceCase::Complete,
        crate::strong_compile_decode::tests::cone_named("type-access-provider"),
    );
    let dependency_bytes = dependency.artifact();
    let provider = support::front(&dependency_bytes);
    let mut current = default_fixture::fixture(default_fixture::Case::Defined);
    let foreign_type = dependency
        .interface
        .callable_interfaces()
        .get(dependency.owner)
        .unwrap()
        .result()
        .clone();
    let local_type = current.interface.default_templates().records()[0].result();
    assert_ne!(&foreign_type, local_type);
    support::add_unused_local(&mut current, foreign_type);
    let mut interface = current.interface.clone();
    let bytes = cross_cone_artifact_for_with_hir_foundation(
        current.cone.clone(),
        vec![
            open_graph(&dependency_bytes)
                .decode_cross_cone_hir_front_sections()
                .unwrap()
                .dependency_record(),
        ],
        &current.foundation,
        encode(&interface.index_for_wire().unwrap()).unwrap(),
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
    for supplied in [true, false] {
        let dependencies = if supplied {
            vec![ValidatedNominalProviderView {
                identity: dependency.cone.identity(),
                identities: &provider.identities,
                foundation: &provider.foundations.hir,
                core: &provider.hir_core_production,
                interface: &provider.hir_interface,
            }]
        } else {
            vec![]
        };
        let mut authority = CanonicalCrossConeHirSurfaceAuthority::new(
            current.cone.identity(),
            &front.identities,
            &front.foundations.hir,
            &front.hir_interface,
            dependencies,
            front.graph.envelope.meter_mut(),
        );
        if supplied {
            authority.validate_default_provider_contracts().unwrap();
            authority
                .validate_default_type_access(&front.hir_core_production)
                .unwrap();
        } else {
            let Err(Error::Reference { source, .. }) =
                authority.validate_default_type_access(&front.hir_core_production)
            else {
                panic!("a same-shaped local nominal must not replace its foreign provider")
            };
            assert!(matches!(*source, Error::Declaration(error)
                if matches!(*error, CrossConeHirNominalAuthorityError::UnreachableProvider { origin }
                    if origin == dependency.cone.identity())));
        }
    }
}
