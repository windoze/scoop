use super::*;

#[test]
fn boxing_only_materializes_the_payload_class() {
    let mut h = Harness::new();
    let int = h.int;
    let s = h.strukt("S", &[("x", int)]);
    let s_ty = h.struct_ty(s);
    let any = h.any();
    let mut locals = Arena::new();
    let a = locals.alloc(local("a", any));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![val_decl(
                a,
                expr(
                    hir::ExprKind::Box(Box::new(struct_init(&h, s_ty, vec![int_lit(&h, 1)]))),
                    any,
                ),
            )],
        },
    );
    let export = h.finish(main);
    let concrete = scoop_hir_lower::concretize_legacy_export(&export);
    let module = lower(&export);

    let boxed = boxed_class(&module, "box$D1_SX");
    assert_eq!(boxed.declared_fields().len(), 1);
    assert_eq!(boxed.declared_fields()[0].name, "value");
    assert_eq!(
        boxed.declared_fields()[0].ty,
        mir::Type::Struct(la_arena::Idx::from_raw(0.into()))
    );
    assert!(boxed.vtable.is_empty());
    assert!(boxed.itables.is_empty());
    assert_eq!(module.meta.boxed_types.len(), 1);
    let boxed_meta = &module.meta.boxed_types[0];
    assert_eq!(
        boxed_meta.payload(),
        &mir::Type::Struct(la_arena::Idx::from_raw(0.into()))
    );
    let scoop_identity::GeneratedNominalKey::BoxedValue { payload } =
        boxed_meta.identity().generated_type_record().key()
    else {
        panic!("boxed metadata carries a boxed-value identity")
    };
    let exact_type = concrete
        .module()
        .exact_type_identities
        .type_for_identity(*payload)
        .expect("the MIR payload identity comes from local-concrete HIR");
    let exact = &concrete.module().exact_type_identities[exact_type];
    assert_eq!(
        boxed_meta.identity().generated_type_record().key(),
        &scoop_identity::GeneratedNominalKey::BoxedValue {
            payload: exact.id()
        }
    );
    let scoop_identity::ExactTypeKey::Nominal(owner) = exact.key() else {
        panic!("the parameter-free source struct is nominal")
    };
    assert_eq!(
        boxed_meta.identity().root(),
        &mir::ExactOwnerRoot::SourceNominal(*owner)
    );
    assert_eq!(
        module.classes[boxed_meta.class()].declared_fields()[0].ty,
        mir::Type::Struct(la_arena::Idx::from_raw(0.into()))
    );
    assert_eq!(module.classes[boxed_meta.class()].name, "box$D1_SX");
    assert!(module.functions.iter().all(|(_, function)| {
        !function.symbol.starts_with("scoop.eq.") && !function.symbol.starts_with("scoop.tostring.")
    }));
}

#[test]
fn boxed_interface_implementations_dispatch_through_adjust_thunks() {
    let mut h = Harness::new();
    let int = h.int;
    let iface = h.interface("Describable", &["describe"]);
    h.interfaces[iface].link_stem = nominal_link_stem("$pkg$api$interface$Describable");
    let iface_ty = h.interface_ty(iface);
    let s = h.strukt_with("S", &[("x", int)], &[iface]);
    h.structs[s].link_stem = nominal_link_stem("$pkg$model$struct$S");
    let s_ty = h.struct_ty(s);
    let _describe = empty_method(&mut h, "S", "describe", s_ty);
    // `val d: Describable = S(1)` — a Box whose target is the
    // interface.
    let mut locals = Arena::new();
    let d = locals.alloc(local("d", iface_ty));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![val_decl(
                d,
                expr(
                    hir::ExprKind::Box(Box::new(struct_init(&h, s_ty, vec![int_lit(&h, 1)]))),
                    iface_ty,
                ),
            )],
        },
    );
    let module = lower(&h.finish(main));

    let boxed_meta = module
        .meta
        .boxed_types
        .iter()
        .find(|boxed| matches!(boxed.payload(), mir::Type::Struct(_)))
        .expect("the source struct is boxed");
    let boxed = &module.classes[boxed_meta.class()];
    assert_eq!(boxed.interfaces.len(), 1);
    assert_eq!(boxed.itables.len(), 1);
    let record = &boxed.itables[0];
    assert_eq!(record.interface, boxed.interfaces[0]);
    assert_eq!(record.slots.len(), 1);
    let thunk_symbol = slot_fn(&module, &record.slots[0]);
    let payload = mir::encode_type(&module, boxed_meta.payload()).unwrap();
    let interface = mir::encode_type(&module, &mir::Type::Interface(record.interface)).unwrap();
    assert_eq!(
        thunk_symbol,
        format!("scoop.thunk.{payload}.{interface}.describe")
    );
    assert!(boxed.link_stem.as_str().contains(&payload));
    assert!(!thunk_symbol.contains(".Describable."));

    // The thunk takes the boxed object as `this`, unboxes it and
    // tail-calls the value method.
    let thunk = module
        .functions
        .iter()
        .map(|(_, f)| f)
        .find(|f| f.symbol == thunk_symbol)
        .expect("the thunk is a MIR function");
    assert_eq!(thunk.params.len(), 1);
    assert_eq!(thunk.params[0].ty, mir::Type::Any);
    assert_eq!(thunk.params[0].name, "this");
    let (call, _) = statement_call(&entry_statements(&thunk.body)[0]);
    assert!(matches!(call.target.kind, mir::CallKind::Direct));
    let mir::Callee::User(impl_id) = call.target.callee else {
        panic!("the thunk calls a user function")
    };
    assert_eq!(module.functions[impl_id].symbol, "scoop.S.describe");
    assert_eq!(call.args.len(), 1);
    assert!(matches!(&call.args[0].kind, mir::ExprKind::Unbox(operand)
            if matches!(operand.kind, mir::ExprKind::Local(local) if local == thunk.params[0].local)));
}

#[test]
fn generic_nominal_and_tuple_boxes_use_their_exact_odr_roots() {
    let mut h = Harness::new();
    let (int, boolean) = (h.int, h.boolean);
    let parameter = h
        .types
        .alloc(hir::Type::Param(hir::TypeParamId::from_raw(0)));
    let wrapper = h.declare_struct(
        "Wrapper",
        vec![type_param("T")],
        vec![parameter],
        &[("value", parameter)],
        &[],
    );
    let wrapper_int = h.struct_app(wrapper, vec![int]);
    let tuple = h.tuple(&[int, boolean]);
    let any = h.any();
    let mut locals = Arena::new();
    let wrapped = locals.alloc(local("wrapped", any));
    let paired = locals.alloc(local("paired", any));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![
                val_decl(
                    wrapped,
                    expr(
                        hir::ExprKind::Box(Box::new(struct_init(
                            &h,
                            wrapper_int,
                            vec![int_lit(&h, 1)],
                        ))),
                        any,
                    ),
                ),
                val_decl(
                    paired,
                    expr(
                        hir::ExprKind::Box(Box::new(expr(
                            hir::ExprKind::TupleLiteral(vec![int_lit(&h, 2), bool_lit(&h, true)]),
                            tuple,
                        ))),
                        any,
                    ),
                ),
            ],
        },
    );
    let export = h.finish(main);
    let concrete = scoop_hir_lower::concretize_legacy_export(&export);
    let module = lower(&export);

    let boxed_for = |predicate: fn(&mir::Type) -> bool| {
        module
            .meta
            .boxed_types
            .iter()
            .find(|boxed| predicate(boxed.payload()))
            .expect("both requested box shapes are materialized")
    };
    let nominal = boxed_for(|payload| matches!(payload, mir::Type::Struct(_)));
    let tuple = boxed_for(|payload| matches!(payload, mir::Type::Tuple(_)));
    let exact_for = |boxed: &mir::BoxedType| {
        let scoop_identity::GeneratedNominalKey::BoxedValue { payload } =
            boxed.identity().generated_type_record().key()
        else {
            panic!("boxed metadata carries a boxed-value identity")
        };
        let ty = concrete
            .module()
            .exact_type_identities
            .type_for_identity(*payload)
            .expect("the MIR payload identity comes from local-concrete HIR");
        &concrete.module().exact_type_identities[ty]
    };
    let nominal_exact = exact_for(nominal);
    let tuple_exact = exact_for(tuple);
    let mir::ExactOwnerRoot::NominalApplication(nominal_root) = nominal.identity().root() else {
        panic!("generic nominal boxes are ODR-owned")
    };
    let scoop_identity::ExactTypeKey::NominalApplication { origin, arguments } =
        nominal_exact.key()
    else {
        panic!("Wrapper<Int> is a nominal application")
    };
    let expected_group =
        scoop_identity::CborIdentityRecord::from_key(scoop_identity::SpecializationKey::Nominal {
            origin: *origin,
            arguments: arguments.clone(),
        })
        .unwrap();
    assert_eq!(nominal_root.group(), expected_group.id());
    let nominal_ty = concrete
        .module()
        .exact_type_identities
        .type_for_identity(nominal_exact.id())
        .unwrap();
    assert_eq!(
        nominal_root.group(),
        concrete
            .module()
            .exact_type_identities
            .nominal_specialization(nominal_ty)
            .unwrap()
            .id()
    );
    let mir::ExactOwnerRoot::Structural(tuple_root) = tuple.identity().root() else {
        panic!("tuple boxes are ODR-owned")
    };
    assert_eq!(
        tuple_root.group_record().key(),
        &scoop_identity::SpecializationKey::StructuralType {
            exact_type: tuple_exact.id()
        }
    );
}
