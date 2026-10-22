use super::*;

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
