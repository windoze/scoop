use super::*;

#[test]
fn bounded_receiver_call_records_exact_interface_member_identity() {
    let show = interface_decl(
        "Show",
        vec![method_full(
            false,
            true,
            "show",
            Vec::new(),
            Some(ty_named("Int")),
            FunctionBody::None,
        )],
    );
    let shown = struct_decl_full(
        "Shown",
        Vec::new(),
        vec!["Show"],
        vec![method_full(
            true,
            false,
            "show",
            Vec::new(),
            Some(ty_named("Int")),
            FunctionBody::Expr(Box::new(int_lit(7))),
        )],
    );
    let mut read = fun_expr(
        "read",
        vec!["T"],
        vec![("value", ty_named("T"))],
        Some(ty_named("Int")),
        method_call(var("value"), "show", Vec::new()),
    );
    let Decl::Function(read_decl) = &mut read else {
        unreachable!()
    };
    read_decl.type_params[0] = upper("T", ty_named("Show"));

    let output = lower_user_output(file(vec![
        show,
        shown,
        read,
        fun(
            "main",
            vec![stmt(call("read", vec![struct_init("Shown", Vec::new())]))],
        ),
    ]))
    .expect("a bound method must resolve from the declared interface capability");

    let bound = output
        .export
        .bound_callable_refs
        .iter()
        .map(|(_, bound)| bound)
        .find(|bound| {
            let hir::BoundCallableSource::Interface { member, .. } = bound.source else {
                return false;
            };
            let member = &output.export.interface_methods[member];
            output.export.interfaces[member.owner].name == "Show"
        })
        .expect("read<T> has one non-core bound call");
    let hir::BoundCallableSource::Interface { member, .. } = bound.source else {
        panic!("Show is an interface-bound member")
    };
    let member = &output.export.interface_methods[member];
    assert_eq!(
        output.export.functions[member.function]
            .name
            .rsplit('.')
            .next(),
        Some("show")
    );
    let shown = output
        .export
        .structs
        .iter()
        .find(|(_, declaration)| declaration.name == "Shown")
        .unwrap()
        .1;
    let hir::InterfaceImplementationTarget::Method(application) =
        shown.interface_implementations[0].methods[0].target
    else {
        panic!("Shown conformance has an exact method application")
    };
    assert_eq!(
        output.export.functions[output.export.method_applications[application].function].name,
        "Shown.show"
    );
    let shown = output
        .local
        .structs
        .iter()
        .find(|(_, declaration)| declaration.name == "Shown")
        .expect("Shown has one concrete specialization")
        .1;
    assert_eq!(shown.interface_implementations.len(), 1);
    let implementation = &shown.interface_implementations[0];
    assert_eq!(implementation.methods.len(), 1);
    assert_eq!(implementation.methods[0].slot.into_raw(), 0);
    let hir::concrete::InterfaceImplementationTarget::Method(target) =
        implementation.methods[0].target
    else {
        panic!("value-type conformances are complete concrete method targets")
    };
    assert_eq!(output.local.functions[target].name, "Shown.show");

    let read = output
        .local
        .functions
        .iter()
        .find(|(_, function)| {
            function.name == "read"
                && matches!(
                    function.materialization.context(),
                    hir::concrete::CallableMaterializationContext::Application(_)
                )
        })
        .expect("read<Shown> specialization")
        .1;
    let hir::concrete::FunctionKind::User(body) = &read.kind else {
        panic!("read specialization must have a user body")
    };
    let hir::concrete::ExprKind::MethodCall { callee, .. } = &body
        .statements
        .iter()
        .rev()
        .find_map(|statement| match &statement.kind {
            hir::concrete::StatementKind::Return { value: Some(value) } => Some(value),
            _ => None,
        })
        .expect("read specialization must return a value")
        .kind
    else {
        panic!("read specialization must return its bound call")
    };
    let target = output.local.callable_function(*callee);
    assert_eq!(output.local.functions[target].name, "Shown.show");
}

#[test]
fn bound_member_inherits_through_exact_parent_application() {
    let parent = interface_decl(
        "Parent",
        vec![method_full(
            false,
            true,
            "value",
            Vec::new(),
            Some(ty_named("Int")),
            FunctionBody::None,
        )],
    );
    let mut child = interface_decl("Child", Vec::new());
    let Decl::Interface(child_decl) = &mut child else {
        unreachable!()
    };
    child_decl.supertypes = vec![bare_supertype(ty_named("Parent"))];
    let implementation = struct_decl_full(
        "Implementation",
        Vec::new(),
        vec!["Child"],
        vec![method_full(
            true,
            false,
            "value",
            Vec::new(),
            Some(ty_named("Int")),
            FunctionBody::Expr(Box::new(int_lit(11))),
        )],
    );
    let mut read = fun_expr(
        "readParent",
        vec!["T"],
        vec![("value", ty_named("T"))],
        Some(ty_named("Int")),
        method_call(var("value"), "value", Vec::new()),
    );
    let Decl::Function(read_decl) = &mut read else {
        unreachable!()
    };
    read_decl.type_params[0] = upper("T", ty_named("Child"));

    let output = lower_user_output(file(vec![
        parent,
        child,
        implementation,
        read,
        fun(
            "main",
            vec![stmt(call(
                "readParent",
                vec![struct_init("Implementation", Vec::new())],
            ))],
        ),
    ]))
    .expect("a bound exposes members inherited from its exact parent application");

    let bound = output
        .export
        .bound_callable_refs
        .iter()
        .map(|(_, bound)| bound)
        .find(|bound| {
            let hir::BoundCallableSource::Interface { member, .. } = bound.source else {
                return false;
            };
            let member = &output.export.interface_methods[member];
            output.export.interfaces[member.owner].name == "Parent"
        })
        .expect("readParent<T> has one non-core bound call");
    let hir::BoundCallableSource::Interface {
        bound: root,
        member,
    } = bound.source
    else {
        panic!("Parent is reached through an interface bound")
    };
    assert_eq!(
        output.export.interfaces[output.export.interface_applications[root].template].name,
        "Child"
    );
    assert_eq!(
        output.export.interfaces[output.export.interface_methods[member].owner].name,
        "Parent"
    );
    let implementation = output
        .export
        .structs
        .iter()
        .find(|(_, declaration)| declaration.name == "Implementation")
        .unwrap()
        .1;
    let child_conformance = implementation
        .interface_implementations
        .iter()
        .find(|conformance| {
            let hir::Type::Interface(application) = output.export.types[conformance.interface]
            else {
                return false;
            };
            output.export.interfaces[output.export.interface_applications[application].template]
                .name
                == "Child"
        })
        .expect("Child conformance");
    assert_eq!(
        child_conformance.methods[0].member,
        hir::InterfaceMethodReference::Local(member)
    );
}

#[test]
fn unrelated_bounds_with_the_same_member_are_ambiguous() {
    let method = || {
        method_full(
            false,
            true,
            "value",
            Vec::new(),
            Some(ty_named("Int")),
            FunctionBody::None,
        )
    };
    let mut read = fun_expr(
        "ambiguous",
        vec!["T"],
        vec![("receiver", ty_named("T"))],
        Some(ty_named("Int")),
        method_call(var("receiver"), "value", Vec::new()),
    );
    let Decl::Function(read_decl) = &mut read else {
        unreachable!()
    };
    read_decl.type_params[0] = upper("T", ty_named("Left"));
    read_decl.where_clause = Some(where_clause(vec![(
        "T",
        ast::TypeBound::Upper(ty_named("Right")),
    )]));
    let errors = lower_user(file(vec![
        interface_decl("Left", vec![method()]),
        interface_decl("Right", vec![method()]),
        read,
        fun("main", Vec::new()),
    ]))
    .expect_err("unrelated bounds retain distinct member identities");
    assert!(errors.iter().any(|error| {
        error
            .message
            .starts_with("call to `value` is ambiguous in member candidate layer:")
    }));
}

#[test]
fn interface_inheritance_cycle_is_rejected_at_hir() {
    let mut left = interface_decl("Left", Vec::new());
    let Decl::Interface(left_decl) = &mut left else {
        unreachable!()
    };
    left_decl.supertypes = vec![bare_supertype(ty_named("Right"))];
    let mut right = interface_decl("Right", Vec::new());
    let Decl::Interface(right_decl) = &mut right else {
        unreachable!()
    };
    right_decl.supertypes = vec![bare_supertype(ty_named("Left"))];
    let errors = lower_user(file(vec![left, right, fun("main", Vec::new())]))
        .expect_err("interface inheritance cycles must be diagnosed");
    assert!(errors.iter().any(|error| {
        error.message == "interface `Left` directly or indirectly inherits from itself"
    }));
    assert!(errors.iter().any(|error| {
        error.message == "interface `Right` directly or indirectly inherits from itself"
    }));
}
