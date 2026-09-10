use std::collections::HashSet;

use scoop_hir::concrete;
use scoop_identity::LocalValueSelector;

use super::*;

fn concrete_function<'module>(
    module: &'module concrete::Module,
    name: &str,
) -> (concrete::FunctionId, &'module concrete::Function) {
    module
        .functions
        .iter()
        .find(|(_, function)| function.name == name)
        .unwrap_or_else(|| panic!("missing concrete function `{name}`"))
}

fn concrete_body(function: &concrete::Function) -> &concrete::Body {
    let concrete::FunctionKind::User(body) = &function.kind else {
        panic!("expected a concrete user body")
    };
    body
}

fn concrete_local_by_name<'body>(
    body: &'body concrete::Body,
    name: &str,
) -> (concrete::LocalId, &'body concrete::Local) {
    body.locals
        .iter()
        .find(|(_, local)| local.name == name)
        .unwrap_or_else(|| panic!("missing concrete local `{name}`"))
}

#[test]
fn generic_applications_give_the_same_template_local_distinct_persistent_values() {
    let output = lower_user_output(file(vec![
        fun_sig(
            "copy",
            vec!["T"],
            vec![("value", ty_named("T"))],
            Some(ty_named("T")),
            vec![val("saved", var("value")), ret(Some(var("saved")))],
        ),
        fun(
            "main",
            vec![
                stmt(call("copy", vec![int_lit(1)])),
                stmt(call("copy", vec![str_lit("value")])),
            ],
        ),
    ]))
    .expect("one generic body materializes in two exact contexts");
    let module = &output.local;
    let instances = module
        .functions
        .iter()
        .filter(|(_, function)| function.name == "copy")
        .collect::<Vec<_>>();
    assert_eq!(instances.len(), 2);

    let mut parameter_ids = HashSet::new();
    let mut saved_ids = HashSet::new();
    for (function_id, function) in instances {
        let body = concrete_body(function);
        let parameter = function.params[0].local;
        let parameter_identity = module
            .local_value_identities
            .function_local(function_id, parameter);
        assert_eq!(parameter_identity.key().owner(), function.materialization);
        assert!(matches!(
            parameter_identity.key().selector(),
            LocalValueSelector::Parameter {
                declaration_index: 0
            }
        ));
        parameter_ids.insert(parameter_identity.id());

        let (saved, _) = concrete_local_by_name(body, "saved");
        let saved_identity = module
            .local_value_identities
            .function_local(function_id, saved);
        assert_eq!(saved_identity.key().owner(), function.materialization);
        assert!(matches!(
            saved_identity.key().selector(),
            LocalValueSelector::LocalDeclaration { .. }
        ));
        saved_ids.insert(saved_identity.id());
    }
    assert_eq!(parameter_ids.len(), 2);
    assert_eq!(saved_ids.len(), 2);
}

#[test]
fn local_function_capture_parameter_reuses_the_captured_value_identity() {
    let output = lower_user_output(file(vec![fun(
        "main",
        vec![
            val("base", int_lit(2)),
            local_fun_sig(
                "add",
                Vec::new(),
                vec![("value", ty_named("Int"))],
                Some(ty_named("Int")),
                vec![ret(Some(binary(
                    ast::BinOp::Add,
                    var("base"),
                    var("value"),
                )))],
            ),
            stmt(call("add", vec![int_lit(40)])),
        ],
    )]))
    .expect("a captured local function has one lifted capture parameter");
    let module = &output.local;
    let (main_id, main) = concrete_function(module, "main");
    let (base, _) = concrete_local_by_name(concrete_body(main), "base");
    let base_identity = module.local_value_identities.function_local(main_id, base);

    let (_, local_function) = module
        .local_functions
        .iter()
        .next()
        .expect("one concrete local function");
    let lifted = &module.functions[local_function.function];
    let capture_parameter = lifted.params[0].local;
    let capture_identity = module
        .local_value_identities
        .function_local(local_function.function, capture_parameter);

    assert_eq!(capture_identity.id(), base_identity.id());
    assert_eq!(capture_identity.key().owner(), main.materialization);
    assert!(matches!(
        capture_identity.key().selector(),
        LocalValueSelector::LocalDeclaration { .. }
    ));
    assert_eq!(
        lifted.materialization.context(),
        main.materialization.context(),
        "lexical capture aliases must remain in the enclosing materialization context"
    );
}

#[test]
fn constructor_receivers_and_parameters_have_typed_persistent_values() {
    let output = lower_user_output(file(vec![
        class_decl(
            ast::ClassModifier::Final,
            "Box",
            vec![(false, "value", ty_named("Int"))],
            None,
            Vec::new(),
            Vec::new(),
        ),
        struct_decl("Point", vec![("x", ty_named("Int"))]),
        fun(
            "main",
            vec![
                val("box", call("Box", vec![int_lit(1)])),
                val("point", call("Point", vec![int_lit(2)])),
            ],
        ),
    ]))
    .expect("class and struct constructors materialize together");
    let module = &output.local;

    let class = module
        .classes
        .iter()
        .find_map(|(id, class)| (class.name == "Box").then_some(id))
        .expect("Box class");
    let (class_constructor, constructor) = module
        .class_constructors
        .iter()
        .find(|(_, constructor)| constructor.class == class)
        .expect("Box constructor");
    let receiver = module
        .local_value_identities
        .class_receiver(class_constructor);
    let parameter = module
        .local_value_identities
        .class_parameter(class_constructor, 0);
    assert_eq!(receiver.key().owner(), constructor.materialization);
    assert!(matches!(
        receiver.key().selector(),
        LocalValueSelector::This
    ));
    assert_eq!(parameter.key().owner(), constructor.materialization);
    assert!(matches!(
        parameter.key().selector(),
        LocalValueSelector::Parameter {
            declaration_index: 0
        }
    ));

    let structure = module
        .structs
        .iter()
        .find_map(|(id, structure)| (structure.name == "Point").then_some(id))
        .expect("Point struct");
    let (struct_constructor, constructor) = module
        .struct_constructors
        .iter()
        .find(|(_, constructor)| constructor.structure == structure)
        .expect("Point constructor");
    let parameter = module
        .local_value_identities
        .struct_parameter(struct_constructor, 0);
    assert_eq!(parameter.key().owner(), constructor.materialization);
    assert!(matches!(
        parameter.key().selector(),
        LocalValueSelector::Parameter {
            declaration_index: 0
        }
    ));
    assert!(
        module
            .local_value_identities
            .struct_receiver(struct_constructor)
            .is_none(),
        "a primary struct constructor has no receiver value"
    );
}
