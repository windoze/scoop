use super::*;

#[test]
fn producer_rejects_unprojected_dispatch_instead_of_emitting_partial_tables() {
    let core = trusted_core();
    let mut source = file(vec![interface_decl(
        "WithSlot",
        vec![method("call", Vec::new(), None, Vec::new())],
    )]);
    make_core_public(&mut source);
    let ordinary = parsed_ordinary(source);
    let core_inputs = core
        .foundation
        .import_core_inputs(&core.interface, &[])
        .unwrap();
    let world = core.world(ordinary.cone());
    let input = OrdinarySources::try_new(&ordinary, core_inputs, &world).unwrap();
    let output = lower_ordinary(scoop_identity::RequestedConeKind::Library, &input).unwrap();
    let public = public_interface(&output);

    assert!(matches!(
        produce_cross_cone_type_semantics(&output, &public),
        Err(hir::CrossConeTypeSemanticsProductionError::UnsupportedDispatch(_))
    ));
}

#[test]
fn producer_keeps_final_direct_methods_in_the_m23_5_partition() {
    let core = trusted_core();
    let mut source = file(vec![struct_decl_methods(
        "WithDirectMethod",
        Vec::new(),
        vec![method("call", Vec::new(), None, Vec::new())],
    )]);
    make_core_public(&mut source);
    let ordinary = parsed_ordinary(source);
    let core_inputs = core
        .foundation
        .import_core_inputs(&core.interface, &[])
        .unwrap();
    let world = core.world(ordinary.cone());
    let input = OrdinarySources::try_new(&ordinary, core_inputs, &world).unwrap();
    let output = lower_ordinary(scoop_identity::RequestedConeKind::Library, &input).unwrap();
    let public = public_interface(&output);
    let production = produce_cross_cone_type_semantics(&output, &public).unwrap();

    assert_eq!(production.section().inheritance().records().len(), 1);
    assert!(
        production.section().inheritance().records()[0]
            .slots()
            .records()
            .is_empty()
    );
}

#[test]
fn producer_rejects_generic_materialization_without_odr_authority() {
    let core = trusted_core();
    let mut source = file(vec![
        generic_struct_decl("Box", vec!["T"], vec![("value", ty_named("T"))]),
        struct_decl(
            "UsesBox",
            vec![("value", ty_generic("Box", vec![ty_named("Int")]))],
        ),
    ]);
    make_core_public(&mut source);
    let ordinary = parsed_ordinary(source);
    let core_inputs = core
        .foundation
        .import_core_inputs(&core.interface, &[])
        .unwrap();
    let world = core.world(ordinary.cone());
    let input = OrdinarySources::try_new(&ordinary, core_inputs, &world).unwrap();
    let output = lower_ordinary(scoop_identity::RequestedConeKind::Library, &input).unwrap();
    let public = public_interface(&output);

    assert!(matches!(
        produce_cross_cone_type_semantics(&output, &public),
        Err(hir::CrossConeTypeSemanticsProductionError::GenericOdrRequired(_))
    ));
}

#[test]
fn producer_rejects_generic_class_backing_field_without_odr_authority() {
    let core = trusted_core();
    let mut source = file(vec![
        generic_struct_decl("Box", vec!["T"], vec![("value", ty_named("T"))]),
        class_decl(
            ast::ClassModifier::Final,
            "Holder",
            vec![(false, "value", ty_generic("Box", vec![ty_named("Int")]))],
            None,
            Vec::new(),
            Vec::new(),
        ),
    ]);
    make_core_public(&mut source);
    let ordinary = parsed_ordinary(source);
    let core_inputs = core
        .foundation
        .import_core_inputs(&core.interface, &[])
        .unwrap();
    let world = core.world(ordinary.cone());
    let input = OrdinarySources::try_new(&ordinary, core_inputs, &world).unwrap();
    let output = lower_ordinary(scoop_identity::RequestedConeKind::Library, &input).unwrap();
    let public = public_interface(&output);

    assert!(matches!(
        produce_cross_cone_type_semantics(&output, &public),
        Err(hir::CrossConeTypeSemanticsProductionError::GenericOdrRequired(_))
    ));
}

#[test]
fn producer_rejects_generic_constructor_parameter_without_odr_authority() {
    let core = trusted_core();
    let mut holder = class_decl(
        ast::ClassModifier::Final,
        "Holder",
        vec![(false, "value", ty_generic("Box", vec![ty_named("Int")]))],
        None,
        Vec::new(),
        Vec::new(),
    );
    let Decl::Class(declaration) = &mut holder else {
        unreachable!()
    };
    let ast::ClassConstructorDecl::Declared(constructor) = &mut declaration.constructor else {
        unreachable!()
    };
    constructor[0].property = ast::PrimaryParameterProperty::Plain;
    constructor[0].member_visibility = None;
    let mut source = file(vec![
        generic_struct_decl("Box", vec!["T"], vec![("value", ty_named("T"))]),
        holder,
    ]);
    make_core_public(&mut source);
    let ordinary = parsed_ordinary(source);
    let core_inputs = core
        .foundation
        .import_core_inputs(&core.interface, &[])
        .unwrap();
    let world = core.world(ordinary.cone());
    let input = OrdinarySources::try_new(&ordinary, core_inputs, &world).unwrap();
    let output = lower_ordinary(scoop_identity::RequestedConeKind::Library, &input).unwrap();
    let public = public_interface(&output);

    assert!(matches!(
        produce_cross_cone_type_semantics(&output, &public),
        Err(hir::CrossConeTypeSemanticsProductionError::GenericOdrRequired(_))
    ));
}

#[test]
fn generic_source_only_root_has_independent_definition_origin_authority() {
    let core = trusted_core();
    let mut source = file(vec![generic_struct_decl("Box", vec!["T"], Vec::new())]);
    make_core_public(&mut source);
    let ordinary = parsed_ordinary(source);
    let core_inputs = core
        .foundation
        .import_core_inputs(&core.interface, &[])
        .unwrap();
    let world = core.world(ordinary.cone());
    let input = OrdinarySources::try_new(&ordinary, core_inputs, &world).unwrap();
    let output = lower_ordinary(scoop_identity::RequestedConeKind::Library, &input).unwrap();
    let public = public_interface(&output);
    let production = produce_cross_cone_type_semantics(&output, &public).unwrap();

    assert!(production.section().definition_sources().is_empty());
    assert!(
        production
            .section()
            .representation_support()
            .records()
            .is_empty()
    );
    let mut meter = BudgetMeter::new(DecodeLimits::default());
    hir::CheckedNominalInheritanceGraphV1::validate_with_source_roots(
        production.local_inheritance_edges().iter(),
        production.source_roots().iter().copied(),
        production.foundation(),
        &mut meter,
    )
    .unwrap();
}

#[test]
fn empty_slot_schemas_cover_transitive_interfaces() {
    let core = trusted_core();
    let mut source = file(vec![
        interface_decl("A", Vec::new()),
        interface_with_parent("B", "A"),
        struct_decl_full("Value", Vec::new(), vec!["B"], Vec::new()),
    ]);
    make_core_public(&mut source);
    let ordinary = parsed_ordinary(source);
    let core_inputs = core
        .foundation
        .import_core_inputs(&core.interface, &[])
        .unwrap();
    let world = core.world(ordinary.cone());
    let input = OrdinarySources::try_new(&ordinary, core_inputs, &world).unwrap();
    let output = lower_ordinary(scoop_identity::RequestedConeKind::Library, &input).unwrap();
    let public = public_interface(&output);
    let production = produce_cross_cone_type_semantics(&output, &public).unwrap();
    let export = output.output().export.module();
    let value = &export.structs[export.public_surface.structs[0]];
    let exact = export.type_identities
        [export.struct_applications[value.self_application].canonical_type]
        .exact()
        .unwrap()
        .id();
    let implementation = production.section().inheritance().get(exact).unwrap();
    assert_eq!(implementation.slot_schemas().records().len(), 2);
    for interface in production
        .section()
        .inheritance()
        .records()
        .iter()
        .filter(|record| record.edges().modality() == hir::NominalInheritanceModalityV1::Interface)
    {
        assert_eq!(interface.slot_schemas().records().len(), 1);
        assert!(
            interface
                .slot_schemas()
                .get(hir::InheritanceSlotSchemaRoleV1::Interface {
                    interface_exact: interface.owner(),
                })
                .is_some()
        );
    }
}

fn interface_with_parent(name: &str, parent: &str) -> Decl {
    Decl::Interface(ast::InterfaceDecl {
        annotations: Vec::new(),
        visibility: ast::VisibilitySyntax::Omitted,
        name: ident(name),
        type_params: Vec::new(),
        supertypes: vec![ast::SupertypeSpec {
            ty: ty_named(parent),
            constructor_arguments: None,
            span: sp(),
        }],
        where_clause: None,
        methods: Vec::new(),
        properties: Vec::new(),
        nested: Vec::new(),
        companion: None,
        span: sp(),
    })
}
