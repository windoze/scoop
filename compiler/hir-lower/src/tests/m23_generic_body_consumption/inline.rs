use super::*;

#[test]
fn ordinary_inline_bodies_keep_construction_field_writes_and_strings() {
    with_provider_consumer(
        r#"
        package dev.example.upstream
        public struct Snapshot(val count: Int, val owner: Counter)
        public class Counter public constructor(private var count: Int) {
            public fun snapshot(): Snapshot = Snapshot(count, this)
            public fun advance() { count += 1 }
            public fun copy(): Counter = Counter(count)
            public fun label(): String = "counter"
        }
        "#,
        r#"
        package dev.example.consumer
        import dev.example.upstream.Counter
        import dev.example.upstream.Snapshot
        public fun sample(counter: Counter): Snapshot {
            counter.advance()
            return counter.snapshot()
        }
        public fun copy(counter: Counter): Counter = counter.copy()
        public fun label(counter: Counter): String = counter.label()
        "#,
        |output, _, _, _, _| {
            let local = output.output().local.module();
            assert_eq!(local.imported_inline_bodies.len(), 4);
            let mut strings = Vec::new();
            local
                .visit_executable_expressions::<()>(|occurrence| {
                    if let hir::concrete::ExprKind::StringLiteral { value, owner } =
                        &occurrence.expression.kind
                    {
                        strings.push((occurrence.position.root, value.clone(), *owner));
                    }
                    Ok(())
                })
                .unwrap();
            assert_eq!(strings.len(), 1);
            assert_eq!(strings[0].1, "counter");
            assert_eq!(strings[0].2, hir::StringConstantOwner::CurrentDefinition);
            assert!(
                local
                    .imported_inline_bodies
                    .values()
                    .any(|body| body.materialization == strings[0].0)
            );
            for body in local.imported_inline_bodies.values() {
                assert!(
                    local
                        .functions
                        .values()
                        .all(|function| function.materialization != body.materialization)
                );
            }
        },
    )
    .unwrap_or_else(|errors| panic!("{errors:?}"));
}

#[test]
fn ordinary_inline_bodies_keep_external_identity_and_integer_semantics() {
    with_provider_consumer(
        r#"
        package dev.example.upstream
        public fun add(a: Int, b: Int): Int = a + b
        public fun divide(a: Int, b: Int): Int = a / b
        "#,
        r#"
        package dev.example.consumer
        import dev.example.upstream.add
        import dev.example.upstream.divide
        public fun sum(a: Int, b: Int): Int = add(a, b)
        public fun quotient(a: Int, b: Int): Int = divide(a, b)
        "#,
        |output, _, _, _, _| {
            let local = output.output().local.module();
            assert_eq!(local.imported_inline_bodies.len(), 2);
            assert!(
                local
                    .functions
                    .values()
                    .all(|function| { !["add", "divide"].contains(&function.name.as_str()) })
            );
            let mut operations = Vec::new();
            local
                .visit_executable_expressions::<()>(|occurrence| {
                    if let hir::concrete::ExprKind::IntegerOperation(operation) =
                        &occurrence.expression.kind
                    {
                        operations.push((occurrence.position.root, operation.is_managed()));
                    }
                    Ok(())
                })
                .unwrap();
            assert_eq!(operations.len(), 2);
            assert_eq!(operations.iter().filter(|(_, managed)| *managed).count(), 1);
            assert!(operations.iter().all(|(owner, _)| {
                local
                    .imported_inline_bodies
                    .values()
                    .any(|body| body.materialization == *owner)
            }));
        },
    )
    .unwrap();
}

#[test]
fn ordinary_member_bodies_keep_receivers_and_typed_accessor_identities() {
    with_provider_consumer(
        r#"
        package dev.example.upstream
        public struct Counter(val value: Int) {
            public fun shifted(delta: Int): Int = value + delta
            public val doubled: Int get() = value * 2
        }
        public class Cell public constructor(private val value: Int) {
            public fun shifted(delta: Int): Int = value + delta
            public val doubled: Int get() = value * 2
        }
        public val Counter.extra: Int get() = value + 3
        "#,
        r#"
        package dev.example.consumer
        import dev.example.upstream.Counter
        import dev.example.upstream.Cell
        import dev.example.upstream.extra
        public fun readCounter(value: Counter, delta: Int): Int =
            value.shifted(delta) + value.doubled + value.extra
        public fun readCell(value: Cell, delta: Int): Int =
            value.shifted(delta) + value.doubled
        "#,
        |output, _, _, _, _| {
            use scoop_identity::{CallableTemplateOrigin, CallableTemplateOwner};

            let local = output.output().local.module();
            assert_eq!(local.imported_inline_bodies.len(), 5);
            let mut methods = 0;
            let mut accessors = 0;
            for body in local.imported_inline_bodies.values() {
                let declaration = local.imported_dependency_callables[body.callable]
                    .reference()
                    .declaration();
                match (declaration, body.materialization.template()) {
                    (
                        CallableTemplateOrigin::Function(source),
                        CallableTemplateOwner::Function(id),
                    ) => {
                        assert_eq!(source, id);
                        assert_eq!(body.params.len(), 2);
                        methods += 1;
                    }
                    (
                        CallableTemplateOrigin::Accessor(source),
                        CallableTemplateOwner::Accessor(id),
                    ) => {
                        assert_eq!(source, id);
                        assert_eq!(body.params.len(), 1);
                        accessors += 1;
                    }
                    identity => {
                        panic!("imported body changed its declaration identity: {identity:?}")
                    }
                }
                assert_eq!(body.params[0].name, "this");
                assert!(
                    local
                        .functions
                        .values()
                        .all(|function| { function.materialization != body.materialization })
                );
            }
            assert_eq!((methods, accessors), (2, 3));
        },
    )
    .unwrap_or_else(|errors| panic!("{errors:?}"));
}

#[test]
fn virtual_and_direct_uses_share_one_ordinary_member_body() {
    with_provider_consumer(
        r#"
        package dev.example.upstream
        public open class Base public constructor() {
            public open fun shifted(value: Int): Int = value + 1
        }
        "#,
        r#"
        package dev.example.consumer
        import dev.example.upstream.Base
        public class Derived public constructor(): Base() {
            public override fun shifted(value: Int): Int = super.shifted(value) + 1
        }
        public fun read(value: Base, input: Int): Int = value.shifted(input)
        "#,
        |output, _, _, _, _| {
            let local = output.output().local.module();
            assert_eq!(local.imported_inline_bodies.len(), 1);
            let body = local.imported_inline_bodies.values().next().unwrap();
            let origin = local.imported_dependency_callables[body.callable]
                .reference()
                .declaration();
            let dispatches = local
                .imported_dependency_callables
                .values()
                .filter(|callable| callable.reference().declaration() == origin)
                .map(|callable| callable.dispatch())
                .collect::<Vec<_>>();
            assert_eq!(dispatches.len(), 2);
            assert!(dispatches.contains(&hir::ImportedDependencyDispatch::Direct));
            assert!(
                dispatches
                    .iter()
                    .any(|dispatch| { *dispatch != hir::ImportedDependencyDispatch::Direct })
            );
            local
                .visit_executable_expressions::<()>(|_| Ok(()))
                .unwrap();
        },
    )
    .unwrap_or_else(|errors| panic!("{errors:?}"));
}
