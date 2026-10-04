use super::*;
mod objects;
mod private_accessors;
mod property_slots;
mod protected_scopes;
mod support;

fn visibility(value: ast::DeclaredVisibility) -> ast::VisibilitySyntax {
    ast::VisibilitySyntax::Explicit {
        visibility: value,
        span: sp(),
    }
}

fn set_decl_visibility(declaration: &mut Decl, value: ast::DeclaredVisibility) {
    let visibility = visibility(value);
    match declaration {
        Decl::Global(declaration) => declaration.visibility = visibility,
        Decl::TypeAlias(declaration) => declaration.visibility = visibility,
        Decl::Struct(declaration) => declaration.visibility = visibility,
        Decl::Enum(declaration) => declaration.visibility = visibility,
        Decl::Class(declaration) => declaration.visibility = visibility,
        Decl::Interface(declaration) => declaration.visibility = visibility,
        Decl::Object(declaration) => declaration.visibility = visibility,
        Decl::Function(declaration) => declaration.visibility = visibility,
    }
}

fn set_method_visibility(method: &mut ast::FunctionDecl, value: ast::DeclaredVisibility) {
    method.visibility = visibility(value);
}

fn global(name: &str, value: i64, visibility_value: ast::DeclaredVisibility) -> Decl {
    Decl::Global(ast::PropertyDecl {
        context_parameters: Vec::new(),
        annotations: vec![ast::Annotation {
            name: ident("Global"),
            args: Vec::new(),
            span: sp(),
        }],
        visibility: visibility(visibility_value),
        modifier: ast::MethodModifier::Final,
        is_override: false,
        mutable: true,
        receiver_ty: None,
        type_params: Vec::new(),
        where_clause: None,
        name: ident(name),
        ty: ty_named("Int"),
        body: ast::PropertyBodySyntax::Initializer {
            expression: Box::new(int_lit(value)),
            accessors: ast::AccessorSyntax::default(),
        },
        span: sp(),
    })
}

fn with_default(mut declaration: Decl, parameter: usize, expression: Expr) -> Decl {
    let Decl::Function(function) = &mut declaration else {
        unreachable!("test helper accepts a function")
    };
    function.params[parameter].syntax = ast::ParameterSyntax::Default {
        expression,
        equals_span: sp(),
    };
    declaration
}

#[test]
fn omission_normalizes_to_internal_and_export_surface_is_explicit() {
    let mut exported = fun("exported", Vec::new());
    set_decl_visibility(&mut exported, ast::DeclaredVisibility::Public);
    let mut exported_global = global("answer", 42, ast::DeclaredVisibility::Public);
    set_decl_visibility(&mut exported_global, ast::DeclaredVisibility::Public);

    let mut api = class_decl(
        ast::ClassModifier::Final,
        "Api",
        vec![(false, "value", ty_named("Int"))],
        None,
        Vec::new(),
        vec![
            method("hidden", Vec::new(), None, Vec::new()),
            method("visible", Vec::new(), None, Vec::new()),
        ],
    );
    let Decl::Class(api_source) = &mut api else {
        unreachable!()
    };
    api_source.visibility = visibility(ast::DeclaredVisibility::Public);
    let ast::ClassConstructorDecl::Declared(constructor) = &mut api_source.constructor else {
        unreachable!()
    };
    constructor.parameters[0].member_visibility = Some(visibility(ast::DeclaredVisibility::Public));
    let ast::ClassMember::Function(visible) = &mut api_source.members[1] else {
        unreachable!()
    };
    set_method_visibility(visible, ast::DeclaredVisibility::Public);

    let mut constructible = class_decl(
        ast::ClassModifier::Final,
        "Constructible",
        Vec::new(),
        None,
        Vec::new(),
        Vec::new(),
    );
    let Decl::Class(constructible_source) = &mut constructible else {
        unreachable!()
    };
    constructible_source.visibility = visibility(ast::DeclaredVisibility::Public);
    let ast::ClassConstructorDecl::Declared(constructor) = &mut constructible_source.constructor
    else {
        unreachable!()
    };
    constructor.visibility = visibility(ast::DeclaredVisibility::Public);

    let module = lower_user(file(vec![
        api,
        constructible,
        exported,
        exported_global,
        fun("main", Vec::new()),
    ]))
    .expect("explicit public declarations and an internal main must lower");

    let main = module.entry();
    assert_eq!(
        module.functions[main].access.declared,
        hir::DeclaredVisibility::Internal
    );
    assert_eq!(
        module.functions[main].access.lookup.0.constraints(),
        &[hir::AccessConstraint::Cone(
            module.source_files[1].identity.cone()
        )]
    );
    assert!(!module.public_surface.functions.contains(&main));

    let (exported, _) = module
        .functions
        .iter()
        .find(|(_, function)| function.name == "exported")
        .expect("exported function");
    assert!(module.public_surface.functions.contains(&exported));
    assert!(module.functions[exported].access.lookup.0.is_universal());

    let (answer, _) = module
        .globals
        .iter()
        .find(|(_, global)| module.properties[global.property].name == "answer")
        .expect("exported property");
    let answer = module.globals[answer].property;
    assert!(module.public_surface.properties.contains(&answer));

    let (api, api_decl) = module
        .classes
        .iter()
        .find(|(_, class)| class.name == "Api")
        .expect("Api class");
    assert!(module.public_surface.classes.contains(&api));
    let api_constructor = api_decl.constructors[0];
    assert_eq!(
        module.class_constructors[api_constructor].access.declared,
        hir::DeclaredVisibility::Internal
    );
    assert!(
        !module
            .public_surface
            .class_constructors
            .contains(&api_constructor)
    );
    let field = api_decl.fields[0];
    let property = module.class_fields[field].property;
    assert!(module.public_surface.properties.contains(&property));
    let hidden = api_decl
        .methods
        .iter()
        .copied()
        .find(|&method| module.functions[method].name.ends_with(".hidden"))
        .expect("hidden method");
    let visible = api_decl
        .methods
        .iter()
        .copied()
        .find(|&method| module.functions[method].name.ends_with(".visible"))
        .expect("visible method");
    assert_eq!(
        module.functions[hidden].access.declared,
        hir::DeclaredVisibility::Internal
    );
    assert!(!module.public_surface.functions.contains(&hidden));
    assert!(module.public_surface.functions.contains(&visible));

    let (_, constructible_decl) = module
        .classes
        .iter()
        .find(|(_, class)| class.name == "Constructible")
        .expect("Constructible class");
    assert!(
        module
            .public_surface
            .class_constructors
            .contains(&constructible_decl.constructors[0])
    );
}

#[test]
fn illegal_visibility_positions_and_hidden_obligations_are_diagnosed() {
    let mut top = fun("top", Vec::new());
    set_decl_visibility(&mut top, ast::DeclaredVisibility::Protected);

    let mut structure = struct_decl_methods(
        "Value",
        Vec::new(),
        vec![method("member", Vec::new(), None, Vec::new())],
    );
    let Decl::Struct(structure_source) = &mut structure else {
        unreachable!()
    };
    let ast::StructMember::Function(member) = &mut structure_source.members[0] else {
        unreachable!()
    };
    set_method_visibility(member, ast::DeclaredVisibility::Protected);

    let mut interface = interface_decl(
        "Contract",
        vec![bodyless_method(false, "required", Vec::new(), None)],
    );
    set_decl_visibility(&mut interface, ast::DeclaredVisibility::Public);

    let mut abstract_class = class_decl(
        ast::ClassModifier::Abstract,
        "OpenApi",
        Vec::new(),
        None,
        Vec::new(),
        vec![bodyless_method(true, "required", Vec::new(), None)],
    );
    set_decl_visibility(&mut abstract_class, ast::DeclaredVisibility::Public);

    let errors = lower_user(file(vec![
        top,
        structure,
        interface,
        abstract_class,
        fun("main", Vec::new()),
    ]))
    .expect_err("invalid access declarations must fail");
    assert!(errors.iter().all(|diagnostic| diagnostic.file == 1));
    let messages = errors
        .into_iter()
        .map(|diagnostic| diagnostic.message)
        .collect::<Vec<_>>();
    assert!(messages.contains(&"top-level function cannot be protected".to_string()));
    assert!(messages.contains(&"method on struct `Value` cannot be protected".to_string()));
    assert!(messages.contains(
        &"member `required` of public interface `Contract` must be explicitly public".to_string()
    ));
    assert!(messages.contains(
        &"abstract method `required` is not visible throughout the inheritance domain of class `OpenApi`"
            .to_string()
    ));
}

#[test]
fn protected_explicit_receiver_uses_the_current_subclass_static_type() {
    let mut secret = method("secret", Vec::new(), None, Vec::new());
    set_method_visibility(&mut secret, ast::DeclaredVisibility::Protected);
    let mut base = class_decl(
        ast::ClassModifier::Open,
        "Base",
        Vec::new(),
        None,
        Vec::new(),
        vec![secret],
    );
    set_decl_visibility(&mut base, ast::DeclaredVisibility::Public);
    let derived = class_decl(
        ast::ClassModifier::Final,
        "Derived",
        Vec::new(),
        Some(("Base", Vec::new())),
        Vec::new(),
        vec![method_expr(
            "probe",
            vec![("other", ty_named("Base"))],
            None,
            method_call(var("other"), "secret", Vec::new()),
        )],
    );
    let errors = lower_user(file(vec![base, derived, fun("main", Vec::new())]))
        .expect_err("a Base-typed explicit receiver cannot expose protected state");
    assert!(errors.iter().any(|diagnostic| {
        diagnostic.message
            == "protected method `secret` cannot be accessed through receiver of static type `Base`; receiver must be `Derived` or one of its subclasses"
    }));
}

#[test]
fn public_override_in_internal_owner_preserves_public_slot_contract() {
    let mut base_method = method("run", Vec::new(), None, Vec::new());
    base_method.modifier = ast::MethodModifier::Open;
    set_method_visibility(&mut base_method, ast::DeclaredVisibility::Public);
    let mut base = class_decl(
        ast::ClassModifier::Open,
        "Base",
        Vec::new(),
        None,
        Vec::new(),
        vec![base_method],
    );
    set_decl_visibility(&mut base, ast::DeclaredVisibility::Public);

    let mut override_method = override_method_expr("run", Vec::new(), None, unit_lit());
    set_method_visibility(&mut override_method, ast::DeclaredVisibility::Public);
    let derived = class_decl(
        ast::ClassModifier::Final,
        "Derived",
        Vec::new(),
        Some(("Base", Vec::new())),
        Vec::new(),
        vec![override_method],
    );
    let module = lower_core_with_additional_declarations(vec![base, derived]);
    let (derived_method, function) = module
        .functions
        .iter()
        .find(|(_, function)| function.name == "Derived.run")
        .expect("derived override");
    assert!(!function.access.lookup.0.is_universal());
    assert!(
        function
            .access
            .slot
            .as_ref()
            .is_some_and(|slot| slot.0.is_universal())
    );
    assert!(!module.public_surface.functions.contains(&derived_method));
    let callables = hir::CanonicalCallableInterfacesV1::from_export_hir(&module).unwrap();
    let hir::HirFunctionIdentity::Source(hir::HirSourceFunctionIdentity::Plain(identity)) =
        &module.function_identities[derived_method]
    else {
        panic!("the override has a source function identity")
    };
    assert!(
        callables
            .get(scoop_identity::CallableTemplateOrigin::Function(
                identity.id()
            ))
            .is_none()
    );
}

#[test]
fn override_cannot_narrow_a_public_slot() {
    let mut base_method = method("run", Vec::new(), None, Vec::new());
    base_method.modifier = ast::MethodModifier::Open;
    set_method_visibility(&mut base_method, ast::DeclaredVisibility::Public);
    let mut base = class_decl(
        ast::ClassModifier::Open,
        "Base",
        Vec::new(),
        None,
        Vec::new(),
        vec![base_method],
    );
    set_decl_visibility(&mut base, ast::DeclaredVisibility::Public);
    let derived = class_decl(
        ast::ClassModifier::Final,
        "Derived",
        Vec::new(),
        Some(("Base", Vec::new())),
        Vec::new(),
        vec![override_method_expr("run", Vec::new(), None, unit_lit())],
    );
    let errors = lower_user(file(vec![base, derived, fun("main", Vec::new())]))
        .expect_err("omitted override visibility is internal and cannot cover public");
    assert!(errors.iter().any(|diagnostic| {
        diagnostic.message == "visibility of `run` does not cover inherited slot `Base.run`"
    }));
}

#[test]
fn signature_and_default_dependencies_must_cover_the_full_call_domain() {
    let mut hidden = class_decl(
        ast::ClassModifier::Final,
        "Hidden",
        Vec::new(),
        None,
        Vec::new(),
        Vec::new(),
    );
    set_decl_visibility(&mut hidden, ast::DeclaredVisibility::Private);
    let mut leak = fun_expr(
        "leak",
        Vec::new(),
        Vec::new(),
        Some(ty_named("Hidden")),
        struct_init("Hidden", Vec::new()),
    );
    set_decl_visibility(&mut leak, ast::DeclaredVisibility::Public);

    let helper = fun_expr(
        "helper",
        Vec::new(),
        Vec::new(),
        Some(ty_named("Int")),
        int_lit(1),
    );
    let mut consume = with_default(
        fun_expr(
            "consume",
            Vec::new(),
            vec![("value", ty_named("Int"))],
            Some(ty_named("Int")),
            var("value"),
        ),
        0,
        call("helper", Vec::new()),
    );
    set_decl_visibility(&mut consume, ast::DeclaredVisibility::Public);

    let errors = lower_user(file(vec![
        hidden,
        leak,
        helper,
        consume,
        fun("main", Vec::new()),
    ]))
    .expect_err("public signatures and defaults cannot leak narrower declarations");
    assert!(errors.iter().all(|diagnostic| diagnostic.file == 1));
    let messages = errors
        .into_iter()
        .map(|diagnostic| diagnostic.message)
        .collect::<Vec<_>>();
    assert!(messages.contains(
        &"signature of function `leak` exposes type `Hidden` outside its access domain".to_string()
    ));
    assert!(
        messages.contains(
            &"default expression references a callable outside the callable's complete call domain"
                .to_string()
        )
    );
}

#[test]
fn file_private_functions_and_properties_have_distinct_file_namespaces() {
    fn source(wrapper: &str, value: i64) -> SourceFile {
        let mut private_function = fun_expr(
            "same",
            Vec::new(),
            Vec::new(),
            Some(ty_named("Int")),
            int_lit(value),
        );
        set_decl_visibility(&mut private_function, ast::DeclaredVisibility::Private);
        let mut public_wrapper = fun_expr(
            wrapper,
            Vec::new(),
            Vec::new(),
            Some(ty_named("Int")),
            call("same", Vec::new()),
        );
        set_decl_visibility(&mut public_wrapper, ast::DeclaredVisibility::Public);
        file(vec![
            global("privateValue", value, ast::DeclaredVisibility::Private),
            private_function,
            public_wrapper,
        ])
    }

    let module = lower(&[
        core_file(),
        source("one", 1),
        source("two", 2),
        file(vec![fun(
            "main",
            vec![stmt(call("one", Vec::new())), stmt(call("two", Vec::new()))],
        )]),
    ])
    .map(|output| output.export)
    .expect("same-name private declarations in different files must coexist");
    assert_eq!(
        module
            .functions
            .iter()
            .filter(|(_, function)| function.name == "same")
            .count(),
        2
    );
    assert_eq!(
        module
            .globals
            .iter()
            .filter(|(_, global)| module.properties[global.property].name == "privateValue")
            .count(),
        2
    );
    assert!(
        module
            .globals
            .iter()
            .filter(|(_, global)| module.properties[global.property].name == "privateValue")
            .all(|(_, global)| !module.public_surface.properties.contains(&global.property))
    );
}

#[test]
fn internal_declaration_is_not_visible_across_cones() {
    let library = file(vec![fun("secret", Vec::new())]);
    let user = file(vec![fun("main", vec![stmt(call("secret", Vec::new()))])]);
    let errors = lower(&[core_file(), library, user])
        .expect_err("internal core declaration must not cross the Cone boundary");
    assert!(
        errors
            .iter()
            .any(|diagnostic| { diagnostic.message == "function `secret` is not accessible here" })
    );
}
