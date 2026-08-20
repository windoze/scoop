use super::*;

#[test]
fn no_transition_distinguishes_plain_static_storage_from_tls() {
    with_source(
        r#"
        @Global var global: Int = 7
        @ThreadLocal var local: Int = 8
        @Extern(name = "m24_global") @Global var external: Int
        @Extern(name = "m24_tls") @ThreadLocal var externalTls: Int
        @NoGC @Unsafe public fun globalRead(): Int = global
        @NoGC @Unsafe public fun externalRead(): Int = external
        @NoGC @Unsafe public fun tlsRead(): Int = local
        @NoGC @Unsafe public fun externalTlsRead(): Int = externalTls
        @NoGC @Unsafe public fun plainAddress(): Ptr<Int> = addressOf(external)
        @NoGC @Unsafe public fun tlsAddress(): Ptr<Int> = addressOf(externalTls)
        const val folded: Int = 8 / 2
        @NoGC public fun foldedDivision(): Int = folded
    "#,
        |export| {
            for name in [
                "globalRead",
                "externalRead",
                "plainAddress",
                "foldedDivision",
            ] {
                available(export, name, &[]);
            }
            for name in ["tlsRead", "externalTlsRead", "tlsAddress"] {
                unavailable(export, name);
            }
        },
    );
}

#[test]
fn no_transition_uses_static_call_graph_and_recursive_components() {
    with_source(
        r#"
        @NoGC public fun increment(value: Int): Int = value + 1
        @NoGC public fun first(value: Int): Int {
            if (value == 0) { return value }
            return second(value - 1)
        }
        @NoGC public fun second(value: Int): Int {
            if (value == 0) { return increment(value) }
            return first(value - 1)
        }
        public fun managed(value: Int): Int = increment(value)
        @Extern(name = "m24_native") public fun native(value: Int): Int
        @NoGC @Unsafe public fun transition(value: Int): Int = native(value)
        @NoGC @Unsafe public fun indirect(value: Int): Int = transition(value)
        @NoGC @Unsafe public fun poisonedA(value: Int): Int {
            if (value == 0) { return poisonedB(value) }
            return transition(value)
        }
        @NoGC @Unsafe public fun poisonedB(value: Int): Int = poisonedA(value)
    "#,
        |export| {
            for name in ["increment", "first", "second"] {
                available(export, name, &[]);
            }
            for name in [
                "managed",
                "native",
                "transition",
                "indirect",
                "poisonedA",
                "poisonedB",
            ] {
                unavailable(export, name);
            }
        },
    );
}

#[test]
fn generic_edges_substitute_only_required_value_parameters() {
    with_source(
        r#"
        public struct Phantom<T>(val code: Int) {}
        @NoGC public fun <T> identity(value: T): T = value
        @NoGC public fun <T, U> forward(value: T, ignored: Phantom<U>): T = identity(value)
        @NoGC public fun <T, U> recursiveA(value: T, ignored: Phantom<U>): T {
            if (ignored.code == 0) { return value }
            return recursiveB(value, ignored)
        }
        @NoGC public fun <T, U> recursiveB(value: T, ignored: Phantom<U>): T = recursiveA(value, ignored)
        @NoGC public fun concrete(value: Int): Int = identity(value)
    "#,
        |export| {
            available(export, "identity", &["T"]);
            available(export, "forward", &["T"]);
            available(export, "recursiveA", &["T"]);
            available(export, "recursiveB", &["T"]);
            available(export, "concrete", &[]);
        },
    );
}

#[test]
fn struct_secondary_constructors_participate_in_the_same_graph() {
    with_source(
        r#"
        @NoGC public fun <T> identity(value: T): T = value
        public struct Native<T>(val value: T) {
            @NoGC public constructor(value: T, flag: Boolean): this(identity(value)) {}
        }
        @NoGC public fun <T> construct(value: T): Native<T> = Native(value, true)
    "#,
        |export| {
            available(export, "construct", &["T"]);
            let constructor = export
                .struct_constructors
                .values()
                .find(|constructor| {
                    matches!(
                        constructor.kind,
                        hir::StructConstructorKind::Secondary { .. }
                    )
                })
                .unwrap();
            let hir::ReleaseCallability::NoTransition { requirements } =
                &constructor.release_callability
            else {
                panic!("secondary value constructor has a release effect");
            };
            assert_eq!(
                requirements,
                &[export.structs[constructor.owner].type_params[0].id]
            );
        },
    );
}
