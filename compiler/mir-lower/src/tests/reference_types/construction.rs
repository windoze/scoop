use super::*;

#[test]
fn class_initializers_chain_on_one_exact_allocation() {
    // open class Root(val label: String)
    // open class Base(val name: String) : Root("root")
    // class Point(val x: Int) : Base("point")
    let mut h = Harness::new();
    let (int, string) = (h.int, h.string);
    let root = h.class(
        "Root",
        hir::ClassModifier::Open,
        &[("label", string)],
        None,
        &[],
    );
    let base = h.class(
        "Base",
        hir::ClassModifier::Open,
        &[("name", string)],
        Some((root, vec![str_lit(&h, "root")])),
        &[],
    );
    let point = h.class(
        "Point",
        hir::ClassModifier::Final,
        &[("x", int)],
        Some((base, vec![str_lit(&h, "point")])),
        &[],
    );
    let point_ty = h.class_ty(point);
    let point_constructor = h
        .class_constructor_applications
        .iter()
        .find_map(|(id, app)| {
            (app.constructor
                == scoop_hir::ClassConstructorDefinition::Local(h.classes[point].constructors[0]))
            .then_some(id)
        })
        .expect("point primary constructor application");
    let mut locals = Arena::new();
    let p = locals.alloc(local("p", point_ty));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![val_decl(
                p,
                expr(
                    hir::ExprKind::ClassInit {
                        constructor: point_constructor,
                        args: vec![int_lit(&h, 1)],
                    },
                    point_ty,
                ),
            )],
        },
    );
    let module = lower(&h.finish(main));

    let point_initializer = module
        .functions
        .iter()
        .find_map(|(id, function)| (function.name == "init.Point.$c2").then_some((id, function)))
        .expect("Point initializer");
    let point_source = module
        .meta
        .source_callable_materializations
        .get(point_initializer.0)
        .expect("the class initializer has an exact MIR location");
    assert!(matches!(
        point_source.materialization().template(),
        scoop_identity::CallableTemplateOwner::Constructor(_)
    ));
    let point_signature = point_source.signature_record().signature();
    let receiver_exact = module
        .meta
        .source_exact_types
        .get(&point_initializer.1.params[0].ty)
        .unwrap()
        .identity_record()
        .id();
    let parameter_exact = module
        .meta
        .source_exact_types
        .get(&point_initializer.1.params[1].ty)
        .unwrap()
        .identity_record()
        .id();
    let unit_exact = module
        .meta
        .source_exact_types
        .get(&mir::Type::Unit)
        .unwrap()
        .identity_record()
        .id();
    assert_eq!(
        point_signature.receiver(),
        scoop_identity::OptionalExactOwner::Present(receiver_exact)
    );
    assert_eq!(point_signature.parameters(), &[parameter_exact]);
    assert_eq!(point_signature.result(), unit_exact);
    let receiver = module
        .meta
        .local_values
        .get(point_initializer.0, point_initializer.1.params[0].local)
        .expect("the initializer receiver keeps its LocalConcrete value identity");
    let parameter = module
        .meta
        .local_values
        .get(point_initializer.0, point_initializer.1.params[1].local)
        .expect("the initializer parameter keeps its LocalConcrete value identity");
    assert!(matches!(
        receiver.identity_record().key().selector(),
        scoop_identity::LocalValueSelector::This
    ));
    assert!(matches!(
        parameter.identity_record().key().selector(),
        scoop_identity::LocalValueSelector::Parameter {
            declaration_index: 0
        }
    ));

    // The use site performs the only allocation. Each initializer receives
    // that object, calls the direct base initializer, then writes its own
    // complete-layout field.
    check_mir_snapshot("class_initializers_chain_on_one_exact_allocation", &module);
}

#[test]
fn abstract_classes_keep_an_initializer_for_derived_delegation() {
    let mut h = Harness::new();
    let _base = h.class("Base", hir::ClassModifier::Abstract, &[], None, &[]);
    let main = empty_main(&mut h);
    let module = lower(&h.finish(main));

    assert!(
        module
            .functions
            .iter()
            .any(|(_, f)| f.name == "init.Base.$c0")
    );
}

#[test]
fn field_assignment_lowers_to_field_set() {
    // `p.y = 3` on a class with two properties (index 1 in the
    // flattened layout).
    let mut h = Harness::new();
    let int = h.int;
    let c = h.class(
        "C",
        hir::ClassModifier::Final,
        &[("x", int), ("y", int)],
        None,
        &[],
    );
    let c_ty = h.class_ty(c);
    let c_application = h.class_application_of(c_ty);
    let y_field = h.classes[c].fields[1];
    let mut locals = Arena::new();
    let p = locals.alloc(local("p", c_ty));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![stmt(hir::StatementKind::Assign {
                target: hir::AssignTarget::Field {
                    receiver: Box::new(local_ref(p, c_ty)),
                    field: h.class_field_ref(c_application, y_field),
                },
                value: int_lit(&h, 3),
            })],
        },
    );
    let module = lower(&h.finish(main));

    let body = &module.functions[module
        .output
        .executable_entry()
        .expect("test module is executable")]
    .body;
    let mir::StatementKind::FieldSet {
        object,
        index: 1,
        value,
    } = &entry_statements(body)[0].kind
    else {
        panic!("a class property assignment must lower to FieldSet")
    };
    assert!(matches!(object.kind, mir::ExprKind::Local(_)));
    assert!(matches!(
        value.kind,
        mir::ExprKind::IntegerLiteral(mir::MirIntegerConstant::Signed32(3))
    ));
}
