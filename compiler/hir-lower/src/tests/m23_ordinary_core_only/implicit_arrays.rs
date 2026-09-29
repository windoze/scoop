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
                matches!(ty, scoop_hir::Type::ImportedClass(class)
                if matches!(class.declaration.interface.source_shape(),
                    scoop_hir::NominalSourceShapeV1::Intrinsic(representation)
                        if representation.family() == scoop_hir::IntrinsicTypeKind::Array))
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
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-implicit-array-capability/empty.scoop"
    ));
    constants::with_input(source, |input| {
        let errors = lower_current_cone(scoop_identity::RequestedConeKind::Library, input)
            .err()
            .unwrap();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].file, 0);
        assert_eq!(
            errors[0].message,
            "cannot infer the element type of an empty array literal"
        );
        let span = errors[0].span.unwrap();
        assert_eq!(&source[span.start as usize..span.end as usize], "[]");
    });
}

#[test]
fn imported_array_literals_work_inside_generic_defaults() {
    assert_lowers("default.scoop");
}
