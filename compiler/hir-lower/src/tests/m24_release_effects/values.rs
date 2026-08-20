use super::*;

#[test]
fn release_values_follow_representation_and_ignore_phantom_arguments() {
    with_source(
        r#"
        public struct Phantom<T>(val code: Int) {}
        public struct Value<T>(val value: T) {}
        public enum Choice<T> { Empty, Payload(T) }
        @NoGC public fun <T> phantom(value: Phantom<T>): Phantom<T> = value
        @NoGC public fun phantomManaged(value: Phantom<String>): Phantom<String> = value
        @NoGC public fun <T> payload(value: Value<T>): Value<T> = value
        @NoGC public fun <T> variants(value: Choice<T>): Choice<T> = value
        @NoGC public fun <T> pointer(value: Ptr<T>): Ptr<T> = value
        @NoGC public fun <T> pointerPhantom(value: Ptr<Phantom<T>>): Ptr<Phantom<T>> = value
        @NoGC public fun <T : value> layoutOnly(): ULong = sizeOf<T>()
    "#,
        |export| {
            for name in ["phantom", "phantomManaged", "pointerPhantom", "layoutOnly"] {
                available(export, name, &[]);
            }
            for name in ["payload", "variants", "pointer"] {
                available(export, name, &["T"]);
            }
        },
    );
}

#[test]
fn handles_callbacks_and_nested_protocol_values_remain_unavailable() {
    with_source(
        r#"
        public struct Wrapped(val handle: GcHandle<String>) {}
        public enum Hidden { Empty, Payload(GcHandle<String>) }
        @NoGC public fun handle(value: GcHandle<String>): GcHandle<String> = value
        @NoGC public fun pinned(value: PinnedPtr<String>): PinnedPtr<String> = value
        @NoGC public fun callable(value: FunPtr<(Int) -> Int>): FunPtr<(Int) -> Int> = value
        @NoGC public fun callback(value: ForeignCallback<(Int) -> Int>): ForeignCallback<(Int) -> Int> = value
        @NoGC public fun nested(value: Wrapped): Wrapped = value
        @NoGC public fun variants(value: Hidden): Hidden = value
        @NoGC public fun pointer(value: Ptr<Wrapped>): Ptr<Wrapped> = value
    "#,
        |export| {
            for name in [
                "handle", "pinned", "callable", "callback", "nested", "variants", "pointer",
            ] {
                unavailable(export, name);
            }
        },
    );
}

#[test]
fn protocol_restrictions_use_identity_and_not_display_names() {
    with_source(
        r#"
        public struct GcHandle<T>(val code: Int) {}
        public struct PinnedPtr<T>(val code: Int) {}
        public struct ForeignCallback<T>(val code: Int) {}
        @NoGC public fun handle(value: GcHandle<String>): GcHandle<String> = value
        @NoGC public fun pinned(value: PinnedPtr<String>): PinnedPtr<String> = value
        @NoGC public fun callback(value: ForeignCallback<String>): ForeignCallback<String> = value
    "#,
        |export| {
            for name in ["handle", "pinned", "callback"] {
                available(export, name, &[]);
            }
        },
    );
}

#[test]
fn recursive_pointer_representations_reach_finite_template_conditions() {
    with_source(
        r#"
        public struct Phantom<T>(val code: Int) {}
        public struct Link<T>(val next: Ptr<Link<Phantom<T>>>) {}
        public struct Payload<T>(val next: Ptr<Payload<T>>, val value: T) {}
        @NoGC public fun <T> link(value: Link<T>): Link<T> = value
        @NoGC public fun <T> payload(value: Payload<T>): Payload<T> = value
    "#,
        |export| {
            available(export, "link", &[]);
            available(export, "payload", &["T"]);
        },
    );
}
