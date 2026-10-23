use super::wire_equivalence::compare_consumers;
use super::*;

#[test]
fn m57_companion_projection_targets_and_signatures_survive_wire_consumption() {
    compare_consumers(
        "conditional projection targets",
        r#"
        package dependency.api
        public interface Value<X> { public fun get(): X }
        public struct Present<X>() { public companion object {} }
        public struct Absent<X>()
        public open class Base<T> public constructor() {
            override interface Value<T.Companion> where T.Companion : ref {
                public open override fun get(): T.Companion = T.Companion
            }
        }
        public class Child<T> public constructor() : Base<T>()
        public class NoCompanion<X> public constructor() : Base<Absent<X>>()
        "#,
        r#"
        import dependency.api.*
        class Local<T> : Base<T>()
        class LocalAbsent : Base<Absent<Int>>() { public fun get(): Int = 5 }
        fun use(): Int {
            val a: Value<Present<Int>.Companion> = Child<Present<Int>>()
            val b: Value<Present<Int>.Companion> = Local<Present<Int>>()
            val x = a.get()
            val y = b.get()
            val plain = NoCompanion<Int>()
            return LocalAbsent().get()
        }
        "#,
    );
}

#[test]
fn m57_inherited_conditions_survive_wire_and_concrete_virtual_dispatch() {
    compare_consumers(
        "inherited conditional interfaces",
        r#"
        package dependency.api
        public interface Read<X> { public fun read(): Int }
        public interface Mark {}
        public struct Good<X>() : Read<Int> { public override fun read(): Int = 7 }
        public struct Empty<X>()
        public open class Base<T> public constructor(public val value: T) {
            override interface Read<Int> where T : Read<Int> {
                public open override fun read(): Int = value.read()
            }
            public open fun stable(): Int = 3
        }
        public class Derived<T> public constructor(value: T) : Base<T>(value)
        public class Relisted<T> public constructor(value: T) : Base<T>(value) {
            override interface Read<Int> where T : Read<Int> { public override fun read(): Int = 9 }
        }
        public abstract class Abstract<T> protected constructor() {
            override interface Read<Int> where T : Read<Int> { public abstract override fun read(): Int }
        }
        "#,
        r#"
        import dependency.api.*
        class Local<T>(value: T) : Base<T>(value) { public override fun read(): Int = 11 }
        class Completed<T> : Abstract<T>() { public override fun read(): Int = 12 }
        class NoObligation : Abstract<Empty<Int>>()
        fun use(): Int {
            val a: Read<Int> = Derived(Good<Int>())
            val b: Read<Int> = Relisted(Good<Int>())
            val c: Read<Int> = Local(Good<Int>())
            val d: Base<Empty<Int>> = Derived(Empty<Int>())
            val e = Completed<Empty<Int>>().read()
            val f = NoObligation()
            val g = Relisted(Empty<Int>()).stable()
            return a.read() + b.read() + c.read() + d.stable() + e + g
        }
        "#,
    );
}

#[test]
fn m57_companion_conditions_do_not_depend_on_static_interface_use() {
    with_provider_consumer(
        r#"
        package dependency.api
        public interface Mark {}
        public struct Seed() { public companion object : Mark {} }
        public struct Guard<T>(val value: T) {
            override interface Mark where T.Companion : Mark {}
        }
    "#,
        r#"
        import dependency.api.*
        fun value(): Any = Guard(Seed())
    "#,
        |output, _, _, _, _| {
            let local = output.output().local.module();
            let guard = local
                .structs
                .values()
                .find(|structure| structure.name == "Guard")
                .unwrap();
            assert_eq!(
                guard.interfaces.len(),
                1,
                "the complete interface set is independent of static conversions"
            );
        },
    )
    .unwrap();
}

#[test]
fn m57_imported_members_and_relations_require_the_original_condition() {
    for body in [
        "fun <T> invalid(box: Box<T>): Read<Int> = box",
        "fun <T> invalid(box: Box<T>): Int = box.read()",
        "fun <T> invalid(box: Box<T>): Int = box.size",
        "fun invalid(box: Box<Empty<Int>>): Read<Int> = box",
        "fun invalid(box: Box<Empty<Int>>): Int = box.read()",
        "fun invalid(box: Box<Empty<Int>>): Int = box.size",
        "fun invalid(): Int = Box<Empty<Int>>.read()",
        "fun <T> invalid(): Int = Box<T>.read()",
    ] {
        let source = format!("import dependency.api.*\n{body}");
        let result = with_provider_consumer(PROVIDER, &source, |output, _, _, _, _| output);
        assert!(result.is_err(), "{body}");
    }
}

#[test]
fn m57_disjoint_defaults_keep_distinct_dispatch_sources() {
    compare_consumers(
        "disjoint interface defaults",
        r#"
        package dependency.api
        public interface Root<X> { public fun read(): Int }
        public interface Left<X> : Root<X> { public override fun read(): Int = 1 }
        public interface Right<X> : Root<X> { public override fun read(): Int = 2 }
        public class Ref<T> public constructor(val value: T)
        public struct Select<T>(val value: T) {
            override interface Left<T> where T : value {}
            override interface Right<T> where T : ref {}
        }
    "#,
        r#"
        import dependency.api.*
        fun use(): Int {
            val a = Select(1).read()
            val b = Select(Ref(1)).read()
            val c: Root<Int> = Select(2)
            val d: Root<Ref<Int>> = Select(Ref(1))
            return d.read()
        }
    "#,
    );
}

const PROVIDER: &str = r#"
        package dependency.api
        public interface Read<X> { public fun read(): Int }
        public interface Sized<X> { public val size: Int }
        public interface Mark {}
        public struct Good<X>(val number: Int) : Read<Int> {
            public override fun read(): Int = number
            public companion object : Read<Int> { public override fun read(): Int = 12 }
        }
        public struct Empty<X>()
        public struct Box<T>(val value: T) {
            override interface Read<Int> where T : Read<Int> {
                public override fun read(): Int = value.read()
            }
            override interface Sized<Int> where T : Read<Int> {
                public override val size: Int get() = value.read()
            }
            override interface Mark where T : Read<Int> {}
            public companion object {
                override interface Read<Int> where T.Companion : Read<Int> {
                    public override fun read(): Int = T.read()
                }
            }
        }
        public class RefBox<T> public constructor(public val value: T) {
            override interface Read<Int> where T : Read<Int> {
                public override fun read(): Int = value.read()
            }
        }
        public enum Choice<T> {
            Item(val value: T)
            override interface Read<Int> where T : Read<Int> {
                public override fun read(): Int = 17
            }
        }
    "#;

#[test]
fn m57_conditional_interfaces_keep_their_domains_through_wire_consumption() {
    compare_consumers(
        "conditional interfaces",
        PROVIDER,
        r#"
        import dependency.api.*
        fun <T : Read<Int>> forward(box: Box<T>): Read<Int> = box
        fun <T> companionRead(value: T): Int where T.Companion : Read<Int> = T.read()
        fun use(): Int {
            val a = Box(Good<Int>(7)).read()
            val b = Box(Good<Int>(8)).size
            val c = forward(Box(Good<Int>(9))).read()
            val d = companionRead(Box(Good<Int>(10)))
            val e = RefBox(Good<Int>(11)).read()
            val f = Choice<Good<Int>>.Item(Good<Int>(12)).read()
            val nested: Read<Int> = Box(Box(Good<Int>(13)))
            val marker: Mark = Box(Good<Int>(14))
            val plain = Box(Empty<Int>())
            return nested.read()
        }
    "#,
    );
}
