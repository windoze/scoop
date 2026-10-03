use super::*;

fn assert_lowers(name: &str) {
    let source = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/m23-implicit-array-capability")
            .join(name),
    )
    .unwrap();
    constants::with_input(&source, |input| {
        let output = lower_current_cone(scoop_identity::RequestedConeKind::Library, input)
            .unwrap_or_else(|errors| panic!("{name}: {errors:#?}"));
        let export = output.output().export.module();
        assert!(
            export.types.iter().any(|(_, ty)| {
                matches!(ty, scoop_hir::Type::Class(application)
                if matches!(export.class_definition(export.class_applications[*application].template).representation,
                    scoop_hir::ClassRepresentation::Intrinsic(scoop_hir::IntrinsicTypeKind::Array)))
            }),
            "{name} must retain the actual imported array declaration"
        );
        scoop_hir::CanonicalHirFoundation::from_dependency_output(&output).unwrap();
    });
}

#[test]
fn imported_array_parameters_use_the_core_application() {
    for name in ["function.scoop", "class.scoop", "struct.scoop"] {
        assert_lowers(name);
    }
}

#[test]
fn imported_array_parameters_preserve_binders_defaults_and_variants() {
    assert_lowers("combined.scoop");
}

#[test]
fn imported_array_literals_use_the_core_application() {
    assert_lowers("literal.scoop");
}

#[test]
fn imported_array_literals_work_inside_generic_defaults() {
    assert_lowers("default.scoop");
}
