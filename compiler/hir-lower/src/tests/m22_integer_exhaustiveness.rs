use super::*;

fn suffixed_integer(magnitude: u64, suffix: ast::IntegerSuffix) -> Expr {
    Expr::IntLiteral(ast::IntegerLiteralSyntax {
        magnitude,
        radix: ast::IntegerRadix::Decimal,
        suffix,
        span: sp(),
    })
}

fn signed_pattern(value: i16) -> ast::Pattern {
    let value = i64::from(value);
    if value < 0 {
        pat_lit(unary(UnOp::Neg, int_lit(-value)))
    } else {
        pat_lit(int_lit(value))
    }
}

fn unsigned_pattern(value: u16) -> ast::Pattern {
    pat_lit(suffixed_integer(
        u64::from(value),
        ast::IntegerSuffix::Unsigned,
    ))
}

fn export_when<'module>(module: &'module hir::Module, function_name: &str) -> &'module hir::When {
    let function = module
        .functions
        .iter()
        .map(|(_, function)| function)
        .find(|function| function.name == function_name)
        .expect("test function must exist");
    let hir::FunctionKind::User(body) = &function.kind else {
        panic!("test function must have a user body")
    };
    body.statements
        .iter()
        .find_map(|statement| match &statement.kind {
            hir::StatementKind::When(value) => Some(value),
            _ => None,
        })
        .expect("test function must contain a when")
}

fn concrete_when<'module>(
    module: &'module hir::concrete::Module,
    function_name: &str,
) -> &'module hir::concrete::When {
    let function = module
        .functions
        .iter()
        .map(|(_, function)| function)
        .find(|function| function.name == function_name)
        .expect("concrete test function must exist");
    let hir::concrete::FunctionKind::User(body) = &function.kind else {
        panic!("concrete test function must have a user body")
    };
    body.statements
        .iter()
        .find_map(|statement| match &statement.kind {
            hir::concrete::StatementKind::When(value) => Some(value),
            _ => None,
        })
        .expect("concrete test function must contain a when")
}

#[test]
fn every_int8_singleton_produces_a_typed_exhaustiveness_proof() {
    let arms = (-128i16..=127)
        .map(|value| arm(signed_pattern(value), None, Vec::new()))
        .collect();
    let output = lower_user_output(file(vec![
        fun_sig(
            "checkInt8",
            Vec::new(),
            vec![("value", ty_named("Int8"))],
            None,
            vec![when_stmt(var("value"), arms, None)],
        ),
        fun("main", Vec::new()),
    ]))
    .expect("all 256 Int8 bit patterns must prove a direct when exhaustive");

    assert!(matches!(
        export_when(&output.export, "checkInt8").fallback,
        hir::WhenFallback::Impossible(hir::ExhaustivenessProof::PatternMatrix { .. })
    ));
    assert!(matches!(
        concrete_when(&output.local, "checkInt8").fallback,
        hir::concrete::WhenFallback::Impossible(
            hir::concrete::ExhaustivenessProof::PatternMatrix { .. }
        )
    ));
}

#[test]
fn complete_uint8_singletons_compose_with_a_wildcard_tail_partition() {
    let mut arms = vec![arm(
        pat_tuple(vec![pat_wild(), pat_lit(bool_lit(false))], None),
        None,
        Vec::new(),
    )];
    arms.extend((0u16..=255).map(|value| {
        arm(
            pat_tuple(vec![unsigned_pattern(value), pat_lit(bool_lit(true))], None),
            None,
            Vec::new(),
        )
    }));
    let output = lower_user_output(file(vec![
        fun_sig(
            "checkProduct",
            Vec::new(),
            vec![(
                "value",
                ty_tuple(vec![ty_named("UInt8"), ty_named("Boolean")]),
            )],
            None,
            vec![when_stmt(var("value"), arms, None)],
        ),
        fun("main", Vec::new()),
    ]))
    .expect("singleton rows plus the symbolic other rows must cover a product");

    assert!(matches!(
        export_when(&output.export, "checkProduct").fallback,
        hir::WhenFallback::Impossible(hir::ExhaustivenessProof::PatternMatrix { .. })
    ));
}

#[test]
fn integer_other_partition_reports_the_first_real_missing_values() {
    let signed_arms = (-128i16..=127)
        .filter(|value| *value != -1)
        .map(|value| arm(signed_pattern(value), None, Vec::new()))
        .collect();
    let unsigned_arms = (0u16..=255)
        .filter(|value| *value != 137)
        .map(|value| arm(unsigned_pattern(value), None, Vec::new()))
        .collect();
    let errors = lower_user(file(vec![
        fun_sig(
            "checkInt8",
            Vec::new(),
            vec![("value", ty_named("Int8"))],
            None,
            vec![when_stmt(var("value"), signed_arms, None)],
        ),
        fun_sig(
            "checkUInt8",
            Vec::new(),
            vec![("value", ty_named("UInt8"))],
            None,
            vec![when_stmt(var("value"), unsigned_arms, None)],
        ),
        fun("main", Vec::new()),
    ]))
    .expect_err("one omitted singleton must leave a real OtherInteger witness");

    let messages = errors
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();
    assert!(messages.contains(&"non-exhaustive when: missing pattern -1"));
    assert!(messages.contains(&"non-exhaustive when: missing pattern 137u"));
}
