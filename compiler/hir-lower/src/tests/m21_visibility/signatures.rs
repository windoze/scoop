use super::*;

const PROTOCOL: &str = r#"
    public interface Transform<T> {
        public fun apply(value: T): T
        public val current: T
    }
    struct Hidden(val value: Long)
    object TransformHidden : Transform<Hidden> {
        public override fun apply(value: Hidden): Hidden = value
        public override val current: Hidden get() = Hidden(7L)
    }
"#;

#[test]
fn a_public_override_keeps_its_owners_signature_access_domain() {
    lower_with_sysroot(&format!(
        r#"
        {PROTOCOL}
        private struct Local(val value: Long) {{
            public companion object : Transform<Local> {{
                public override fun apply(value: Local): Local = value
                public override val current: Local get() = Local(9L)
            }}
        }}
        fun main() {{
            val hidden: Transform<Hidden> = TransformHidden
            val local: Transform<Local> = Local.Companion
            println(hidden.apply(hidden.current).value)
            println(local.apply(local.current).value)
        }}
    "#
    ))
    .unwrap();
}

#[test]
fn a_public_api_cannot_expose_an_internal_type_argument() {
    let errors = lower_with_sysroot(&format!(
        "{PROTOCOL}\npublic fun leak(): Transform<Hidden> = TransformHidden\nfun main() {{}}"
    ))
    .unwrap_err();
    assert!(errors.iter().any(|error| {
        error.message
            == "signature of function `leak` exposes type `Hidden` outside its access domain"
    }));
}
