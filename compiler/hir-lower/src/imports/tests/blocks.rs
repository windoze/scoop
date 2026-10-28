use super::*;

fn parsed(source: &str) -> ast::SourceFile {
    scoop_parser::parse(source).unwrap_or_else(|errors| panic!("{errors:?}"))
}

#[test]
fn block_imports_preserve_definition_order_nested_restoration_and_alias_targets() {
    let left = parsed("package left\nfun answerA(): Int = 1");
    let right = parsed("package right\nfun answerB(): Int = 2");
    let user = parsed(
        r#"
        package consumer
        import left.answerA as selected
        fun main() {
            val before = selected();
            {
                import right.answerB as selected
                val inside = selected()
                val callback: () -> Int = ::selected
                fun nested(): Int = selected()
                val result = nested()
            }
            val after = selected()
        }
    "#,
    );
    let output =
        lower_sources(vec![left, right, user]).expect("block imports preserve lexical binding");
    let module = output.export.module();
    let function = |name| {
        module
            .functions
            .iter()
            .find(|(_, function)| function.name == name)
            .unwrap()
    };
    let left = function("answerA").0;
    let right = function("answerB").0;
    let hir::FunctionKind::User(body) = &function("main").1.kind else {
        panic!("source body");
    };
    for (name, expected) in [("before", left), ("inside", right), ("after", left)] {
        let selected = body
            .statements
            .iter()
            .find_map(|statement| {
                let hir::StatementKind::ValDecl {
                    pattern: hir::Pattern::Binding { local },
                    init,
                } = &statement.kind
                else {
                    return None;
                };
                if body.locals[*local].name != name {
                    return None;
                }
                let hir::ExprKind::Call {
                    callee: hir::CallableTarget::Local(callee),
                    ..
                } = &init.kind
                else {
                    panic!("typed direct call");
                };
                Some(module.callable_function(*callee))
            })
            .expect("named initializer");
        assert_eq!(selected, expected, "{name}");
    }
}

#[test]
fn block_imports_resolve_types_variants_properties_and_extensions() {
    let library = parsed(
        r#"
        package api
        struct Item(val value: Int)
        enum Choice { Ready, Value(Int) }
        const val number: Int = 7
        fun Int.extra(): Int = this
    "#,
    );
    let user = parsed(
        r#"
        package consumer
        fun main() {
            import api.*
            import api.Choice.*
            val item: Item = Item(number)
            val ready: Choice = Ready
            val payload: Choice = Value(item.value.extra())
        }
    "#,
    );
    lower_sources(vec![library, user]).expect("ordinary typed selectors work inside a block");
}

#[test]
fn block_imports_do_not_leak_or_hoist_and_keep_access_diagnostics() {
    let library = parsed("package api\nfun answer(): Int = 1\nprivate fun hidden(): Int = 2");
    for (body, message) in [
        (
            "val before = answer(); import api.answer",
            "unknown function `answer`",
        ),
        (
            "{ import api.answer; val inside = answer() }; val after = answer()",
            "unknown function `answer`",
        ),
        (
            "import api.hidden; val result = hidden()",
            "import target is not accessible",
        ),
        ("import api.missing", "import target is not available"),
    ] {
        let user = parsed(&format!("package consumer\nfun main() {{ {body} }}"));
        let errors = lower_sources(vec![library.clone(), user]).expect_err("local import rule");
        assert!(
            errors.iter().any(|error| error.message.contains(message)),
            "{errors:?}"
        );
    }
}

#[test]
fn block_imports_reject_public_reexports_at_the_modifier() {
    let source = "fun main() { public import api.answer }";
    let errors = scoop_parser::parse(source).expect_err("local public import is invalid");
    let error = errors
        .iter()
        .find(|error| error.message == "public import is only allowed in a file header")
        .expect("specific diagnostic");
    let span = error.span.unwrap();
    assert_eq!(&source[span.start as usize..span.end as usize], "public");
}
