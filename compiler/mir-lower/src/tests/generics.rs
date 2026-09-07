//! Generic nominal specialization and ordinary generic core calls.

use super::*;
use crate::instances::LoweredFunctionInstance;

#[test]
fn monomorphization_metadata_preserves_typed_sources_and_argument_groups() {
    let mut registry = InstanceRegistry::default();
    let hir_function = |raw: u32| hir::concrete::FunctionId::from_raw(raw.into());
    let mir_function = |raw: u32| mir::FunctionId::from_raw(raw.into());
    let arguments = |first, rest| mir::NonEmptyTypeArguments::new(first, rest);

    registry.record(
        hir_function(0),
        mir_function(0),
        "scoop.identity$I32".to_string(),
        "identity".to_string(),
        LoweredFunctionInstance::GenericFunction {
            origin: hir::concrete::GenericFunctionOriginId::from_raw(7),
            arguments: arguments(mir::Type::Integer(mir::IntegerKind::SIGNED_32), Vec::new()),
        },
    );
    registry.record(
        hir_function(1),
        mir_function(1),
        "scoop.identity$S".to_string(),
        "identity".to_string(),
        LoweredFunctionInstance::GenericFunction {
            origin: hir::concrete::GenericFunctionOriginId::from_raw(7),
            arguments: arguments(mir::Type::String, Vec::new()),
        },
    );
    registry.record(
        hir_function(2),
        mir_function(2),
        "scoop.Box$I32.get$I32".to_string(),
        "Box.get".to_string(),
        LoweredFunctionInstance::ParameterizedMethod {
            origin: hir::concrete::OwnerParameterizedMethodOriginId::from_raw(11),
            owner: mir::MonomorphizedMethodOwner::Class(mir::ClassId::from_raw(3_u32.into())),
            owner_arguments: arguments(mir::Type::Integer(mir::IntegerKind::SIGNED_32), Vec::new()),
        },
    );
    registry.record(
        hir_function(3),
        mir_function(3),
        "scoop.Host.convert$S".to_string(),
        "Host.convert".to_string(),
        LoweredFunctionInstance::GenericMethod {
            origin: hir::concrete::GenericMethodOriginId::from_raw(13),
            owner: mir::MonomorphizedMethodOwner::Class(mir::ClassId::from_raw(4_u32.into())),
            owner_arguments: Vec::new(),
            method_arguments: arguments(mir::Type::String, Vec::new()),
        },
    );

    assert_eq!(registry.generic_function_sources.len(), 1);
    assert_eq!(registry.parameterized_method_sources.len(), 1);
    assert_eq!(registry.generic_method_sources.len(), 1);
    assert_eq!(registry.meta.len(), 4);

    let mir::MonomorphizedSource::GenericFunction { source, arguments } =
        &registry.meta[mir::MonomorphizedFunctionId::from_raw(1_u32.into())].source
    else {
        panic!("the second instance must retain its free-function category")
    };
    assert_eq!(
        registry.generic_function_sources[*source].display_name,
        "identity"
    );
    assert_eq!(arguments.to_vec(), vec![mir::Type::String]);

    let mir::MonomorphizedSource::ParameterizedMethod {
        owner,
        owner_arguments,
        ..
    } = &registry.meta[mir::MonomorphizedFunctionId::from_raw(2_u32.into())].source
    else {
        panic!("the owner-parameterized method category must be retained")
    };
    assert!(matches!(owner, mir::MonomorphizedMethodOwner::Class(_)));
    assert_eq!(
        owner_arguments.to_vec(),
        vec![mir::Type::Integer(mir::IntegerKind::SIGNED_32)]
    );

    let mir::MonomorphizedSource::GenericMethod {
        owner,
        owner_arguments,
        method_arguments,
        ..
    } = &registry.meta[mir::MonomorphizedFunctionId::from_raw(3_u32.into())].source
    else {
        panic!("the generic-method category must be retained")
    };
    assert!(matches!(owner, mir::MonomorphizedMethodOwner::Class(_)));
    assert!(owner_arguments.is_empty());
    assert_eq!(method_arguments.to_vec(), vec![mir::Type::String]);
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

    // One instance per (struct, args): `PinnedPtr$V32` once,
    // `PinnedPtr$S` once despite two uses, `Box2$S` once — named
    // like the enum instances. Generic definitions themselves do
    // not survive into MIR: MIR contains no generic types.
    let defs = |name: &str| {
        module
            .structs
            .iter()
            .filter(|(_, def)| def.name == name)
            .map(|(_, def)| def)
            .collect::<Vec<_>>()
    };
    assert!(defs("PinnedPtr").is_empty());
    assert!(defs("Box2").is_empty());
    assert_eq!(defs("PinnedPtr$V32").len(), 1);
    assert_eq!(
        defs("PinnedPtr$V32")[0].declared_fields()[0].ty,
        mir::Type::Integer(mir::IntegerKind::UNSIGNED_64)
    );
    assert!(defs("PinnedPtr$V32")[0].gc_free);
    assert_eq!(defs("PinnedPtr$S").len(), 1);
    assert!(defs("PinnedPtr$S")[0].gc_free);
    assert_eq!(defs("Box2$S").len(), 1);
    // Field substitution: `Box2<String>`'s `x` is `String`.
    assert_eq!(defs("Box2$S")[0].declared_fields()[0].ty, mir::Type::String);
    assert!(!defs("Box2$S")[0].gc_free);

    // Locals and StructInits resolve to the instances.
    let body = &module.functions[module.entry].body;
    let instance_of = |local: mir::LocalId| {
        let mir::Type::Struct(id) = &body.locals[local].ty else {
            panic!("a struct local")
        };
        module.structs[*id].name.as_str()
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
    assert_eq!(instance_of(la), "PinnedPtr$V32");
    assert_eq!(instance_of(lb), "PinnedPtr$S");
    assert_eq!(instance_of(lc), "PinnedPtr$S");
    assert_eq!(instance_of(ld), "Box2$S");
    let mir::Callee::User(first_constructor) = first_constructor.target.callee else {
        panic!("a struct constructor is a direct user function")
    };
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
    assert_eq!(module.structs[*struct_id].name, "PinnedPtr$V32");
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
    h.declare_class(
        "Ints",
        hir::ClassModifier::Final,
        &[],
        None,
        vec![int_channel],
    );
    h.declare_class(
        "Strings",
        hir::ClassModifier::Final,
        &[],
        None,
        vec![string_channel],
    );
    let main = h.user_fn(
        "main",
        hir::Body {
            locals: Arena::new(),
            statements: Vec::new(),
        },
    );
    let module = lower(&h.finish(main));

    let names: Vec<_> = module
        .interfaces
        .iter()
        .map(|(_, interface)| interface.name.as_str())
        .collect();
    assert_eq!(names, ["Channel$I32", "Channel$S"]);
    let class_interfaces: Vec<_> = module
        .classes
        .iter()
        .filter(|(_, class)| class.name == "Ints" || class.name == "Strings")
        .map(|(_, class)| class.interfaces[0])
        .collect();
    assert_ne!(class_interfaces[0], class_interfaces[1]);
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

    let body = &module.functions[module.entry].body;
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
    // The overloads are the first six MIR functions (declaration
    // order: the three `print`s, then the three `println`s), and
    // each overload's symbol carries the parameter encoding.
    assert_eq!(callees, module.top_level[..6]);
    let symbols: Vec<&str> = callees
        .iter()
        .map(|&id| module.functions[id].symbol.as_str())
        .collect();
    assert_eq!(
        symbols,
        [
            "scoop.print.S",
            "scoop.print.I32",
            "scoop.print.B",
            "scoop.println.S",
            "scoop.println.I32",
            "scoop.println.B",
        ]
    );
}
