use super::*;

#[test]
fn generic_iterator_bound_concretizes_value_and_reference_results() {
    let value_iterator = struct_with_interface(
        "ValueIterator",
        ty_generic("Iterator", vec![ty_named("Int")]),
        vec![iterator_next(ty_named("Int"))],
    );
    let reference_iterator = iterator_class("ReferenceIterator", ty_named("Int"));
    let value_source = struct_with_interface(
        "ValueSource",
        ty_generic("IterationSource", vec![ty_named("ValueIterator")]),
        vec![operator_method(
            true,
            ty_named("ValueIterator"),
            struct_init("ValueIterator", Vec::new()),
        )],
    );
    let reference_source = class_with_interface(
        "ReferenceSource",
        ty_generic("IterationSource", vec![ty_named("ReferenceIterator")]),
        vec![operator_method(
            true,
            ty_named("ReferenceIterator"),
            call("ReferenceIterator", Vec::new()),
        )],
    );
    let consume = generic_iteration_consumer();

    let output = lower_user_output(file(vec![
        iteration_source_interface(),
        value_iterator,
        reference_iterator,
        value_source,
        reference_source,
        consume,
        fun(
            "main",
            vec![
                stmt(typed_call(
                    "consume",
                    vec![ty_named("ValueIterator"), ty_named("ValueSource")],
                    vec![struct_init("ValueSource", Vec::new())],
                )),
                stmt(typed_call(
                    "consume",
                    vec![ty_named("ReferenceIterator"), ty_named("ReferenceSource")],
                    vec![call("ReferenceSource", Vec::new())],
                )),
            ],
        ),
    ]))
    .expect("one generic bound witness must concretize for value and reference iterators");

    let mut saw_value = false;
    let mut saw_reference = false;
    for (_, function) in output.local.functions.iter().filter(|(_, function)| {
        function.name == "consume"
            && matches!(
                function.materialization.context(),
                hir::concrete::CallableMaterializationContext::Application(_)
            )
    }) {
        let hir::concrete::FunctionKind::User(body) = &function.kind else {
            panic!("a consume specialization must have a body")
        };
        let raw = concrete_local_with_prefix(body, "$for.iterator.result.");
        let adapted = body
            .locals
            .iter()
            .find_map(|(id, local)| {
                (local.name.starts_with("$for.iterator.")
                    && !local.name.starts_with("$for.iterator.result."))
                .then_some(id)
            })
            .expect("the specialization stores its exact Iterator<Int> view");
        let init = concrete_local_init(body, adapted);
        match &output.local.types[body.locals[raw].ty].kind {
            hir::concrete::TypeKind::Struct(_) => {
                assert!(matches!(&init.kind, hir::concrete::ExprKind::Box(_)));
                saw_value = true;
            }
            hir::concrete::TypeKind::Class(_) => {
                assert!(matches!(&init.kind, hir::concrete::ExprKind::Local(_)));
                saw_reference = true;
            }
            other => panic!("unexpected iterator result kind {other:?}"),
        }
    }
    assert!(
        saw_value,
        "ValueIterator must be boxed exactly at conformance materialization"
    );
    assert!(
        saw_reference,
        "ReferenceIterator must use a zero-cost interface retype"
    );
}

#[test]
fn generic_iteration_concretization_keeps_the_export_selected_winner() {
    let mut extension = extension_expr(
        ty_named("T"),
        "iterator",
        vec!["T"],
        Vec::new(),
        Some(ty_named("SavedIterator")),
        call("SavedIterator", Vec::new()),
    );
    let Decl::Function(extension_function) = &mut extension else {
        unreachable!()
    };
    extension_function.operator = Some(ast::OperatorModifier { span: sp() });
    extension_function.type_params[0] = upper("T", ty_named("Marker"));

    let mut consume = fun_sig(
        "consume",
        vec!["S"],
        vec![("source", ty_named("S"))],
        None,
        vec![for_stmt(pat_bind("item"), var("source"), Vec::new())],
    );
    let Decl::Function(consume_function) = &mut consume else {
        unreachable!()
    };
    consume_function.type_params[0] = upper("S", ty_named("Marker"));

    let output = lower_user_output(file(vec![
        interface_decl("Marker", Vec::new()),
        iterator_class("SavedIterator", ty_named("Int")),
        iterator_class("ConcreteIterator", ty_named("Int")),
        extension,
        class_with_interface(
            "ConcreteSource",
            ty_named("Marker"),
            vec![operator_method(
                false,
                ty_named("ConcreteIterator"),
                call("ConcreteIterator", Vec::new()),
            )],
        ),
        consume,
        fun(
            "main",
            vec![stmt(typed_call(
                "consume",
                vec![ty_named("ConcreteSource")],
                vec![call("ConcreteSource", Vec::new())],
            ))],
        ),
    ]))
    .expect("the generic body selects the bound-compatible extension once");

    assert_eq!(
        export_callee_name(
            &output.export,
            first_for(export_body(&output.export, "consume")).iterator_call(),
        ),
        "iterator"
    );
    let specialization = output
        .local
        .functions
        .iter()
        .find_map(|(_, function)| {
            (function.name == "consume"
                && matches!(
                    function.materialization.context(),
                    hir::concrete::CallableMaterializationContext::Application(_)
                ))
            .then_some(function)
        })
        .expect("consume has one concrete specialization");
    let hir::concrete::FunctionKind::User(body) = &specialization.kind else {
        panic!("the consume specialization has a body")
    };
    let raw = concrete_local_with_prefix(body, "$for.iterator.result.");
    assert_eq!(
        concrete_callee_name(&output.local, concrete_local_init(body, raw)),
        "iterator"
    );
}
