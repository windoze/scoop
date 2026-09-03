use super::*;

fn class_with_interface(
    name: &str,
    base: Option<&str>,
    interface: TypeRef,
    methods: Vec<FunctionDecl>,
) -> Decl {
    let mut decl = class_decl(
        ast::ClassModifier::Final,
        name,
        Vec::new(),
        base.map(|base| (base, Vec::new())),
        Vec::new(),
        methods,
    );
    let Decl::Class(class) = &mut decl else {
        unreachable!()
    };
    class.interfaces.push(interface);
    decl
}

#[test]
fn generic_interfaces_apply_variance_and_instantiate_methods() {
    let producer = generic_interface_decl(
        "Producer",
        vec![(ast::Variance::Out, "T")],
        vec![bodyless_method(false, "get", vec![], Some(ty_named("T")))],
    );
    let sink = generic_interface_decl(
        "Sink",
        vec![(ast::Variance::In, "T")],
        vec![bodyless_method(
            false,
            "put",
            vec![("value", ty_named("T"))],
            None,
        )],
    );
    let animal = class_decl(
        ast::ClassModifier::Open,
        "Animal",
        Vec::new(),
        None,
        Vec::new(),
        Vec::new(),
    );
    let dog = class_decl(
        ast::ClassModifier::Final,
        "Dog",
        Vec::new(),
        Some(("Animal", Vec::new())),
        Vec::new(),
        Vec::new(),
    );
    let dog_producer = class_with_interface(
        "DogProducer",
        None,
        ty_generic("Producer", vec![ty_named("Dog")]),
        vec![override_method_expr(
            "get",
            Vec::new(),
            Some(ty_named("Dog")),
            call("Dog", Vec::new()),
        )],
    );
    let animal_sink = class_with_interface(
        "AnimalSink",
        None,
        ty_generic("Sink", vec![ty_named("Animal")]),
        vec![method_full(
            true,
            false,
            "put",
            vec![("value", ty_named("Animal"))],
            None,
            FunctionBody::Block(block(Vec::new())),
        )],
    );
    let module = lower_user(file(vec![
        producer,
        sink,
        animal,
        dog,
        dog_producer,
        animal_sink,
        fun_expr(
            "widen",
            Vec::new(),
            vec![("p", ty_generic("Producer", vec![ty_named("Dog")]))],
            Some(ty_generic("Producer", vec![ty_named("Animal")])),
            var("p"),
        ),
        fun_expr(
            "narrow",
            Vec::new(),
            vec![("s", ty_generic("Sink", vec![ty_named("Animal")]))],
            Some(ty_generic("Sink", vec![ty_named("Dog")])),
            var("s"),
        ),
        fun_expr(
            "read",
            Vec::new(),
            vec![("p", ty_generic("Producer", vec![ty_named("Dog")]))],
            Some(ty_named("Dog")),
            method_call(var("p"), "get", Vec::new()),
        ),
        fun("main", Vec::new()),
    ]))
    .expect("valid generic interface applications");

    let applications: Vec<_> = module
        .types
        .iter()
        .filter(|(_, ty)| {
            matches!(ty, hir::Type::Interface(application)
                if !module.interface_applications[*application].arguments.is_empty())
        })
        .collect();
    assert!(applications.len() >= 4);
}

#[test]
fn generic_call_infers_through_a_concrete_interface_implementation() {
    let producer = generic_interface_decl(
        "Producer",
        vec![(ast::Variance::Out, "T")],
        vec![bodyless_method(false, "get", vec![], Some(ty_named("T")))],
    );
    let int_producer = class_with_interface(
        "IntProducer",
        None,
        ty_generic("Producer", vec![ty_named("Int")]),
        vec![override_method_expr(
            "get",
            Vec::new(),
            Some(ty_named("Int")),
            int_lit(42),
        )],
    );
    let infer = fun_sig(
        "infer",
        vec!["T"],
        vec![("producer", ty_generic("Producer", vec![ty_named("T")]))],
        Some(ty_named("T")),
        vec![ret(Some(method_call(var("producer"), "get", Vec::new())))],
    );
    let module = lower_user(file(vec![
        producer,
        int_producer,
        infer,
        fun(
            "main",
            vec![stmt(call("infer", vec![call("IntProducer", Vec::new())]))],
        ),
    ]))
    .expect("the implemented interface application should constrain T");

    let infer = module
        .functions
        .iter()
        .find_map(|(id, function)| (function.name == "infer").then_some(id))
        .expect("infer function");
    let generic = module
        .generic_functions
        .iter()
        .find_map(|(id, entity)| (entity.function == infer).then_some(id))
        .expect("infer generic entity");
    let request = module
        .instantiations
        .iter()
        .find_map(|(_, request)| (request.generic == generic).then_some(request))
        .expect("inferred invocation");
    assert_eq!(request.type_args, [module.int]);
}

#[test]
fn invariant_interface_does_not_convert_between_arguments() {
    let invariant = generic_interface_decl(
        "Cell",
        vec![(ast::Variance::Invariant, "T")],
        vec![bodyless_method(false, "get", vec![], Some(ty_named("T")))],
    );
    let errors = lower_user(file(vec![
        invariant,
        class_decl(
            ast::ClassModifier::Open,
            "Animal",
            Vec::new(),
            None,
            Vec::new(),
            Vec::new(),
        ),
        class_decl(
            ast::ClassModifier::Final,
            "Dog",
            Vec::new(),
            Some(("Animal", Vec::new())),
            Vec::new(),
            Vec::new(),
        ),
        fun_expr(
            "bad",
            Vec::new(),
            vec![("c", ty_generic("Cell", vec![ty_named("Dog")]))],
            Some(ty_generic("Cell", vec![ty_named("Animal")])),
            var("c"),
        ),
        fun("main", Vec::new()),
    ]))
    .expect_err("invariant arguments must not convert");
    assert!(
        errors[0]
            .message
            .contains("body of `bad` must be of type Cell<Animal>, found Cell<Dog>")
    );
}

#[test]
fn interface_variance_positions_are_checked_recursively() {
    let errors = lower_user(file(vec![
        generic_interface_decl(
            "Consumer",
            vec![(ast::Variance::In, "T")],
            vec![bodyless_method(
                false,
                "put",
                vec![("value", ty_named("T"))],
                None,
            )],
        ),
        generic_interface_decl(
            "BadProducer",
            vec![(ast::Variance::Out, "T")],
            vec![bodyless_method(
                false,
                "put",
                vec![("value", ty_named("T"))],
                None,
            )],
        ),
        generic_interface_decl(
            "BadNested",
            vec![(ast::Variance::Out, "T")],
            vec![bodyless_method(
                false,
                "make",
                Vec::new(),
                Some(ty_generic("Consumer", vec![ty_named("T")])),
            )],
        ),
        fun("main", Vec::new()),
    ]))
    .expect_err("invalid variance positions must fail");
    assert_eq!(errors.len(), 2);
    assert!(
        errors[0]
            .message
            .contains("covariant type parameter `T` occurs in contravariant position")
    );
    assert!(
        errors[1]
            .message
            .contains("covariant type parameter `T` occurs in contravariant position")
    );
}
