use super::*;
use crate::cross_cone_hir_authority::{
    CrossConeHirNominalAuthorityError, ValidatedNominalProviderView,
};

#[test]
fn value_access_uses_the_actual_provider_even_when_local_name_and_shape_match() {
    let dependency = crate::strong_compile_decode::tests::cone_named("value-access-provider");
    let (provider_bytes, foreign) = support::surface(dependency.clone());
    let provider = declaration_front(&provider_bytes);
    let (current_bytes, local) = support::surface(cone());
    assert_ne!(local.global, foreign.global);
    let mut current = declaration_front(&current_bytes);
    support::insert_reference(&mut current, Target::Global(foreign.global));
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
    let front = decoded
        .validate_foundation_structure(identities)
        .unwrap()
        .resolve_hir_interface()
        .unwrap()
        .validate_hir_production()
        .unwrap();
    for supplied in [true, false] {
        let dependencies = if supplied {
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
        )
        .validate_default_value_access();
        if supplied {
            result.unwrap();
        } else {
            let Err(Error::Reference { source, .. }) = result else {
                panic!("a same-shaped local property must not replace its foreign provider")
            };
            assert!(matches!(*source, Error::Declaration(error)
                if matches!(*error, CrossConeHirNominalAuthorityError::UnreachableProvider { origin }
                    if origin == dependency.identity())));
        }
    }
}
