use super::*;

const ARRAY_PROVIDER: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-generic-arrays/provider.scoop"
));

#[test]
fn imported_arrays_and_varargs_materialize_complete_core_applications() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-generic-arrays/basic.scoop"
    ));
    with_provider_consumer(ARRAY_PROVIDER, source, |output, _, _, _, _| {
        let local = output.output().local.module();
        assert!(local.classes.iter().any(|(_, class)| matches!(
            class.representation,
            hir::concrete::ClassRepresentation::Intrinsic {
                application: hir::concrete::IntrinsicTypeRepresentation::Array { .. },
                ..
            }
        )));
        assert!(local.classes.iter().any(|(_, class)| matches!(
            class.representation,
            hir::concrete::ClassRepresentation::Intrinsic {
                application: hir::concrete::IntrinsicTypeRepresentation::MutableArray { .. },
                ..
            }
        )));
        let foundation = hir::CanonicalHirFoundation::from_dependency_output(&output).unwrap();
        scoop_wire::encode(&foundation).unwrap();
    })
    .unwrap();
}
