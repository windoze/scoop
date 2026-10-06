use super::*;

const PROVIDER: &str = r#"
public fun <T> erase(value: T): Any = value
public fun <T> identity(value: T): T = value
public fun <T> adapt(sink: (Any) -> Unit): (T) -> Unit = sink
public class Holder<T> public constructor(public val value: T) {
    public fun erased(): Any = this.value
}
"#;

#[test]
fn generic_primitive_boxing_retains_source_and_imported_argument_declarations() {
    with_provider_consumer(
        PROVIDER,
        r#"
        private fun <T> localErase(value: T): Any = value
        public fun imported(value: Unit): Any = erase(value)
        public fun local(value: Unit): Any = localErase(value)
        public fun nominal(value: Unit): Any = Holder<Unit>(value).erased()
        public fun adapter(sink: (Any) -> Unit): (Unit) -> Unit = adapt<Unit>(sink)
        "#,
        |output, _, _, _, _| {
            let local = output.output().local.module();
            let primitive_boxes = local
                .structs
                .values()
                .filter(|declaration| {
                    matches!(
                        local.types[declaration.canonical_type].kind,
                        hir::concrete::TypeKind::Unit
                            | hir::concrete::TypeKind::Integer(_)
                            | hir::concrete::TypeKind::Boolean
                    )
                })
                .collect::<Vec<_>>();
            assert_eq!(primitive_boxes.len(), 1);
            assert!(matches!(
                local.types[primitive_boxes[0].canonical_type].kind,
                hir::concrete::TypeKind::Unit
            ));
            hir::CanonicalHirFoundation::from_dependency_output(&output).unwrap();
        },
    )
    .unwrap_or_else(|errors| panic!("{errors:?}"));
}

#[test]
fn generic_primitive_arguments_without_boxing_do_not_create_nominal_roots() {
    with_provider_consumer(
        PROVIDER,
        "public fun unchanged(value: Unit): Unit = identity(value)",
        |output, _, _, _, _| {
            let local = output.output().local.module();
            assert!(local.structs.values().all(|declaration| {
                !matches!(
                    local.types[declaration.canonical_type].kind,
                    hir::concrete::TypeKind::Unit
                        | hir::concrete::TypeKind::Integer(_)
                        | hir::concrete::TypeKind::Boolean
                )
            }));
        },
    )
    .unwrap_or_else(|errors| panic!("{errors:?}"));
}
