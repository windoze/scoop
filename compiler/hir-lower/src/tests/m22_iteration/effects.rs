use super::*;

#[test]
fn no_gc_for_checks_the_hidden_canonical_next_call() {
    let annotation = || ast::Annotation {
        name: ident("NoGC"),
        args: Vec::new(),
        span: sp(),
    };

    let mut iterator = struct_with_interface(
        "NoGcIterator",
        ty_generic("Iterator", vec![ty_named("Int")]),
        vec![iterator_next(ty_named("Int"))],
    );
    let Decl::Struct(iterator_declaration) = &mut iterator else {
        unreachable!()
    };
    iterator_declaration.annotations.push(annotation());

    let mut source_iterator = operator_method(
        false,
        ty_named("NoGcIterator"),
        struct_init("NoGcIterator", Vec::new()),
    );
    source_iterator.annotations.push(annotation());
    let mut source = struct_decl_methods("NoGcSource", Vec::new(), vec![source_iterator]);
    let Decl::Struct(source_declaration) = &mut source else {
        unreachable!()
    };
    source_declaration.annotations.push(annotation());

    let mut consume = fun_sig(
        "consume",
        Vec::new(),
        vec![("source", ty_named("NoGcSource"))],
        None,
        vec![for_stmt(pat_bind("item"), var("source"), Vec::new())],
    );
    let Decl::Function(consume_function) = &mut consume else {
        unreachable!()
    };
    consume_function.annotations.push(annotation());

    let errors = lower_user(file(vec![
        iterator,
        source,
        consume,
        fun("main", Vec::new()),
    ]))
    .expect_err("canonical Iterator.next is a managed call inside @NoGC code");
    assert!(errors.iter().any(|error| {
        error.message == "`@NoGC` code may not call managed function `Iterator.next`"
    }));
}

#[test]
fn iteration_default_regions_keep_structural_suspend_ownership() {
    let suspend_iterator = with_suspend(operator_method(
        false,
        ty_named("DefaultIterator"),
        call("DefaultIterator", Vec::new()),
    ));
    let suspend_source = class_decl(
        ast::ClassModifier::Final,
        "DefaultSource",
        Vec::new(),
        None,
        Vec::new(),
        vec![suspend_iterator],
    );
    let default_value = Expr::If(Box::new(ast::If {
        cond: bool_lit(true),
        then_block: block(vec![
            for_stmt(
                pat_bind("item"),
                call("DefaultSource", Vec::new()),
                Vec::new(),
            ),
            stmt(int_lit(1)),
        ]),
        else_block: Some(block(vec![stmt(int_lit(2))])),
        span: sp(),
    }));
    let mut choose_declaration = fun_sig(
        "choose",
        Vec::new(),
        vec![("value", ty_named("Int"))],
        None,
        Vec::new(),
    );
    let Decl::Function(choose) = &mut choose_declaration else {
        unreachable!()
    };
    choose.is_suspend = true;
    choose.params[0].syntax = ast::ParameterSyntax::Default {
        expression: default_value,
        equals_span: sp(),
    };
    let executable = lower_user(file(vec![
        iterator_class("DefaultIterator", ty_named("Int")),
        suspend_source,
        choose_declaration,
        fun("main", Vec::new()),
    ]))
    .expect("a suspend declaration owns a default region with suspend iteration");
    let export = executable.into_module();
    let expression = export
        .source_parameter_interfaces
        .iter()
        .flat_map(|interface| &interface.parameters)
        .find_map(|parameter| match parameter.calling {
            hir::ExportParameterCalling::Default { source, .. } => {
                Some(export.export_default_sources[source].declared().unwrap().0)
            }
            _ => None,
        })
        .expect("choose has one exported default");
    assert!(export.export_default_exprs[expression].allows_suspend);
}
