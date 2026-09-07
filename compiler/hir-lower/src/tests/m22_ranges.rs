use super::*;

#[derive(Clone, Copy)]
struct IntegerRangeCase {
    key: &'static str,
    kind: hir::IntegerKind,
    range: &'static str,
    element: hir::IntegerKind,
}

const INTEGER_RANGE_CASES: [IntegerRangeCase; 8] = [
    IntegerRangeCase {
        key: "int8",
        kind: hir::IntegerKind::SIGNED_8,
        range: "IntRange",
        element: hir::IntegerKind::SIGNED_32,
    },
    IntegerRangeCase {
        key: "int16",
        kind: hir::IntegerKind::SIGNED_16,
        range: "IntRange",
        element: hir::IntegerKind::SIGNED_32,
    },
    IntegerRangeCase {
        key: "int",
        kind: hir::IntegerKind::SIGNED_32,
        range: "IntRange",
        element: hir::IntegerKind::SIGNED_32,
    },
    IntegerRangeCase {
        key: "long",
        kind: hir::IntegerKind::SIGNED_64,
        range: "LongRange",
        element: hir::IntegerKind::SIGNED_64,
    },
    IntegerRangeCase {
        key: "uint8",
        kind: hir::IntegerKind::UNSIGNED_8,
        range: "UIntRange",
        element: hir::IntegerKind::UNSIGNED_32,
    },
    IntegerRangeCase {
        key: "uint16",
        kind: hir::IntegerKind::UNSIGNED_16,
        range: "UIntRange",
        element: hir::IntegerKind::UNSIGNED_32,
    },
    IntegerRangeCase {
        key: "uint",
        kind: hir::IntegerKind::UNSIGNED_32,
        range: "UIntRange",
        element: hir::IntegerKind::UNSIGNED_32,
    },
    IntegerRangeCase {
        key: "ulong",
        kind: hir::IntegerKind::UNSIGNED_64,
        range: "ULongRange",
        element: hir::IntegerKind::UNSIGNED_64,
    },
];

#[derive(Clone, Copy)]
struct RangeCase {
    key: &'static str,
    range: &'static str,
    element: hir::IntegerKind,
    source_owner: hir::IntegerKind,
}

const RANGE_CASES: [RangeCase; 4] = [
    RangeCase {
        key: "int",
        range: "IntRange",
        element: hir::IntegerKind::SIGNED_32,
        source_owner: hir::IntegerKind::SIGNED_8,
    },
    RangeCase {
        key: "long",
        range: "LongRange",
        element: hir::IntegerKind::SIGNED_64,
        source_owner: hir::IntegerKind::SIGNED_64,
    },
    RangeCase {
        key: "uint",
        range: "UIntRange",
        element: hir::IntegerKind::UNSIGNED_32,
        source_owner: hir::IntegerKind::UNSIGNED_8,
    },
    RangeCase {
        key: "ulong",
        range: "ULongRange",
        element: hir::IntegerKind::UNSIGNED_64,
        source_owner: hir::IntegerKind::UNSIGNED_64,
    },
];

#[derive(Clone, Copy)]
enum RangeRole {
    RangeTo,
    RangeUntil,
    Until,
    DownTo,
}

impl RangeRole {
    const ALL: [Self; 4] = [Self::RangeTo, Self::RangeUntil, Self::Until, Self::DownTo];

    const fn name(self) -> &'static str {
        match self {
            Self::RangeTo => "rangeTo",
            Self::RangeUntil => "rangeUntil",
            Self::Until => "until",
            Self::DownTo => "downTo",
        }
    }

    const fn operator(self) -> Option<hir::OperatorKind> {
        match self {
            Self::RangeTo => Some(hir::OperatorKind::RangeTo),
            Self::RangeUntil => Some(hir::OperatorKind::RangeUntil),
            Self::Until | Self::DownTo => None,
        }
    }

    const fn is_infix(self) -> bool {
        matches!(self, Self::Until | Self::DownTo)
    }
}

fn explicit_visibility(visibility: ast::DeclaredVisibility) -> ast::VisibilitySyntax {
    ast::VisibilitySyntax::Explicit {
        visibility,
        span: sp(),
    }
}

fn infix(lhs: Expr, name: &str, rhs: Expr) -> Expr {
    Expr::InfixCall {
        lhs: Box::new(lhs),
        target: ast::InfixTarget::Named(ident(name)),
        rhs: Box::new(rhs),
        span: sp(),
    }
}

fn role_expression(role: RangeRole, lhs: Expr, rhs: Expr) -> Expr {
    match role {
        RangeRole::RangeTo => binary(BinOp::RangeTo, lhs, rhs),
        RangeRole::RangeUntil => binary(BinOp::RangeUntil, lhs, rhs),
        RangeRole::Until | RangeRole::DownTo => infix(lhs, role.name(), rhs),
    }
}

fn literal(kind: hir::IntegerKind, magnitude: u64) -> Expr {
    let suffix = match kind {
        hir::IntegerKind::SIGNED_64 => ast::IntegerSuffix::Long,
        hir::IntegerKind::UNSIGNED_64 => ast::IntegerSuffix::UnsignedLong,
        kind if kind.signedness() == hir::IntegerSignedness::Unsigned => {
            ast::IntegerSuffix::Unsigned
        }
        _ => ast::IntegerSuffix::None,
    };
    Expr::IntLiteral(ast::IntegerLiteralSyntax {
        magnitude,
        radix: ast::IntegerRadix::Decimal,
        suffix,
        span: sp(),
    })
}

fn for_stmt(pattern: ast::Pattern, iterable: Expr, body: Vec<Statement>) -> Statement {
    Statement {
        kind: StatementKind::For(ast::For {
            pattern,
            iterable,
            body: block(body),
            span: sp(),
        }),
        span: sp(),
    }
}

fn illegal_argument_exception() -> Decl {
    let mut declaration = class_decl(
        ast::ClassModifier::Final,
        "IllegalArgumentException",
        vec![(false, "message", ty_nullable(ty_named("String")))],
        Some(("Exception", vec![var("message")])),
        Vec::new(),
        Vec::new(),
    );
    let Decl::Class(class) = &mut declaration else {
        unreachable!("class declaration builder returns a class")
    };
    let ast::ClassConstructorDecl::Declared(constructor) = &mut class.constructor else {
        unreachable!("class declaration builder declares a primary constructor")
    };
    let parameter = &mut constructor.parameters[0];
    parameter.property = ast::PrimaryParameterProperty::Plain;
    parameter.member_visibility = None;
    parameter.syntax = ast::ParameterSyntax::Default {
        expression: some(str_lit("illegal argument")),
        equals_span: sp(),
    };
    declaration
}

fn range_test_iterator() -> Decl {
    let mut next = override_method_expr(
        "next",
        Vec::new(),
        Some(ty_generic("Option", vec![ty_named("T")])),
        none(),
    );
    next.visibility = explicit_visibility(ast::DeclaredVisibility::Public);
    let mut declaration = class_decl(
        ast::ClassModifier::Final,
        "RangeTestIterator",
        Vec::new(),
        None,
        Vec::new(),
        vec![next],
    );
    let Decl::Class(class) = &mut declaration else {
        unreachable!("class declaration builder returns a class")
    };
    class.type_params = vec![type_param("T")];
    class
        .supertypes
        .push(bare_supertype(ty_generic("Iterator", vec![ty_named("T")])));
    declaration
}

fn range_declaration(case: RangeCase) -> Decl {
    let element = case.element.canonical_name();
    let mut iterator = override_method_expr(
        "iterator",
        Vec::new(),
        Some(ty_generic("Iterator", vec![ty_named(element)])),
        typed_call("RangeTestIterator", vec![ty_named(element)], Vec::new()),
    );
    iterator.operator = Some(ast::OperatorModifier { span: sp() });

    let mut step = method_expr(
        "step",
        vec![("value", ty_named(element))],
        Some(ty_named(case.range)),
        call(
            case.range,
            vec![field(this_expr(), "first"), field(this_expr(), "last")],
        ),
    );
    step.infix = Some(ast::InfixModifier { span: sp() });

    let mut contains = method_expr(
        "contains",
        vec![("value", ty_named(element))],
        Some(ty_named("Boolean")),
        bool_lit(true),
    );
    contains.operator = Some(ast::OperatorModifier { span: sp() });

    let mut declaration = class_decl(
        ast::ClassModifier::Final,
        case.range,
        vec![
            (false, "first", ty_named(element)),
            (false, "last", ty_named(element)),
        ],
        None,
        Vec::new(),
        vec![iterator, step, contains],
    );
    let Decl::Class(class) = &mut declaration else {
        unreachable!("class declaration builder returns a class")
    };
    class.supertypes.push(bare_supertype(ty_generic(
        "Iterable",
        vec![ty_named(element)],
    )));
    declaration
}

fn widened_endpoint(owner: hir::IntegerKind, element: hir::IntegerKind, value: Expr) -> Expr {
    if owner == element {
        return value;
    }
    let conversion = match element {
        hir::IntegerKind::SIGNED_32 => "toInt32",
        hir::IntegerKind::UNSIGNED_32 => "toUInt32",
        _ => unreachable!("only narrow Int8/Int16 and UInt8/UInt16 endpoints widen"),
    };
    method_call(value, conversion, Vec::new())
}

fn integer_range_methods(case: IntegerRangeCase) -> Vec<FunctionDecl> {
    RangeRole::ALL
        .into_iter()
        .map(|role| {
            let mut method = method_expr(
                role.name(),
                vec![("endpoint", ty_named(case.kind.canonical_name()))],
                Some(ty_named(case.range)),
                call(
                    case.range,
                    vec![
                        widened_endpoint(case.kind, case.element, this_expr()),
                        widened_endpoint(case.kind, case.element, var("endpoint")),
                    ],
                ),
            );
            method.operator = role
                .operator()
                .map(|_| ast::OperatorModifier { span: sp() });
            method.infix = role.is_infix().then_some(ast::InfixModifier { span: sp() });
            method
        })
        .collect()
}

fn restrict_range_representation(source: &mut SourceFile) {
    for declaration in &mut source.declarations {
        let Decl::Class(class) = declaration else {
            continue;
        };
        let is_range = RANGE_CASES.iter().any(|case| class.name.text == case.range);
        let is_helper = class.name.text == "RangeTestIterator";
        if !is_range && !is_helper {
            continue;
        }
        if is_helper {
            class.visibility = explicit_visibility(ast::DeclaredVisibility::Internal);
        }
        let ast::ClassConstructorDecl::Declared(constructor) = &mut class.constructor else {
            unreachable!("range test classes declare primary constructors")
        };
        constructor.visibility = explicit_visibility(ast::DeclaredVisibility::Internal);
        if is_range {
            for parameter in &mut constructor.parameters {
                parameter.member_visibility =
                    Some(explicit_visibility(ast::DeclaredVisibility::Internal));
            }
        }
    }
}

fn m22_range_core() -> SourceFile {
    let mut core = core_file();
    core.declarations.push(illegal_argument_exception());
    core.declarations.push(range_test_iterator());
    core.declarations
        .extend(RANGE_CASES.into_iter().map(range_declaration));
    for case in INTEGER_RANGE_CASES {
        let owner = core
            .declarations
            .iter_mut()
            .find_map(|declaration| match declaration {
                Decl::Struct(owner) if owner.name.text == case.kind.canonical_name() => Some(owner),
                _ => None,
            })
            .unwrap_or_else(|| panic!("test core declares {}", case.kind.canonical_name()));
        owner.members.extend(
            integer_range_methods(case)
                .into_iter()
                .map(Box::new)
                .map(ast::StructMember::Function),
        );
    }
    make_core_public(&mut core);
    restrict_range_representation(&mut core);
    core
}

fn find_class(module: &hir::Module, name: &str) -> hir::ClassId {
    module
        .classes
        .iter()
        .find_map(|(id, class)| (class.name == name).then_some(id))
        .unwrap_or_else(|| panic!("missing class `{name}`"))
}

fn class_method(module: &hir::Module, class: hir::ClassId, name: &str) -> hir::FunctionId {
    module.classes[class]
        .methods
        .iter()
        .copied()
        .find(|method| module.functions[*method].name.rsplit('.').next() == Some(name))
        .unwrap_or_else(|| panic!("missing method `{}.{name}`", module.classes[class].name))
}

fn integer_method(module: &hir::Module, kind: hir::IntegerKind, name: &str) -> hir::FunctionId {
    let owner = module.intrinsic_type_core.integers.owner(kind);
    module.structs[owner]
        .methods
        .iter()
        .copied()
        .find(|method| module.functions[*method].name.rsplit('.').next() == Some(name))
        .unwrap_or_else(|| panic!("missing method `{}.{name}`", kind.canonical_name()))
}

fn callee_function(module: &hir::Module, expression: &hir::Expr) -> hir::FunctionId {
    match &expression.kind {
        hir::ExprKind::Call { callee, .. } => module.callable_function(*callee),
        hir::ExprKind::MethodCall { callee, .. } => module.callable_function(*callee),
        other => panic!("expected a resolved call, found {other:?}"),
    }
}

fn user_body(module: &hir::Module, function: hir::FunctionId) -> &hir::Body {
    let hir::FunctionKind::User(body) = &module.functions[function].kind else {
        panic!("expected an ordinary Scoop function body")
    };
    body
}

fn main_body(module: &hir::Module) -> &hir::Body {
    user_body(module, module.entry)
}

fn for_plans(body: &hir::Body) -> Vec<&hir::ForIterationPlan> {
    body.statements
        .iter()
        .filter_map(|statement| match &statement.kind {
            hir::StatementKind::For(plan) => Some(plan.as_ref()),
            _ => None,
        })
        .collect()
}

fn assert_plain_public_method(
    module: &hir::Module,
    function: hir::FunctionId,
    owner: &str,
    name: &str,
    parameter: Option<(&str, &str)>,
    result: &str,
    modifiers: (Option<hir::OperatorKind>, bool),
) {
    let (operator, infix) = modifiers;
    let function = &module.functions[function];
    assert_eq!(function.name, format!("{owner}.{name}"));
    assert_eq!(function.access.declared, hir::DeclaredVisibility::Public);
    assert!(!function.is_suspend);
    assert!(matches!(
        &function.genericity,
        hir::FunctionGenericity::Plain
    ));
    assert!(matches!(&function.kind, hir::FunctionKind::User(_)));
    assert_eq!(function.modifiers.operator, operator);
    assert_eq!(function.modifiers.is_infix, infix);
    let method = function.method.expect("range API must be an owner member");
    assert_eq!(hir::type_name(module, method.owner), owner);
    assert_eq!(function.params[0].name, "this");
    assert_eq!(hir::type_name(module, function.params[0].ty), owner);
    match parameter {
        Some((name, ty)) => {
            assert_eq!(function.params.len(), 2);
            assert_eq!(function.params[1].name, name);
            assert_eq!(hir::type_name(module, function.params[1].ty), ty);
        }
        None => assert_eq!(function.params.len(), 1),
    }
    assert_eq!(hir::type_name(module, function.return_ty), result);
}

#[test]
fn m22_range_nominal_surface_owner_matrix_and_exception_boundary_are_exact() {
    let module = lower(&[m22_range_core(), file(vec![fun("main", Vec::new())])])
        .map(|output| output.export)
        .expect("the ordinary core range surface must lower");

    let range_ids = RANGE_CASES
        .into_iter()
        .map(|case| find_class(&module, case.range))
        .collect::<Vec<_>>();
    for (index, left) in range_ids.iter().enumerate() {
        for right in &range_ids[index + 1..] {
            assert_ne!(left, right, "each range spelling is a distinct nominal id");
        }
    }

    for case in RANGE_CASES {
        let class_id = find_class(&module, case.range);
        let class = &module.classes[class_id];
        assert_eq!(class.modifier, hir::ClassModifier::Final, "{}", case.range);
        assert_eq!(
            class.access.declared,
            hir::DeclaredVisibility::Public,
            "{}",
            case.range
        );
        assert!(module.public_surface.classes.contains(&class_id));
        assert!(class.base_class.is_none());
        assert_eq!(class.interfaces.len(), 1);
        assert_eq!(
            hir::type_name(&module, class.interfaces[0]),
            format!("Iterable<{}>", case.element.canonical_name())
        );
        assert_eq!(class.constructors.len(), 1);
        let constructor = class.constructors[0];
        assert_eq!(
            module.class_constructors[constructor].access.declared,
            hir::DeclaredVisibility::Internal
        );
        assert!(
            !module
                .public_surface
                .class_constructors
                .contains(&constructor)
        );
        assert_eq!(class.properties.len(), 2);
        assert!(class.properties.iter().all(|property| {
            module.properties[*property].access.declared == hir::DeclaredVisibility::Internal
                && !module.public_surface.properties.contains(property)
        }));
        assert_eq!(class.methods.len(), 3);

        let element = case.element.canonical_name();
        let iterator = class_method(&module, class_id, "iterator");
        assert_plain_public_method(
            &module,
            iterator,
            case.range,
            "iterator",
            None,
            &format!("Iterator<{element}>"),
            (Some(hir::OperatorKind::Iterator), false),
        );
        assert!(
            !module.functions[iterator].override_access.is_empty(),
            "{}.iterator must carry its exact override witness",
            case.range
        );
        assert_plain_public_method(
            &module,
            class_method(&module, class_id, "step"),
            case.range,
            "step",
            Some(("value", element)),
            case.range,
            (None, true),
        );
        assert_plain_public_method(
            &module,
            class_method(&module, class_id, "contains"),
            case.range,
            "contains",
            Some(("value", element)),
            "Boolean",
            (Some(hir::OperatorKind::Contains), false),
        );
    }

    for case in INTEGER_RANGE_CASES {
        let owner = case.kind.canonical_name();
        for role in RangeRole::ALL {
            assert_plain_public_method(
                &module,
                integer_method(&module, case.kind, role.name()),
                owner,
                role.name(),
                Some(("endpoint", owner)),
                case.range,
                (role.operator(), role.is_infix()),
            );
        }
    }

    let illegal_argument = find_class(&module, "IllegalArgumentException");
    let illegal_argument_class = &module.classes[illegal_argument];
    assert_eq!(illegal_argument_class.modifier, hir::ClassModifier::Final);
    assert!(illegal_argument_class.type_params.is_empty());
    assert!(illegal_argument_class.is_declared());
    assert_eq!(
        illegal_argument_class.access.declared,
        hir::DeclaredVisibility::Public
    );
    assert!(module.public_surface.classes.contains(&illegal_argument));
    assert_eq!(illegal_argument_class.constructors.len(), 1);
    let illegal_argument_constructor = illegal_argument_class.constructors[0];
    assert_eq!(
        module.class_constructors[illegal_argument_constructor]
            .access
            .declared,
        hir::DeclaredVisibility::Public
    );
    assert!(
        module
            .public_surface
            .class_constructors
            .contains(&illegal_argument_constructor)
    );
    assert_eq!(
        module.class_constructors[illegal_argument_constructor]
            .parameters
            .len(),
        1
    );
    assert_eq!(
        hir::type_name(
            &module,
            module.class_constructors[illegal_argument_constructor].parameters[0].ty
        ),
        "Option<String>"
    );
    assert_eq!(
        hir::type_name(
            &module,
            illegal_argument_class
                .base_class
                .expect("IllegalArgumentException extends Exception")
        ),
        "Exception"
    );
    let hir::CompilerExceptionCore {
        throwable,
        unwrap_exception,
        class_cast_exception,
        arithmetic_exception,
        index_out_of_bounds_exception,
        illegal_state_exception,
        illegal_state_message_constructor: _,
    } = module.exception_core;
    for compiler_owned in [
        throwable.class(),
        unwrap_exception.class(),
        class_cast_exception.class(),
        arithmetic_exception.class(),
        index_out_of_bounds_exception.class(),
        illegal_state_exception.class(),
    ] {
        assert_ne!(illegal_argument, compiler_owned);
    }
}

#[test]
fn m22_range_resolution_widening_and_for_elements_preserve_exact_targets() {
    let mut statements = Vec::new();
    for case in INTEGER_RANGE_CASES {
        let endpoint = format!("{}Endpoint", case.key);
        statements.push(val_ty(
            &endpoint,
            Some(ty_named(case.kind.canonical_name())),
            literal(case.kind, 2),
        ));
        for role in RangeRole::ALL {
            statements.push(val(
                &format!("{}_{}", case.key, role.name()),
                role_expression(role, literal(case.kind, 1), var(&endpoint)),
            ));
        }
    }

    for case in RANGE_CASES {
        let owner = INTEGER_RANGE_CASES
            .into_iter()
            .find(|owner| owner.kind == case.source_owner)
            .expect("each range case names an integer owner");
        let endpoint = format!("{}Endpoint", owner.key);
        let source = || role_expression(RangeRole::RangeTo, var(&endpoint), literal(owner.kind, 3));
        statements.push(val(
            &format!("{}Stepped", case.key),
            infix(source(), "step", literal(case.element, 2)),
        ));
        statements.push(val(
            &format!("{}Contains", case.key),
            binary(BinOp::Contains, literal(case.element, 2), source()),
        ));
    }

    for case in INTEGER_RANGE_CASES {
        let endpoint = format!("{}Endpoint", case.key);
        let item = format!("{}Item", case.key);
        statements.push(for_stmt(
            pat_bind(&item),
            role_expression(RangeRole::RangeTo, var(&endpoint), literal(case.kind, 3)),
            vec![val_ty(
                &format!("{}Seen", case.key),
                Some(ty_named(case.element.canonical_name())),
                var(&item),
            )],
        ));
    }

    let module = lower(&[m22_range_core(), file(vec![fun("main", statements)])])
        .map(|output| output.export)
        .expect("all core range spellings must resolve through ordinary HIR calls");
    let body = main_body(&module);

    for case in INTEGER_RANGE_CASES {
        for role in RangeRole::ALL {
            let name = format!("{}_{}", case.key, role.name());
            let expression = local_init(body, &name);
            assert_eq!(hir::type_name(&module, expression.ty), case.range, "{name}");
            assert_eq!(
                callee_function(&module, expression),
                integer_method(&module, case.kind, role.name()),
                "{name}"
            );
        }
    }

    for case in RANGE_CASES {
        let stepped = format!("{}Stepped", case.key);
        let stepped_expression = local_init(body, &stepped);
        assert_eq!(hir::type_name(&module, stepped_expression.ty), case.range);
        let range = find_class(&module, case.range);
        assert_eq!(
            callee_function(&module, stepped_expression),
            class_method(&module, range, "step")
        );

        let contains = format!("{}Contains", case.key);
        let contains_expression = local_init(body, &contains);
        assert_eq!(contains_expression.ty, module.boolean);
        assert_eq!(
            callee_function(&module, contains_expression),
            class_method(&module, range, "contains")
        );
    }

    let plans = for_plans(body);
    assert_eq!(plans.len(), INTEGER_RANGE_CASES.len());
    for (plan, case) in plans.into_iter().zip(INTEGER_RANGE_CASES) {
        assert_eq!(hir::type_name(&module, plan.source_init().ty), case.range);
        assert_eq!(
            callee_function(&module, plan.source_init()),
            integer_method(&module, case.kind, "rangeTo")
        );
        assert_eq!(
            callee_function(&module, plan.iterator_call()),
            class_method(&module, find_class(&module, case.range), "iterator")
        );
        assert_eq!(
            hir::type_name(&module, plan.next().element().ty),
            case.element.canonical_name()
        );
        let hir::IrrefutableBindingShape::Binding(binding) = &plan.binding().shape else {
            panic!("range for bindings are plain exact element bindings")
        };
        assert_eq!(
            hir::type_name(&module, binding.ty),
            case.element.canonical_name()
        );
    }

    for case in INTEGER_RANGE_CASES {
        let expected_conversions = (case.kind != case.element).then_some((case.kind, case.element));
        for role in RangeRole::ALL {
            let body = user_body(&module, integer_method(&module, case.kind, role.name()));
            let conversions = body
                .statements
                .iter()
                .filter_map(|statement| match &statement.kind {
                    hir::StatementKind::ValDecl {
                        init:
                            hir::Expr {
                                kind: hir::ExprKind::IntegerConversion { conversion, .. },
                                ..
                            },
                        ..
                    } => Some(conversion),
                    _ => None,
                })
                .collect::<Vec<_>>();
            match expected_conversions {
                Some((source, target)) => {
                    assert_eq!(
                        conversions.len(),
                        2,
                        "{}.{}",
                        case.kind.canonical_name(),
                        role.name()
                    );
                    assert!(
                        conversions
                            .iter()
                            .all(|conversion| conversion.source == source
                                && conversion.target_kind == target)
                    );
                }
                None => assert!(
                    conversions.is_empty(),
                    "{}.{} must preserve its already-canonical endpoint width",
                    case.kind.canonical_name(),
                    role.name()
                ),
            }
        }
    }
}
