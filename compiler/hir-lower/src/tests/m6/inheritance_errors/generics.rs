use super::*;

#[test]
fn final_generic_member_functions_are_resolved() {
    let mut generic = method_expr(
        "id",
        vec![("value", ty_named("T"))],
        Some(ty_named("T")),
        var("value"),
    );
    generic.type_params = vec![type_param("T")];
    let file = file(vec![
        class_decl(Final, "C", vec![], None, vec![], vec![generic]),
        fun(
            "main",
            vec![stmt(method_call(
                call("C", vec![]),
                "id",
                vec![str_lit("ok")],
            ))],
        ),
    ]);
    let module = lower_user(file).expect("a final generic method must lower");
    let method = find_fn(&module, "C.id");
    assert_eq!(module.functions[method].type_param_count(), 1);
    assert_eq!(module.functions[method].type_params()[0].name, "T");
    let hir::FunctionGenericity::GenericMethod {
        definition: generic,
        ..
    } = module.functions[method].genericity
    else {
        panic!("C.id generic entity")
    };
    let (_, request) = module
        .generic_method_applications
        .iter()
        .find(|(_, request)| request.method == generic)
        .expect("the call requests an instance");
    assert_eq!(request.method_arguments.to_vec(), [module.string]);
    assert!(matches!(request.owner, hir::GenericMethodOwner::Class(_)));
}
