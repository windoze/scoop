use super::*;

#[test]
fn local_functions_bind_contexts_at_their_own_entry() {
    with_source(
        r#"
        fun use(): String {
            context(value: String)
            fun read(): String = value
            return context("local") { read() }
        }
    "#,
        |export| {
            let function = export
                .functions
                .values()
                .find(|f| f.name.ends_with(".read"))
                .unwrap();
            let hir::FunctionKind::User(body) = &function.kind else {
                panic!("source body")
            };
            assert!(matches!(&body.statements[0].kind,
                hir::StatementKind::ValDecl { init, .. } if matches!(init.kind, hir::ExprKind::ContextLookup(_))));
        },
    );
}

#[test]
fn methods_extensions_and_computed_properties_keep_context_contracts() {
    with_source(
        r#"
        class Request(val id: Int)
        interface Reader {
            context(request: Request)
            fun read(): Int
            context(request: Request)
            val current: Int
        }
        class ReaderImpl : Reader {
            context(other: Request)
            override fun read(): Int = other.id
            context(other: Request)
            override val current: Int get() = other.id
        }
        context(request: Request)
        fun Request.plus(offset: Int = 1): Int = this.id + request.id + offset
        fun use(request: Request): Int = context(request) {
            val reader: Reader = ReaderImpl()
            reader.read() + reader.current + request.plus()
        }
    "#,
        |_| {},
    );
}

#[test]
fn override_requirements_preserve_count_order_and_exact_types() {
    for contract in ["", "context(b: String, a: Request)", "context(a: Request)"] {
        rejects(
            &format!(
                r#"
            class Request
            interface Reader {{
                context(a: Request, b: String)
                fun read(): Int
            }}
            class ReaderImpl : Reader {{
                {contract}
                override fun read(): Int = 1
            }}
        "#
            ),
            "context requirements must have the same count, order and exact types",
        );
    }
}

#[test]
fn inherited_context_conflicts_are_rejected_without_a_body() {
    rejects(
        r#"
        interface First { context(value: String) fun read(): Int }
        interface Second { fun read(): Int }
        interface Both : First, Second {}
    "#,
        "inherits conflicting context requirements",
    );
}

#[test]
fn a_stored_property_cannot_implement_a_contextual_property() {
    rejects(
        r#"
        interface Reader { context(value: String) val length: Int }
        class ReaderImpl(override val length: Int) : Reader
    "#,
        "context requirements must have the same count, order and exact types",
    );
}

#[test]
fn inherited_class_implementations_must_match_new_interface_obligations() {
    rejects(
        r#"
        open class Base { open fun read(): Int = 1 }
        interface Reader { context(value: String) fun read(): Int }
        class Derived : Base(), Reader
    "#,
        "context requirements must have the same count, order and exact types",
    );
}
