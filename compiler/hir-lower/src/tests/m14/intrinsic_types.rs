use super::*;

#[test]
fn intrinsic_type_contract_is_complete_in_export_and_local_hir() {
    let output = lower_user_output(file(vec![fun("main", vec![])]))
        .expect("the core intrinsic type contract must lower");
    let export = &output.export;
    let core = export.intrinsic_type_core;
    for (id, kind, representation) in [
        (
            core.int,
            hir::IntrinsicTypeKind::Int,
            hir::IntrinsicTypeRepresentation::Int,
        ),
        (
            core.uint,
            hir::IntrinsicTypeKind::UInt,
            hir::IntrinsicTypeRepresentation::UInt,
        ),
        (
            core.boolean,
            hir::IntrinsicTypeKind::Boolean,
            hir::IntrinsicTypeRepresentation::Boolean,
        ),
    ] {
        let hir::StructRepresentation::Intrinsic(declaration) = export.structs[id].representation
        else {
            panic!("fixed intrinsic struct must not masquerade as an empty declaration")
        };
        assert_eq!(declaration.kind, kind);
        assert_eq!(declaration.provider, hir::IntrinsicProviderId::from_raw(0));
        assert_eq!(
            export.struct_applications[export.structs[id].self_application].representation,
            hir::StructApplicationRepresentation::Intrinsic(representation)
        );
    }

    let hir::ClassRepresentation::Intrinsic(string_declaration) =
        export.classes[core.string].representation
    else {
        panic!("String must have an explicit intrinsic representation")
    };
    assert_eq!(string_declaration.kind, hir::IntrinsicTypeKind::String);
    assert!(matches!(
        export.class_applications[export.classes[core.string].self_application].representation,
        hir::ClassApplicationRepresentation::Intrinsic(hir::IntrinsicTypeRepresentation::String)
    ));
    for (id, mutable) in [(core.array, false), (core.mutable_array, true)] {
        let application = &export.class_applications[export.classes[id].self_application];
        let element = match &application.representation {
            hir::ClassApplicationRepresentation::Intrinsic(
                hir::IntrinsicTypeRepresentation::Array { element },
            ) if !mutable => element,
            hir::ClassApplicationRepresentation::Intrinsic(
                hir::IntrinsicTypeRepresentation::MutableArray { element },
            ) if mutable => element,
            _ => panic!("generic intrinsic family must carry its element type"),
        };
        assert!(matches!(export.types[*element], hir::Type::Param(_)));
    }

    let local = &output.local;
    for (id, expected) in [
        (
            local.intrinsic_type_core.int,
            hir::concrete::IntrinsicTypeRepresentation::Int,
        ),
        (
            local.intrinsic_type_core.uint,
            hir::concrete::IntrinsicTypeRepresentation::UInt,
        ),
        (
            local.intrinsic_type_core.boolean,
            hir::concrete::IntrinsicTypeRepresentation::Boolean,
        ),
    ] {
        assert!(matches!(
            &local.structs[id].representation,
            hir::concrete::StructRepresentation::Intrinsic { application, .. }
                if application == &expected
        ));
    }
    assert!(matches!(
        local.classes[local.intrinsic_type_core.string].representation,
        hir::concrete::ClassRepresentation::Intrinsic {
            application: hir::concrete::IntrinsicTypeRepresentation::String,
            ..
        }
    ));
}

#[test]
fn intrinsic_type_shape_is_validated_at_its_source() {
    let mut core = core_file();
    let int = core
        .declarations
        .iter_mut()
        .find_map(|declaration| match declaration {
            Decl::Struct(declaration) if declaration.name.text == "Int" => Some(declaration),
            _ => None,
        })
        .expect("core Int declaration");
    int.fields = ast::StructRepresentationDecl::Declared(Vec::new());
    let errors = lower(&[core, file(vec![fun("main", vec![])])])
        .expect_err("an intrinsic type must omit its compiler representation");
    assert!(errors.iter().any(|error| {
        error.file == 0
            && error.message
                == "intrinsic type `core_int` must omit its fields or primary constructor"
    }));
}

#[test]
fn allowlisted_type_provider_preserves_provenance_without_relaxing_shape() {
    let mut core = core_file();
    let int_index = core
        .declarations
        .iter()
        .position(
            |declaration| matches!(declaration, Decl::Struct(declaration) if declaration.name.text == "Int"),
        )
        .expect("core Int declaration");
    let int = core.declarations.remove(int_index);
    let user = file(vec![int, fun("main", vec![])]);
    let core_provider = hir::IntrinsicProviderId::from_raw(3);
    let test_provider = hir::IntrinsicProviderId::from_raw(7);
    let unit = CompilationUnit {
        core: vec![ProviderSource {
            source: &core,
            provider: core_provider,
            name: "core.scoop",
            source_text: "",
        }],
        user: ProviderSource {
            source: &user,
            provider: test_provider,
            name: "user.scoop",
            source_text: "",
        },
    };
    let output = lower_compilation_unit(
        &unit,
        IntrinsicDeclarationPolicy::AllowListedForTesting {
            providers: std::collections::HashSet::from([test_provider]),
        },
    )
    .expect("the internal allowlist grants only declaration authority");
    let hir::StructRepresentation::Intrinsic(declaration) =
        output.export.structs[output.export.intrinsic_type_core.int].representation
    else {
        panic!("the allowlisted declaration remains typed")
    };
    assert_eq!(declaration.provider, test_provider);
}

#[test]
fn intrinsic_types_do_not_acquire_zero_argument_source_constructors() {
    let errors = lower_user_output(file(vec![fun(
        "main",
        vec![stmt(call("Int", vec![])), stmt(call("String", vec![]))],
    )]))
    .expect_err("hidden intrinsic construction is not a source constructor");
    assert!(
        errors
            .iter()
            .any(|error| error.message == "intrinsic struct `Int` has no source constructor")
    );
    assert!(
        errors
            .iter()
            .any(|error| error.message == "intrinsic class `String` has no source constructor")
    );
}
