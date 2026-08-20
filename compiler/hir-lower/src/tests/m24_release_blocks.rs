use scoop_hir as hir;

use super::m23_ordinary_core_only::support::{parsed_ordinary_text, trusted_core};
use crate::{CurrentConeSources, lower_current_cone};

fn lower(source: &str) -> Result<hir::DependencyHirOutput, Vec<scoop_ast::Diagnostic>> {
    let core = trusted_core();
    let parsed = parsed_ordinary_text(source);
    let world = core.world(parsed.cone());
    let input = CurrentConeSources::try_new(
        &parsed,
        core.foundation.import_core_inputs(&core.interface).unwrap(),
        &world,
    )
    .unwrap();
    lower_current_cone(scoop_identity::RequestedConeKind::Library, &input)
}

fn rejected(source: &str, message: &str) {
    let errors = lower(source).err().expect(source);
    assert!(
        errors.iter().any(|error| error.message.contains(message)),
        "{errors:#?}\n{source}"
    );
}

#[test]
fn hook_uses_own_storage_and_locals_without_a_managed_receiver() {
    let output = lower(
        r#"
        @Extern(name = "m24_drop") fun drop(value: Int)
        public class Owner(private var handle: Int) {
            public val custom: Int = 7
                get() = handle + 10
            release {
                val saved = custom
                var handle = handle
                handle += saved
                @Unsafe { drop(handle) }
            }
        }
    "#,
    )
    .unwrap();
    let export = output.output().export.module();
    assert_eq!(export.release_hooks.len(), 1);
    let dump = hir::dump_module(export);
    assert!(dump.contains("ReleaseFieldLoad"), "{dump}");
    let concrete = output.output().local.module();
    assert_eq!(concrete.release_hooks.len(), 1);
    let (_, hook) = concrete.release_hooks.iter().next().unwrap();
    assert!(!hook.body.locals.values().any(|local| local.name == "this"));
}

#[test]
fn release_owner_restrictions_and_duplicate_blocks_are_diagnosed() {
    for source in [
        "open class Owner { release {} }",
        "abstract class Owner { release {} }",
    ] {
        rejected(source, "ordinary final class");
    }
    rejected(
        "class Owner { release {}\n release {} }",
        "only one `release`",
    );
    rejected("object Owner { release {} }", "not allowed in object");
    rejected("class Owner: Throwable { release {} }", "Throwable subtype");
}

#[test]
fn release_rejects_receiver_escape_field_mutation_and_control_transfers() {
    for (body, message) in [
        ("val leaked = this", "`this`"),
        ("handle = 2", "read-only"),
        ("handle += 2", "read-only"),
        ("handle++", "read-only"),
        ("@Unsafe { addressOf(handle) }", "cannot take the address"),
        ("return", "`return`"),
        ("fun nested() {}", "local function"),
        ("val closure = { -> 1 }", "lambda"),
        ("val reference = ::helper", "callable reference"),
        ("try {} finally {}", "`try`"),
    ] {
        rejected(
            &format!(
                "fun helper() {{}} class Owner(private var handle: Int) {{ release {{ {body} }} }}"
            ),
            message,
        );
    }
}

#[test]
fn release_rejects_callable_references_inside_expanded_defaults() {
    rejected(
        r#"
        fun target(): Int = 1
        fun invoke(value: () -> Int): Int = value()
        @NoGC fun helper(value: Int = invoke(::target)): Int = value
        class Owner { release { helper() } }
        "#,
        "callable reference",
    );
}

#[test]
fn release_checks_expanded_values_and_helper_effects() {
    for body in [
        "val value = text",
        "val value = handle / 2",
        "val value = ordinary(handle)",
        "val value = transitions(handle)",
        "withDefault()",
    ] {
        rejected(
            &format!(
                r#"
            @Extern(name = "m24_read") fun native(value: Int): Int
            fun ordinary(value: Int): Int = value
            @NoGC @Unsafe fun transitions(value: Int): Int = native(value)
            @NoGC fun withDefault(value: Int = ordinary(3)): Int = value
            class Owner(private val handle: Int, private val text: String) {{
                release {{ @Unsafe {{ {body} }} }}
            }}
        "#
            ),
            "ReleaseValue values and direct NoTransition calls",
        );
    }
}

#[test]
fn release_value_conditions_apply_to_type_only_uses() {
    let definition = "public class Owner<T>(private val value: T) { release { val copy = value } }";
    rejected(
        &format!("{definition} fun onlyType(value: Owner<String>) {{}}"),
        "requires ReleaseValue",
    );
    let output = lower(&format!(
        "{definition} public fun onlyType(value: Owner<Int>) {{}}"
    ))
    .unwrap();
    assert_eq!(
        output
            .output()
            .export
            .module()
            .release_hooks
            .iter()
            .next()
            .unwrap()
            .1
            .requirements
            .len(),
        1
    );
    rejected(
        "class Owner<T: ref>(private val value: T) { release { val copy = value } }",
        "reference bound",
    );
}
