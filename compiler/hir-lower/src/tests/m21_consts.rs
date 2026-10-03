use super::*;

fn const_property(name: &str, ty: TypeRef, expression: Expr) -> ast::PropertyDecl {
    ast::PropertyDecl {
        annotations: Vec::new(),
        visibility: ast::VisibilitySyntax::Omitted,
        modifier: ast::MethodModifier::Final,
        is_override: false,
        mutable: false,
        receiver_ty: None,
        type_params: Vec::new(),
        where_clause: None,
        name: ident(name),
        ty,
        body: ast::PropertyBodySyntax::Const(Box::new(expression)),
        span: sp(),
    }
}

fn ordinary_property(name: &str, expression: Expr) -> ast::PropertyDecl {
    ast::PropertyDecl {
        annotations: vec![ast::Annotation {
            name: ident("Global"),
            args: Vec::new(),
            span: sp(),
        }],
        mutable: true,
        body: ast::PropertyBodySyntax::Initializer {
            expression: Box::new(expression),
            accessors: ast::AccessorSyntax::default(),
        },
        ..const_property(name, ty_named("Int"), int_lit(0))
    }
}

fn integer_with_suffix(magnitude: u64, suffix: ast::IntegerSuffix) -> Expr {
    Expr::IntLiteral(ast::IntegerLiteralSyntax {
        magnitude,
        radix: ast::IntegerRadix::Hexadecimal,
        suffix,
        span: sp(),
    })
}

fn integer_infix(lhs: Expr, name: &str, rhs: Expr) -> Expr {
    Expr::InfixCall {
        lhs: Box::new(lhs),
        target: ast::InfixTarget::Named(ident(name)),
        rhs: Box::new(rhs),
        span: sp(),
    }
}

fn converted_integer(expression: Expr, conversion: &str) -> Expr {
    method_call(expression, conversion, Vec::new())
}

fn integer_constant(kind: hir::IntegerKind, raw: u64) -> hir::HirIntegerConstant {
    match kind {
        hir::IntegerKind::SIGNED_8 => hir::HirIntegerConstant::Signed8(raw as u8),
        hir::IntegerKind::SIGNED_16 => hir::HirIntegerConstant::Signed16(raw as u16),
        hir::IntegerKind::SIGNED_32 => hir::HirIntegerConstant::Signed32(raw as u32),
        hir::IntegerKind::SIGNED_64 => hir::HirIntegerConstant::Signed64(raw),
        hir::IntegerKind::UNSIGNED_8 => hir::HirIntegerConstant::Unsigned8(raw as u8),
        hir::IntegerKind::UNSIGNED_16 => hir::HirIntegerConstant::Unsigned16(raw as u16),
        hir::IntegerKind::UNSIGNED_32 => hir::HirIntegerConstant::Unsigned32(raw as u32),
        hir::IntegerKind::UNSIGNED_64 => hir::HirIntegerConstant::Unsigned64(raw),
    }
}

fn const_integer(module: &hir::Module, name: &str) -> hir::HirIntegerConstant {
    module
        .properties
        .iter()
        .find_map(|(_, property)| {
            (property.name == name).then(|| match &property.representation {
                hir::PropertyRepresentation::Const {
                    value: hir::ConstPropertyValue::Integer(value),
                } => *value,
                _ => panic!("{name} must be an integer const property"),
            })
        })
        .unwrap_or_else(|| panic!("missing const property {name}"))
}

fn user_function<'module>(module: &'module hir::Module, name: &str) -> &'module hir::Function {
    module
        .functions
        .iter()
        .find_map(|(_, function)| (function.name == name).then_some(function))
        .unwrap_or_else(|| panic!("function `{name}` must exist"))
}

fn function_result(function: &hir::Function) -> &hir::Expr {
    let hir::FunctionKind::User(body) = &function.kind else {
        panic!("test function must have a user body")
    };
    return_value(&body.statements)
}

#[test]
fn const_properties_fold_forward_references_and_disappear_at_read_sites() {
    let mut answer = const_property(
        "answer",
        ty_named("Int"),
        binary(BinOp::Add, var("base"), int_lit(2)),
    );
    answer.visibility = ast::VisibilitySyntax::Explicit {
        visibility: ast::DeclaredVisibility::Public,
        span: sp(),
    };
    let source = file(vec![
        Decl::Global(answer),
        Decl::Global(const_property("base", ty_named("Int"), int_lit(40))),
        Decl::Global(const_property(
            "label",
            ty_named("String"),
            binary(BinOp::Add, str_lit("forty"), str_lit("-two")),
        )),
        Decl::Global(const_property(
            "valid",
            ty_named("Boolean"),
            binary(BinOp::Eq, var("answer"), int_lit(42)),
        )),
        fun_expr(
            "readAnswer",
            Vec::new(),
            Vec::new(),
            Some(ty_named("Int")),
            var("answer"),
        ),
        fun_expr(
            "readLabel",
            Vec::new(),
            Vec::new(),
            Some(ty_named("String")),
            var("label"),
        ),
        fun_expr(
            "readValid",
            Vec::new(),
            Vec::new(),
            Some(ty_named("Boolean")),
            var("valid"),
        ),
        fun("main", Vec::new()),
    ]);
    let module = lower_user(source).expect("valid const definitions must lower");

    let (_, answer) = module
        .properties
        .iter()
        .find(|(_, property)| property.name == "answer")
        .expect("answer const property");
    assert!(matches!(
        answer.representation,
        hir::PropertyRepresentation::Const {
            value: hir::ConstPropertyValue::Integer(hir::HirIntegerConstant::Signed32(42))
        }
    ));
    assert_eq!(
        module.property_getters[answer.capability.getter()].implementation,
        hir::PropertyAccessorImplementation::Constant
    );
    assert!(
        module
            .public_surface
            .properties
            .iter()
            .any(|property| { module.properties[*property].name == "answer" })
    );
    assert!(module.globals.iter().all(|(_, global)| !matches!(
        module.properties[global.property].name.as_str(),
        "answer" | "base" | "label" | "valid"
    )));

    assert!(matches!(
        function_result(user_function(&module, "readAnswer")).kind,
        hir::ExprKind::IntegerLiteral(hir::HirIntegerConstant::Signed32(42))
    ));
    let (label, _) = module
        .properties
        .iter()
        .find(|(_, property)| property.name == "label")
        .expect("label const property");
    assert!(matches!(
        &function_result(user_function(&module, "readLabel")).kind,
        hir::ExprKind::StringLiteral {
            value,
            owner: hir::StringConstantOwner::Property(owner),
        } if value == "forty-two" && *owner == label
    ));
    assert!(matches!(
        function_result(user_function(&module, "readValid")).kind,
        hir::ExprKind::BoolLiteral(true)
    ));
}

#[test]
fn const_integer_intrinsics_fold_with_runtime_width_and_shift_semantics() {
    let unsigned = |value| integer_with_suffix(value, ast::IntegerSuffix::Unsigned);
    let long = |value| integer_with_suffix(value, ast::IntegerSuffix::Long);
    let source = file(vec![
        Decl::Global(const_property(
            "masked",
            ty_named("UInt"),
            integer_infix(unsigned(0xf0), "and", unsigned(0x0f)),
        )),
        Decl::Global(const_property(
            "shifted",
            ty_named("Int"),
            integer_infix(int_lit(1), "shl", long(31)),
        )),
        Decl::Global(const_property(
            "logical",
            ty_named("Long"),
            integer_infix(unary(UnOp::Neg, int_lit(1)), "ushr", long(1)),
        )),
        Decl::Global(const_property(
            "narrowed",
            ty_named("UInt8"),
            method_call(int_lit(257), "toUInt8", Vec::new()),
        )),
        Decl::Global(const_property(
            "negatedUnsigned",
            ty_named("UInt8"),
            unary(UnOp::Neg, unsigned(1)),
        )),
        fun("main", Vec::new()),
    ]);
    let module = lower_user(source).expect("typed integer const intrinsics must fold");
    let value = |name| {
        module
            .properties
            .iter()
            .find_map(|(_, property)| {
                (property.name == name).then(|| match &property.representation {
                    hir::PropertyRepresentation::Const { value } => value.clone(),
                    _ => panic!("{name} must remain a const property"),
                })
            })
            .unwrap_or_else(|| panic!("missing const property {name}"))
    };
    assert_eq!(
        value("masked"),
        hir::ConstPropertyValue::Integer(hir::HirIntegerConstant::Unsigned32(0))
    );
    assert_eq!(
        value("shifted"),
        hir::ConstPropertyValue::Integer(hir::HirIntegerConstant::Signed32(0x8000_0000))
    );
    assert_eq!(
        value("logical"),
        hir::ConstPropertyValue::Integer(hir::HirIntegerConstant::Signed64(i64::MAX as u64))
    );
    assert_eq!(
        value("narrowed"),
        hir::ConstPropertyValue::Integer(hir::HirIntegerConstant::Unsigned8(1))
    );
    assert_eq!(
        value("negatedUnsigned"),
        hir::ConstPropertyValue::Integer(hir::HirIntegerConstant::Unsigned8(255))
    );
}

#[test]
fn const_integer_no_gc_intrinsics_are_total_for_every_kind() {
    struct Case {
        name: &'static str,
        ty: &'static str,
        conversion: &'static str,
        signed: bool,
        maximum: Expr,
        minimum: Expr,
        incremented: hir::HirIntegerConstant,
        decremented: hir::HirIntegerConstant,
    }

    let signed = |magnitude| integer_with_suffix(magnitude, ast::IntegerSuffix::None);
    let long = |magnitude| integer_with_suffix(magnitude, ast::IntegerSuffix::Long);
    let unsigned = |magnitude| integer_with_suffix(magnitude, ast::IntegerSuffix::Unsigned);
    let cases = vec![
        Case {
            name: "int8",
            ty: "Int8",
            conversion: "toInt8",
            signed: true,
            maximum: signed(0x7f),
            minimum: unary(UnOp::Neg, signed(0x80)),
            incremented: hir::HirIntegerConstant::Signed8(0x80),
            decremented: hir::HirIntegerConstant::Signed8(0x7f),
        },
        Case {
            name: "int16",
            ty: "Int16",
            conversion: "toInt16",
            signed: true,
            maximum: signed(0x7fff),
            minimum: unary(UnOp::Neg, signed(0x8000)),
            incremented: hir::HirIntegerConstant::Signed16(0x8000),
            decremented: hir::HirIntegerConstant::Signed16(0x7fff),
        },
        Case {
            name: "int",
            ty: "Int",
            conversion: "toInt32",
            signed: true,
            maximum: signed(0x7fff_ffff),
            minimum: unary(UnOp::Neg, long(0x8000_0000)),
            incremented: hir::HirIntegerConstant::Signed32(0x8000_0000),
            decremented: hir::HirIntegerConstant::Signed32(0x7fff_ffff),
        },
        Case {
            name: "long",
            ty: "Long",
            conversion: "toInt64",
            signed: true,
            maximum: long(0x7fff_ffff_ffff_ffff),
            minimum: unary(UnOp::Neg, long(0x8000_0000_0000_0000)),
            incremented: hir::HirIntegerConstant::Signed64(0x8000_0000_0000_0000),
            decremented: hir::HirIntegerConstant::Signed64(0x7fff_ffff_ffff_ffff),
        },
        Case {
            name: "uint8",
            ty: "UInt8",
            conversion: "toUInt8",
            signed: false,
            maximum: unsigned(0xff),
            minimum: unsigned(0),
            incremented: hir::HirIntegerConstant::Unsigned8(0),
            decremented: hir::HirIntegerConstant::Unsigned8(0xff),
        },
        Case {
            name: "uint16",
            ty: "UInt16",
            conversion: "toUInt16",
            signed: false,
            maximum: unsigned(0xffff),
            minimum: unsigned(0),
            incremented: hir::HirIntegerConstant::Unsigned16(0),
            decremented: hir::HirIntegerConstant::Unsigned16(0xffff),
        },
        Case {
            name: "uint",
            ty: "UInt",
            conversion: "toUInt32",
            signed: false,
            maximum: unsigned(0xffff_ffff),
            minimum: unsigned(0),
            incremented: hir::HirIntegerConstant::Unsigned32(0),
            decremented: hir::HirIntegerConstant::Unsigned32(0xffff_ffff),
        },
        Case {
            name: "ulong",
            ty: "ULong",
            conversion: "toUInt64",
            signed: false,
            maximum: unsigned(0xffff_ffff_ffff_ffff),
            minimum: unsigned(0),
            incremented: hir::HirIntegerConstant::Unsigned64(0),
            decremented: hir::HirIntegerConstant::Unsigned64(0xffff_ffff_ffff_ffff),
        },
    ];

    let mut declarations = Vec::new();
    let mut expected = Vec::new();
    for case in cases {
        let maximum = converted_integer(case.maximum, case.conversion);
        let minimum = converted_integer(case.minimum, case.conversion);
        let zero = converted_integer(unsigned(0), case.conversion);
        let one = converted_integer(unsigned(1), case.conversion);
        let two = converted_integer(unsigned(2), case.conversion);
        let inc_name = format!("{}_inc", case.name);
        let dec_name = format!("{}_dec", case.name);
        let compare_name = format!("{}_compare", case.name);
        let compare_equal_name = format!("{}_compare_equal", case.name);
        let compare_greater_name = format!("{}_compare_greater", case.name);
        let equals_name = format!("{}_equals", case.name);
        let compare_less = if case.signed {
            minimum.clone()
        } else {
            zero.clone()
        };
        declarations.push(Decl::Global(const_property(
            &inc_name,
            ty_named(case.ty),
            method_call(maximum, "inc", Vec::new()),
        )));
        declarations.push(Decl::Global(const_property(
            &dec_name,
            ty_named(case.ty),
            method_call(minimum, "dec", Vec::new()),
        )));
        declarations.push(Decl::Global(const_property(
            &compare_name,
            ty_named("Long"),
            method_call(compare_less, "compareTo", vec![one.clone()]),
        )));
        declarations.push(Decl::Global(const_property(
            &compare_equal_name,
            ty_named("Long"),
            method_call(one.clone(), "compareTo", vec![one.clone()]),
        )));
        declarations.push(Decl::Global(const_property(
            &compare_greater_name,
            ty_named("Long"),
            method_call(one.clone(), "compareTo", vec![zero.clone()]),
        )));
        declarations.push(Decl::Global(const_property(
            &equals_name,
            ty_named("Boolean"),
            method_call(zero.clone(), "equals", vec![zero]),
        )));
        expected.push((inc_name, hir::ConstPropertyValue::Integer(case.incremented)));
        expected.push((dec_name, hir::ConstPropertyValue::Integer(case.decremented)));
        expected.push((
            compare_name,
            hir::ConstPropertyValue::Integer(hir::HirIntegerConstant::Signed64(u64::MAX)),
        ));
        expected.push((
            compare_equal_name,
            hir::ConstPropertyValue::Integer(hir::HirIntegerConstant::Signed64(0)),
        ));
        expected.push((
            compare_greater_name,
            hir::ConstPropertyValue::Integer(hir::HirIntegerConstant::Signed64(1)),
        ));
        expected.push((equals_name, hir::ConstPropertyValue::Boolean(true)));

        let owner_operations = vec![
            (
                "unary_plus",
                method_call(one.clone(), "unaryPlus", Vec::new()),
            ),
            (
                "unary_minus",
                method_call(one.clone(), "unaryMinus", Vec::new()),
            ),
            ("plus", method_call(one.clone(), "plus", vec![two.clone()])),
            (
                "minus",
                method_call(one.clone(), "minus", vec![two.clone()]),
            ),
            (
                "times",
                method_call(two.clone(), "times", vec![two.clone()]),
            ),
            ("and", method_call(one.clone(), "and", vec![two.clone()])),
            ("or", method_call(one.clone(), "or", vec![two.clone()])),
            ("xor", method_call(one.clone(), "xor", vec![two.clone()])),
            ("inv", method_call(one.clone(), "inv", Vec::new())),
            ("shl", method_call(one.clone(), "shl", vec![long(1)])),
            ("shr", method_call(one.clone(), "shr", vec![long(1)])),
        ];
        for (operation, expression) in owner_operations {
            declarations.push(Decl::Global(const_property(
                &format!("{}_{}", case.name, operation),
                ty_named(case.ty),
                expression,
            )));
        }
        if case.signed {
            declarations.push(Decl::Global(const_property(
                &format!("{}_ushr", case.name),
                ty_named(case.ty),
                method_call(one.clone(), "ushr", vec![long(1)]),
            )));
        }
    }
    declarations.push(fun("main", Vec::new()));

    let module = lower_user(file(declarations))
        .expect("every typed no-gc integer operation and conversion must fold");
    for (name, expected) in expected {
        let value = module
            .properties
            .iter()
            .find_map(|(_, property)| {
                (property.name == name).then(|| match &property.representation {
                    hir::PropertyRepresentation::Const { value } => value.clone(),
                    _ => panic!("{name} must remain a const property"),
                })
            })
            .unwrap_or_else(|| panic!("missing const property {name}"));
        assert_eq!(value, expected, "wrong folded value for {name}");
    }
}

#[test]
fn const_integer_conversions_preserve_nontrivial_bits_for_every_pair() {
    struct Source {
        name: &'static str,
        kind: hir::IntegerKind,
        conversion: &'static str,
        expression: Expr,
        raw: u64,
    }

    let signed = |magnitude| integer_with_suffix(magnitude, ast::IntegerSuffix::None);
    let long = |magnitude| integer_with_suffix(magnitude, ast::IntegerSuffix::Long);
    let unsigned = |magnitude| integer_with_suffix(magnitude, ast::IntegerSuffix::Unsigned);
    let sources = [
        Source {
            name: "int8",
            kind: hir::IntegerKind::SIGNED_8,
            conversion: "toInt8",
            expression: unary(UnOp::Neg, signed(1)),
            raw: 0xff,
        },
        Source {
            name: "int16",
            kind: hir::IntegerKind::SIGNED_16,
            conversion: "toInt16",
            expression: unary(UnOp::Neg, signed(257)),
            raw: 0xfeff,
        },
        Source {
            name: "int",
            kind: hir::IntegerKind::SIGNED_32,
            conversion: "toInt32",
            expression: unary(UnOp::Neg, signed(65_537)),
            raw: 0xfffe_ffff,
        },
        Source {
            name: "long",
            kind: hir::IntegerKind::SIGNED_64,
            conversion: "toInt64",
            expression: unary(UnOp::Neg, long(4_294_967_297)),
            raw: 0xffff_fffe_ffff_ffff,
        },
        Source {
            name: "uint8",
            kind: hir::IntegerKind::UNSIGNED_8,
            conversion: "toUInt8",
            expression: unsigned(0xff),
            raw: 0xff,
        },
        Source {
            name: "uint16",
            kind: hir::IntegerKind::UNSIGNED_16,
            conversion: "toUInt16",
            expression: unsigned(0xffff),
            raw: 0xffff,
        },
        Source {
            name: "uint",
            kind: hir::IntegerKind::UNSIGNED_32,
            conversion: "toUInt32",
            expression: unsigned(0xffff_ffff),
            raw: 0xffff_ffff,
        },
        Source {
            name: "ulong",
            kind: hir::IntegerKind::UNSIGNED_64,
            conversion: "toUInt64",
            expression: unsigned(0xffff_ffff_ffff_ffff),
            raw: u64::MAX,
        },
    ];
    let targets = [
        ("int8", "Int8", "toInt8", hir::IntegerKind::SIGNED_8),
        ("int16", "Int16", "toInt16", hir::IntegerKind::SIGNED_16),
        ("int", "Int", "toInt32", hir::IntegerKind::SIGNED_32),
        ("long", "Long", "toInt64", hir::IntegerKind::SIGNED_64),
        ("uint8", "UInt8", "toUInt8", hir::IntegerKind::UNSIGNED_8),
        (
            "uint16",
            "UInt16",
            "toUInt16",
            hir::IntegerKind::UNSIGNED_16,
        ),
        ("uint", "UInt", "toUInt32", hir::IntegerKind::UNSIGNED_32),
        ("ulong", "ULong", "toUInt64", hir::IntegerKind::UNSIGNED_64),
    ];

    let mut declarations = Vec::new();
    let mut expected = Vec::new();
    for source in sources {
        let source_expression = converted_integer(source.expression, source.conversion);
        for (target_name, target_ty, conversion, target_kind) in targets {
            let name = format!("{}_to_{target_name}", source.name);
            declarations.push(Decl::Global(const_property(
                &name,
                ty_named(target_ty),
                method_call(source_expression.clone(), conversion, Vec::new()),
            )));
            let source_value = match source.kind.signedness() {
                hir::IntegerSignedness::Signed => {
                    let bits = source.kind.width().bits();
                    let sign = 1_u64 << (bits - 1);
                    if source.raw & sign == 0 {
                        i128::from(source.raw)
                    } else {
                        i128::from(source.raw) - (1_i128 << bits)
                    }
                }
                hir::IntegerSignedness::Unsigned => i128::from(source.raw),
            };
            let raw = (source_value as u128 & u128::from(target_kind.width().raw_mask())) as u64;
            expected.push((name, integer_constant(target_kind, raw)));
        }
    }
    declarations.push(fun("main", Vec::new()));

    let module = lower_user(file(declarations))
        .expect("all 8x8 typed integer conversions must be const-foldable");
    for (name, expected) in expected {
        assert_eq!(
            const_integer(&module, &name),
            expected,
            "wrong value for {name}"
        );
    }
}

#[test]
fn const_integer_div_rem_methods_fold_every_kind_and_signed_boundary() {
    struct Case {
        name: &'static str,
        kind: hir::IntegerKind,
        ty: &'static str,
        conversion: &'static str,
        dividend: Expr,
        divisor: Expr,
        quotient_raw: u64,
        remainder_raw: u64,
    }

    let signed = |magnitude| integer_with_suffix(magnitude, ast::IntegerSuffix::None);
    let long = |magnitude| integer_with_suffix(magnitude, ast::IntegerSuffix::Long);
    let unsigned = |magnitude| integer_with_suffix(magnitude, ast::IntegerSuffix::Unsigned);
    let cases = [
        Case {
            name: "int8",
            kind: hir::IntegerKind::SIGNED_8,
            ty: "Int8",
            conversion: "toInt8",
            dividend: unary(UnOp::Neg, signed(0x80)),
            divisor: unary(UnOp::Neg, signed(1)),
            quotient_raw: 0x80,
            remainder_raw: 0,
        },
        Case {
            name: "int16",
            kind: hir::IntegerKind::SIGNED_16,
            ty: "Int16",
            conversion: "toInt16",
            dividend: unary(UnOp::Neg, signed(0x8000)),
            divisor: unary(UnOp::Neg, signed(1)),
            quotient_raw: 0x8000,
            remainder_raw: 0,
        },
        Case {
            name: "int",
            kind: hir::IntegerKind::SIGNED_32,
            ty: "Int",
            conversion: "toInt32",
            dividend: unary(UnOp::Neg, long(0x8000_0000)),
            divisor: unary(UnOp::Neg, signed(1)),
            quotient_raw: 0x8000_0000,
            remainder_raw: 0,
        },
        Case {
            name: "long",
            kind: hir::IntegerKind::SIGNED_64,
            ty: "Long",
            conversion: "toInt64",
            dividend: unary(UnOp::Neg, long(0x8000_0000_0000_0000)),
            divisor: unary(UnOp::Neg, signed(1)),
            quotient_raw: 0x8000_0000_0000_0000,
            remainder_raw: 0,
        },
        Case {
            name: "uint8",
            kind: hir::IntegerKind::UNSIGNED_8,
            ty: "UInt8",
            conversion: "toUInt8",
            dividend: unsigned(0xff),
            divisor: unsigned(2),
            quotient_raw: 0x7f,
            remainder_raw: 1,
        },
        Case {
            name: "uint16",
            kind: hir::IntegerKind::UNSIGNED_16,
            ty: "UInt16",
            conversion: "toUInt16",
            dividend: unsigned(0xffff),
            divisor: unsigned(2),
            quotient_raw: 0x7fff,
            remainder_raw: 1,
        },
        Case {
            name: "uint",
            kind: hir::IntegerKind::UNSIGNED_32,
            ty: "UInt",
            conversion: "toUInt32",
            dividend: unsigned(0xffff_ffff),
            divisor: unsigned(2),
            quotient_raw: 0x7fff_ffff,
            remainder_raw: 1,
        },
        Case {
            name: "ulong",
            kind: hir::IntegerKind::UNSIGNED_64,
            ty: "ULong",
            conversion: "toUInt64",
            dividend: unsigned(0xffff_ffff_ffff_ffff),
            divisor: unsigned(2),
            quotient_raw: 0x7fff_ffff_ffff_ffff,
            remainder_raw: 1,
        },
    ];

    let mut declarations = Vec::new();
    let mut expected = Vec::new();
    for case in cases {
        let dividend = converted_integer(case.dividend, case.conversion);
        let divisor = converted_integer(case.divisor, case.conversion);
        for (operation, expected_raw) in [("div", case.quotient_raw), ("rem", case.remainder_raw)] {
            let name = format!("{}_{}", case.name, operation);
            declarations.push(Decl::Global(const_property(
                &name,
                ty_named(case.ty),
                method_call(dividend.clone(), operation, vec![divisor.clone()]),
            )));
            expected.push((name, integer_constant(case.kind, expected_raw)));
        }
    }
    declarations.push(fun("main", Vec::new()));

    let module = lower_user(file(declarations))
        .expect("all typed integer div/rem methods must be const-foldable");
    for (name, expected) in expected {
        assert_eq!(
            const_integer(&module, &name),
            expected,
            "wrong value for {name}"
        );
    }
}

#[test]
fn const_integer_div_rem_methods_report_zero_divisors() {
    let int8 = |value| converted_integer(int_lit(value), "toInt8");
    let ulong = |value| {
        converted_integer(
            integer_with_suffix(value, ast::IntegerSuffix::Unsigned),
            "toUInt64",
        )
    };
    let source = file(vec![
        Decl::Global(const_property(
            "badDiv",
            ty_named("Int8"),
            method_call(int8(1), "div", vec![int8(0)]),
        )),
        Decl::Global(const_property(
            "badRem",
            ty_named("ULong"),
            method_call(ulong(1), "rem", vec![ulong(0)]),
        )),
        fun("main", Vec::new()),
    ]);
    let errors = lower_user(source).expect_err("constant div/rem by zero must fail");
    assert_eq!(errors.len(), 2, "unexpected diagnostics: {errors:?}");
    assert!(
        errors
            .iter()
            .all(|diagnostic| { diagnostic.message == "division by zero in const initializer" })
    );
}

#[test]
fn const_integer_joint_inference_is_order_independent() {
    let wide = integer_with_suffix(2_147_483_648, ast::IntegerSuffix::None);
    let source = file(vec![
        Decl::Global(const_property("narrow", ty_named("Int8"), int_lit(2))),
        Decl::Global(const_property("wide", ty_named("Long"), wide.clone())),
        Decl::Global(const_property(
            "literalNarrow",
            ty_named("Boolean"),
            binary(BinOp::Eq, int_lit(2), var("narrow")),
        )),
        Decl::Global(const_property(
            "narrowLiteral",
            ty_named("Boolean"),
            binary(BinOp::Eq, var("narrow"), int_lit(2)),
        )),
        Decl::Global(const_property(
            "literalWide",
            ty_named("Boolean"),
            binary(BinOp::Eq, int_lit(1), wide.clone()),
        )),
        Decl::Global(const_property(
            "wideLiteral",
            ty_named("Boolean"),
            binary(BinOp::Eq, wide.clone(), int_lit(1)),
        )),
        Decl::Global(const_property(
            "compareNarrow",
            ty_named("Long"),
            method_call(int_lit(1), "compareTo", vec![var("narrow")]),
        )),
        Decl::Global(const_property(
            "equalsWide",
            ty_named("Boolean"),
            method_call(int_lit(1), "equals", vec![var("wide")]),
        )),
        Decl::Global(const_property(
            "divNarrow",
            ty_named("Boolean"),
            binary(
                BinOp::Eq,
                method_call(int_lit(6), "div", vec![var("narrow")]),
                int_lit(3),
            ),
        )),
        Decl::Global(const_property(
            "remWide",
            ty_named("Boolean"),
            binary(
                BinOp::Eq,
                method_call(int_lit(1), "rem", vec![var("wide")]),
                int_lit(1),
            ),
        )),
        fun("main", Vec::new()),
    ]);
    let module = lower_user(source).expect("joint const integer inference must be symmetric");
    let value = |name| {
        module
            .properties
            .iter()
            .find_map(|(_, property)| {
                (property.name == name).then(|| match &property.representation {
                    hir::PropertyRepresentation::Const { value } => value.clone(),
                    _ => panic!("{name} must remain a const property"),
                })
            })
            .unwrap_or_else(|| panic!("missing const property {name}"))
    };
    assert_eq!(
        value("literalNarrow"),
        hir::ConstPropertyValue::Boolean(true)
    );
    assert_eq!(
        value("narrowLiteral"),
        hir::ConstPropertyValue::Boolean(true)
    );
    assert_eq!(
        value("literalWide"),
        hir::ConstPropertyValue::Boolean(false)
    );
    assert_eq!(
        value("wideLiteral"),
        hir::ConstPropertyValue::Boolean(false)
    );
    assert_eq!(
        value("compareNarrow"),
        hir::ConstPropertyValue::Integer(hir::HirIntegerConstant::Signed64(u64::MAX))
    );
    assert_eq!(value("equalsWide"), hir::ConstPropertyValue::Boolean(false));
    assert_eq!(value("divNarrow"), hir::ConstPropertyValue::Boolean(true));
    assert_eq!(value("remWide"), hir::ConstPropertyValue::Boolean(true));
}

#[test]
fn const_integer_inference_does_not_contextualize_composite_receivers() {
    let source = file(vec![
        Decl::Global(const_property("narrow", ty_named("Int8"), int_lit(3))),
        Decl::Global(const_property(
            "nestedEquality",
            ty_named("Boolean"),
            binary(
                BinOp::Eq,
                binary(BinOp::Add, int_lit(1), int_lit(2)),
                var("narrow"),
            ),
        )),
        Decl::Global(const_property(
            "nestedOperator",
            ty_named("Int8"),
            binary(
                BinOp::Add,
                binary(BinOp::Add, int_lit(1), int_lit(2)),
                int_lit(3),
            ),
        )),
        Decl::Global(const_property(
            "nestedMethod",
            ty_named("Int8"),
            method_call(
                binary(BinOp::Add, int_lit(1), int_lit(2)),
                "inc",
                Vec::new(),
            ),
        )),
        fun("main", Vec::new()),
    ]);
    let errors = lower_user(source)
        .expect_err("const inference must preserve ordinary receiver inference boundaries");
    let messages = errors
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();
    assert!(messages.iter().any(|message| {
        message == &"const operator operands must have the same type, found Int and Int8"
    }));
    assert!(messages.iter().any(|message| {
        message.contains("const initializer of `nestedOperator` must be of type Int8, found Int")
    }));
    assert!(messages.iter().any(|message| {
        message.contains("const initializer of `nestedMethod` must be of type Int8, found Int")
    }));
}

#[test]
fn const_rejects_invalid_declarations_and_non_constant_expressions() {
    let mut mutable = const_property("mutable", ty_named("Int"), int_lit(1));
    mutable.mutable = true;
    let source = file(vec![
        Decl::Global(mutable),
        Decl::Global(const_property("unit", ty_named("Unit"), unit_lit())),
        Decl::Global(ordinary_property("ordinary", int_lit(3))),
        Decl::Global(const_property(
            "ordinaryRead",
            ty_named("Int"),
            var("ordinary"),
        )),
        Decl::Global(const_property(
            "called",
            ty_named("Int"),
            call("helper", Vec::new()),
        )),
        fun_expr(
            "helper",
            Vec::new(),
            Vec::new(),
            Some(ty_named("Int")),
            int_lit(1),
        ),
        fun("main", Vec::new()),
    ]);
    let errors = lower_user(source).expect_err("invalid const definitions must fail");
    let messages = errors
        .into_iter()
        .map(|diagnostic| diagnostic.message)
        .collect::<Vec<_>>();
    assert!(messages.contains(&"const property must be a `val`".to_string()));
    assert!(messages.contains(&"const property `unit` has unsupported type Unit".to_string()));
    assert!(
        messages.contains(
            &"const initializer may only reference const properties; `ordinary` is not const"
                .to_string()
        )
    );
    assert!(messages.contains(
        &"const initializer must contain only literals, const references, built-in primitive operators, and exact core integer intrinsic calls"
            .to_string()
    ));
}

#[test]
fn const_dependency_cycle_is_detected_through_short_circuit_rhs() {
    let source = file(vec![
        Decl::Global(const_property(
            "first",
            ty_named("Boolean"),
            binary(BinOp::Or, bool_lit(true), var("second")),
        )),
        Decl::Global(const_property(
            "second",
            ty_named("Boolean"),
            binary(BinOp::And, bool_lit(false), var("first")),
        )),
        fun("main", Vec::new()),
    ]);
    let errors = lower_user(source).expect_err("const dependency cycles must fail");
    assert!(errors.iter().any(|diagnostic| {
        diagnostic.message == "const dependency cycle: first -> second -> first"
    }));
}

#[test]
fn const_short_circuit_rhs_is_still_validated_as_a_constant_expression() {
    let invalid = file(vec![
        Decl::Global(const_property(
            "wrongType",
            ty_named("Boolean"),
            binary(BinOp::Or, bool_lit(true), int_lit(1)),
        )),
        Decl::Global(const_property(
            "ordinaryCall",
            ty_named("Boolean"),
            binary(BinOp::And, bool_lit(false), call("helper", Vec::new())),
        )),
        Decl::Global(const_property(
            "binaryDivisionByZero",
            ty_named("Boolean"),
            binary(
                BinOp::And,
                bool_lit(false),
                binary(
                    BinOp::Eq,
                    binary(BinOp::Div, int_lit(1), int_lit(0)),
                    int_lit(0),
                ),
            ),
        )),
        Decl::Global(const_property(
            "methodRemainderByZero",
            ty_named("Boolean"),
            binary(
                BinOp::Or,
                bool_lit(true),
                binary(
                    BinOp::Eq,
                    method_call(int_lit(1), "rem", vec![int_lit(0)]),
                    int_lit(0),
                ),
            ),
        )),
        fun_expr(
            "helper",
            Vec::new(),
            Vec::new(),
            Some(ty_named("Boolean")),
            bool_lit(true),
        ),
        fun("main", Vec::new()),
    ]);
    let errors = lower_user(invalid).expect_err("every short-circuit RHS must be validated");
    let messages = errors
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();
    assert!(messages.iter().any(|message| {
        message.contains("const operator operands must have the same type")
            || message.contains("expected Boolean")
            || message == &"boolean const operator requires Boolean operands"
    }));
    assert!(messages.iter().any(|message| {
        message == &"const initializer must contain only literals, const references, built-in primitive operators, and exact core integer intrinsic calls"
    }));
    assert_eq!(
        messages
            .iter()
            .filter(|message| **message == "division by zero in const initializer")
            .count(),
        2
    );

    let valid = file(vec![
        Decl::Global(const_property(
            "shortFalse",
            ty_named("Boolean"),
            binary(
                BinOp::And,
                bool_lit(false),
                binary(BinOp::Eq, int_lit(1), int_lit(2)),
            ),
        )),
        Decl::Global(const_property(
            "shortTrue",
            ty_named("Boolean"),
            binary(
                BinOp::Or,
                bool_lit(true),
                binary(BinOp::Eq, int_lit(1), int_lit(2)),
            ),
        )),
        fun("main", Vec::new()),
    ]);
    let module = lower_user(valid).expect("valid short-circuit const expressions must fold");
    for (name, expected) in [("shortFalse", false), ("shortTrue", true)] {
        let property = module
            .properties
            .iter()
            .find_map(|(_, property)| (property.name == name).then_some(property))
            .unwrap_or_else(|| panic!("missing const property `{name}`"));
        assert!(matches!(
            property.representation,
            hir::PropertyRepresentation::Const {
                value: hir::ConstPropertyValue::Boolean(value)
            } if value == expected
        ));
    }
}

#[test]
fn const_visibility_is_checked_before_folding() {
    let mut secret = const_property("secret", ty_named("Int"), int_lit(41));
    secret.visibility = ast::VisibilitySyntax::Explicit {
        visibility: ast::DeclaredVisibility::Private,
        span: sp(),
    };
    let source = file(vec![
        Decl::Global(const_property(
            "answer",
            ty_named("Int"),
            binary(BinOp::Add, var("secret"), int_lit(1)),
        )),
        fun("main", Vec::new()),
    ]);
    let errors = lower(&[core_file(), file(vec![Decl::Global(secret)]), source])
        .expect_err("file-private const must not be visible from another file");
    assert!(errors.iter().any(|diagnostic| {
        diagnostic.message == "const property `secret` is not accessible here"
    }));
}
