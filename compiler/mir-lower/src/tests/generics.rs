//! Generic nominal specialization and ordinary generic core calls.

use super::*;

#[test]
fn monomorphization_metadata_preserves_persistent_materializations() {
    use scoop_identity::{
        CallableApplicationKey, CallableInstantiationOwner, CallableMaterialization,
        CallableMaterializationContext, CallableTemplateOwner, CanonicalIdentifier,
        CborIdentityRecord, ConeIdentity, CoreBuiltinNominal, DeclarationScope,
        DefinitionOwnerChain, ExactTypeKey, NonEmptyVec, PackagePath, SourceDeclarationKey,
        SourceDeclarationSite,
    };

    let site = || {
        SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap()
    };
    let source = |name: &str, type_parameters| {
        SourceDeclarationKey::function(
            site(),
            CanonicalIdentifier::new(name).unwrap(),
            type_parameters,
            None,
            Vec::new(),
        )
    };
    let generic = match scoop_hir::HirSourceFunctionIdentity::from_declaration(source(
        "identity", 1,
    ))
    .unwrap()
    {
        scoop_hir::HirSourceFunctionIdentity::Generic(record) => record.id(),
        scoop_hir::HirSourceFunctionIdentity::Plain(_) => unreachable!(),
    };
    let method =
        match scoop_hir::HirSourceFunctionIdentity::from_declaration(source("get", 0)).unwrap() {
            scoop_hir::HirSourceFunctionIdentity::Plain(record) => record.id(),
            scoop_hir::HirSourceFunctionIdentity::Generic(_) => unreachable!(),
        };
    let generic_method =
        match scoop_hir::HirSourceFunctionIdentity::from_declaration(source("convert", 1)).unwrap()
        {
            scoop_hir::HirSourceFunctionIdentity::Generic(record) => record.id(),
            scoop_hir::HirSourceFunctionIdentity::Plain(_) => unreachable!(),
        };
    let exact = |nominal: CoreBuiltinNominal| {
        CborIdentityRecord::from_key(ExactTypeKey::Nominal(nominal.identity_record().id()))
            .unwrap()
            .id()
    };
    let unit = exact(CoreBuiltinNominal::Unit);
    let any = exact(CoreBuiltinNominal::Any);
    let application = |key: &CallableApplicationKey| {
        scoop_identity::PersistentCallableApplicationId::from_key(key).unwrap()
    };
    let identity_unit = CallableApplicationKey::for_generic_function(
        generic,
        CallableInstantiationOwner::NoOwner,
        NonEmptyVec::from_first(unit, []),
    );
    let identity_any = CallableApplicationKey::for_generic_function(
        generic,
        CallableInstantiationOwner::NoOwner,
        NonEmptyVec::from_first(any, []),
    );
    let get = CallableApplicationKey::for_function(
        method,
        CallableInstantiationOwner::ExactNominalOwner(any),
    );
    let convert = CallableApplicationKey::for_generic_function(
        generic_method,
        CallableInstantiationOwner::ExactNominalOwner(any),
        NonEmptyVec::from_first(unit, []),
    );
    let materialization = |template, key: &CallableApplicationKey| {
        CallableMaterialization::new(
            template,
            CallableMaterializationContext::Application(application(key)),
        )
    };
    let materializations = [
        materialization(
            CallableTemplateOwner::GenericFunction(generic),
            &identity_unit,
        ),
        materialization(
            CallableTemplateOwner::GenericFunction(generic),
            &identity_any,
        ),
        materialization(CallableTemplateOwner::Function(method), &get),
        materialization(
            CallableTemplateOwner::GenericFunction(generic_method),
            &convert,
        ),
    ];

    let mut registry = InstanceRegistry::default();
    let hir_function = |raw: u32| hir::concrete::FunctionId::from_raw(raw.into());
    let mir_function = |raw: u32| mir::FunctionId::from_raw(raw.into());
    for (index, name) in ["identity", "identity", "Box.get", "Host.convert"]
        .into_iter()
        .enumerate()
    {
        registry.record(
            hir_function(index as u32),
            mir_function(index as u32),
            name.to_string(),
            materializations[index],
        );
    }

    assert_eq!(registry.meta.len(), 4);
    for (index, expected) in materializations.into_iter().enumerate() {
        let instance = &registry.meta
            [mir::MonomorphizedFunctionId::from_raw(u32::try_from(index).unwrap().into())];
        assert_eq!(instance.materialization, expected);
    }
    assert_eq!(
        registry.meta[mir::MonomorphizedFunctionId::from_raw(1_u32.into())].display_name,
        "identity"
    );
    assert_eq!(
        registry.get(hir_function(3)),
        Some(mir::MonomorphizedFunctionId::from_raw(3_u32.into()))
    );
}

#[test]
fn generic_structs_instantiate_per_argument_list() {
    let mut h = Harness::new();
    let gc = h.gc_core();
    let (string, uint) = (h.string, h.uint());
    // Two applications of one generic struct, one of them twice
    // (dedup), plus a struct whose field mentions its type
    // parameter (the general substitution path).
    let pinned_ptr_v = h.struct_app(gc.pinned_ptr, vec![uint]);
    let pinned_ptr_s = h.struct_app(gc.pinned_ptr, vec![string]);
    let t = h
        .types
        .alloc(hir::Type::Param(hir::TypeParamId::from_raw(0)));
    let box2 = h.declare_struct("Box2", vec![type_param("T")], vec![t], &[("x", t)], &[]);
    let box2_s = h.struct_app(box2, vec![string]);
    let mut locals = Arena::new();
    let a = locals.alloc(local("a", pinned_ptr_v));
    let b = locals.alloc(local("b", pinned_ptr_s));
    let c = locals.alloc(local("c", pinned_ptr_s));
    let d = locals.alloc(local("d", box2_s));
    let raw_one = integer_lit(&h, hir::IntegerKind::UNSIGNED_64, 1);
    let raw_two = integer_lit(&h, hir::IntegerKind::UNSIGNED_64, 2);
    let raw_three = integer_lit(&h, hir::IntegerKind::UNSIGNED_64, 3);
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: vec![
                val_decl(a, struct_init(&h, pinned_ptr_v, vec![raw_one])),
                val_decl(b, struct_init(&h, pinned_ptr_s, vec![raw_two])),
                val_decl(c, struct_init(&h, pinned_ptr_s, vec![raw_three])),
                val_decl(d, struct_init(&h, box2_s, vec![str_lit(&h, "x")])),
            ],
        },
    );
    let module = lower(&h.finish(main));

    // One instance per typed argument list. The source declaration name is
    // display-only; concrete arguments, not an encoded name, distinguish the
    // two PinnedPtr instances.
    let defs = |name: &str| {
        module
            .structs
            .iter()
            .filter(|(_, def)| def.name == name)
            .map(|(_, def)| def)
            .collect::<Vec<_>>()
    };
    let pinned = defs("PinnedPtr");
    assert_eq!(pinned.len(), 2);
    let pinned_uint = pinned
        .iter()
        .find(|definition| {
            definition.type_arguments == [mir::Type::Integer(mir::IntegerKind::UNSIGNED_32)]
        })
        .expect("PinnedPtr<UInt>");
    assert_eq!(
        pinned_uint.declared_fields()[0].ty,
        mir::Type::Integer(mir::IntegerKind::UNSIGNED_64)
    );
    assert!(pinned_uint.gc_free);
    let pinned_string = pinned
        .iter()
        .find(|definition| definition.type_arguments == [mir::Type::String])
        .expect("PinnedPtr<String>");
    assert!(pinned_string.gc_free);
    assert_eq!(
        pinned_uint.declared_fields()[0].identity,
        pinned_string.declared_fields()[0].identity,
        "specializations of one source struct retain one source field identity"
    );
    let box2 = defs("Box2");
    assert_eq!(box2.len(), 1);
    assert_eq!(box2[0].type_arguments, [mir::Type::String]);
    // Field substitution: `Box2<String>`'s `x` is `String`.
    assert_eq!(box2[0].declared_fields()[0].ty, mir::Type::String);
    assert_ne!(
        box2[0].declared_fields()[0].identity,
        pinned_uint.declared_fields()[0].identity,
        "fields owned by different source structs remain distinct"
    );
    assert!(!box2[0].gc_free);

    // Locals and StructInits resolve to the instances.
    let body = &module.functions[module
        .output
        .executable_entry()
        .expect("test module is executable")]
    .body;
    let instance_of = |local: mir::LocalId| {
        let mir::Type::Struct(id) = &body.locals[local].ty else {
            panic!("a struct local")
        };
        &module.structs[*id]
    };
    let (first_constructor, la) = statement_call(&entry_statements(body)[0]);
    let (_, lb) = statement_call(&entry_statements(body)[1]);
    let (_, lc) = statement_call(&entry_statements(body)[2]);
    let (_, ld) = statement_call(&entry_statements(body)[3]);
    let (la, lb, lc, ld) = (
        la.expect("struct constructor returns its value"),
        lb.expect("struct constructor returns its value"),
        lc.expect("struct constructor returns its value"),
        ld.expect("struct constructor returns its value"),
    );
    assert_eq!(instance_of(la).name, "PinnedPtr");
    assert_eq!(
        instance_of(la).type_arguments,
        [mir::Type::Integer(mir::IntegerKind::UNSIGNED_32)]
    );
    assert_eq!(instance_of(lb).name, "PinnedPtr");
    assert_eq!(instance_of(lb).type_arguments, [mir::Type::String]);
    assert_eq!(instance_of(lc).name, "PinnedPtr");
    assert_eq!(instance_of(lc).type_arguments, [mir::Type::String]);
    assert_eq!(instance_of(ld).name, "Box2");
    assert_eq!(instance_of(ld).type_arguments, [mir::Type::String]);
    let mir::Callee::User(first_constructor) = first_constructor.target.callee else {
        panic!("a struct constructor is a direct user function")
    };
    let constructor_source = module
        .meta
        .source_callable_materializations
        .get(first_constructor)
        .expect("the struct constructor has an exact MIR location");
    assert!(matches!(
        constructor_source.materialization().template(),
        scoop_identity::CallableTemplateOwner::Constructor(_)
    ));
    let constructor = &module.functions[first_constructor];
    let signature = constructor_source.signature_record().signature();
    let parameters = constructor
        .params
        .iter()
        .map(|parameter| {
            module
                .meta
                .source_exact_types
                .get(&parameter.ty)
                .unwrap()
                .identity_record()
                .id()
        })
        .collect::<Vec<_>>();
    let result = module
        .meta
        .source_exact_types
        .get(&constructor.return_ty)
        .unwrap()
        .identity_record()
        .id();
    assert!(!signature.receiver().is_present());
    assert_eq!(signature.parameters(), parameters);
    assert_eq!(signature.result(), result);
    assert_eq!(
        module.functions[first_constructor].gc_effect,
        mir::GcEffect::NoGc,
        "primary struct construction only assembles evaluated fields"
    );
    let mir::Terminator::Return {
        value:
            Some(mir::Expr {
                kind: mir::ExprKind::StructInit { struct_id, .. },
                ..
            }),
    } = &module.functions[first_constructor].body.blocks
        [module.functions[first_constructor].body.entry]
        .terminator
    else {
        panic!("a primary struct constructor returns a StructInit")
    };
    assert_eq!(module.structs[*struct_id].name, "PinnedPtr");
    assert_eq!(
        module.structs[*struct_id].type_arguments,
        [mir::Type::Integer(mir::IntegerKind::UNSIGNED_32)]
    );
}

#[test]
fn generic_interface_applications_get_distinct_mir_identities() {
    let mut h = Harness::new();
    let t = h
        .types
        .alloc(hir::Type::Param(hir::TypeParamId::from_raw(0)));
    let interface = h.declare_interface(
        "Channel",
        vec![hir::TypeParamDecl {
            id: hir::TypeParamId::from_raw(0),
            name: "T".to_string(),
            bounds: hir::TypeParamBounds::Unconstrained,
            span: SPAN,
        }],
        vec![t],
        Vec::new(),
    );
    let (int, string) = (h.int, h.string);
    let int_channel = h.interface_app(interface, vec![int]);
    let string_channel = h.interface_app(interface, vec![string]);
    let ints = h.declare_class(
        "Ints",
        hir::ClassModifier::Final,
        &[],
        None,
        vec![int_channel],
    );
    let strings = h.declare_class(
        "Strings",
        hir::ClassModifier::Final,
        &[],
        None,
        vec![string_channel],
    );
    let mut locals = Arena::new();
    let parameters = [ints, strings]
        .into_iter()
        .enumerate()
        .map(|(index, owner)| {
            let ty = h.class_ty(owner);
            let name = format!("value{index}");
            let value = locals.alloc(local(&name, ty));
            param(&name, ty, value)
        })
        .collect();
    h.user_fn_full(
        "observeChannels",
        Vec::new(),
        parameters,
        h.unit,
        hir::Body {
            locals,
            statements: Vec::new(),
        },
    );
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: Vec::new(),
        },
    );
    let module = lower(&h.finish(main));

    let channels = module
        .interfaces
        .iter()
        .filter_map(|(_, interface)| (interface.name == "Channel").then_some(interface))
        .collect::<Vec<_>>();
    assert_eq!(channels.len(), 2);
    assert_eq!(
        channels[0].type_arguments,
        [mir::Type::Integer(mir::IntegerKind::SIGNED_32)]
    );
    assert_eq!(channels[1].type_arguments, [mir::Type::String]);
    let class_interfaces: Vec<_> = module
        .classes
        .iter()
        .filter(|(_, class)| class.name == "Ints" || class.name == "Strings")
        .map(|(_, class)| class.interfaces[0])
        .collect();
    assert_ne!(class_interfaces[0], class_interfaces[1]);
}

#[test]
fn generic_interface_signature_types_follow_typed_interface_order() {
    let mut h = Harness::new();
    let parameter = hir::TypeParamId::from_raw(0);
    let t = h.types.alloc(hir::Type::Param(parameter));
    let option_t = h.option(t);
    let source = h.declare_interface(
        "Source",
        vec![hir::TypeParamDecl {
            id: parameter,
            name: "T".to_string(),
            bounds: hir::TypeParamBounds::Unconstrained,
            span: SPAN,
        }],
        vec![t],
        vec![hir::MethodSig {
            name: "next".to_string(),
            is_suspend: false,
            attributes: hir::FunctionAttributes::default(),
            type_params: Vec::new(),
            params: Vec::new(),
            return_ty: option_t,
            span: SPAN,
        }],
    );
    let (int, uint) = (h.int, h.uint);
    let int_source = h.interface_app(source, vec![int]);
    let uint_source = h.interface_app(source, vec![uint]);
    let mut locals = Arena::new();
    locals.alloc(local("ints", int_source));
    locals.alloc(local("uints", uint_source));
    let main = h.user_fn(
        "main",
        hir::Body {
            locals,
            statements: Vec::new(),
        },
    );
    let module = lower(&h.finish(main));

    let sources = module
        .interfaces
        .iter()
        .filter_map(|(_, interface)| (interface.name == "Source").then_some(interface))
        .collect::<Vec<_>>();
    assert_eq!(sources.len(), 2);
    assert_eq!(
        sources[0].type_arguments,
        [mir::Type::Integer(mir::IntegerKind::SIGNED_32)]
    );
    assert_eq!(
        sources[1].type_arguments,
        [mir::Type::Integer(mir::IntegerKind::UNSIGNED_32)]
    );
    let options = module
        .enums
        .iter()
        .filter_map(|(_, enumeration)| (enumeration.name == "Option").then_some(enumeration))
        .collect::<Vec<_>>();
    assert_eq!(options.len(), 2);
    assert_eq!(
        options[0].type_arguments,
        [mir::Type::Integer(mir::IntegerKind::SIGNED_32)]
    );
    assert_eq!(
        options[1].type_arguments,
        [mir::Type::Integer(mir::IntegerKind::UNSIGNED_32)]
    );
}

#[test]
fn print_overloads_are_ordinary_calls() {
    // M7: calls to core's `print` / `println` overloads resolve to
    // the overload's own MIR function (`Callee::User`); only the
    // intrinsic primitives inside their bodies are runtime calls.
    let mut h = Harness::new();
    let print_string = h.print_string();
    let print_int = h.print_int();
    let print_boolean = h.print_boolean();
    let println_string = h.println_string();
    let println_int = h.println_int();
    let println_boolean = h.println_boolean();
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: vec![
                expr_stmt(call(&h, print_string, vec![str_lit(&h, "s")])),
                expr_stmt(call(&h, print_int, vec![int_lit(&h, 1)])),
                expr_stmt(call(&h, print_boolean, vec![bool_lit(&h, true)])),
                expr_stmt(call(&h, println_string, vec![str_lit(&h, "t")])),
                expr_stmt(call(&h, println_int, vec![int_lit(&h, 2)])),
                expr_stmt(call(&h, println_boolean, vec![bool_lit(&h, false)])),
            ],
        },
    );
    let module = lower(&h.finish(main));

    let body = &module.functions[module
        .output
        .executable_entry()
        .expect("test module is executable")]
    .body;
    let callees: Vec<mir::FunctionId> = entry_statements(body)
        .iter()
        .map(|statement| {
            let (call, _) = statement_call(statement);
            let mir::Callee::User(id) = call.target.callee else {
                panic!("print/println calls must be ordinary user calls")
            };
            id
        })
        .collect();
    // Each call retains the overload selected by HIR. Extern provider
    // entries also occur in the ordinary declaration order.
    let overloads = module
        .top_level
        .iter()
        .copied()
        .filter(|&id| matches!(module.functions[id].name.as_str(), "print" | "println"))
        .collect::<Vec<_>>();
    assert_eq!(callees, overloads);
    let names: Vec<&str> = callees
        .iter()
        .map(|&id| module.functions[id].name.as_str())
        .collect();
    assert_eq!(
        names,
        ["print", "print", "print", "println", "println", "println",]
    );
}
