use super::m23_ordinary_core_only::support::{parsed_ordinary_text, trusted_core};
use crate::{CurrentConeSources, lower_current_cone};
use scoop_hir as hir;

mod contracts;
mod generics;

fn with_source(source: &str, verify: impl FnOnce(&hir::ExportHir)) {
    let core = trusted_core();
    let parsed = parsed_ordinary_text(source);
    let world = core.world(parsed.cone());
    let input = CurrentConeSources::try_new(
        &parsed,
        core.foundation.import_core_inputs(&core.interface).unwrap(),
        &world,
    )
    .unwrap();
    let output = lower_current_cone(scoop_identity::RequestedConeKind::Library, &input)
        .unwrap_or_else(|errors| panic!("{errors:#?}\n{source}"));
    verify(output.output().export.module());
}

fn rejects(source: &str, expected: &str) {
    let core = trusted_core();
    let parsed = parsed_ordinary_text(source);
    let world = core.world(parsed.cone());
    let input = CurrentConeSources::try_new(
        &parsed,
        core.foundation.import_core_inputs(&core.interface).unwrap(),
        &world,
    )
    .unwrap();
    let errors = lower_current_cone(scoop_identity::RequestedConeKind::Library, &input)
        .err()
        .expect("invalid context source must be rejected");
    assert!(
        errors.iter().any(|error| error.message.contains(expected)),
        "{errors:#?}\n{source}"
    );
}

#[test]
fn reference_keys_keep_symbolic_types_and_named_contexts_out_of_value_parameters() {
    with_source(
        r#"
        public class Request(val id: Int)
        context(request: Request, _: String)
        public fun read(offset: Int = 1): Int = request.id + offset
        context(value: T)
        fun <T: ref> identity(): T = value
        context(values: Array<T>)
        fun <T> first(): T = values[0]
    "#,
        |export| {
            let function = export
                .functions
                .values()
                .find(|function| function.name == "read")
                .unwrap();
            assert_eq!(function.params.len(), 1);
            assert_eq!(function.context_parameters.len(), 2);
            let declarations = hir::CanonicalCallableInterfacesV1::from_export_hir(export).unwrap();
            let record = declarations
                .records()
                .iter()
                .find(|record| record.context_parameters().len() == 2)
                .unwrap();
            assert_eq!(record.context_parameters()[0].name().as_str(), "request");
            assert_eq!(record.context_parameters()[1].name().as_str(), "_");
            assert_eq!(record.parameters().parameters().len(), 1);
        },
    );
}

#[test]
fn context_keys_require_non_null_managed_references() {
    for key in [
        "Int",
        "String?",
        "(Int, Int)",
        "Ptr<Int>",
        "FunPtr<() -> Unit>",
    ] {
        rejects(
            &format!("context(value: {key}) fun read() {{}}"),
            "context key must be a non-null managed reference type",
        );
    }
    rejects(
        "context(value: T) fun <T> read() {}",
        "context key must be a non-null managed reference type",
    );
    rejects(
        "interface Service {}\ncontext(value: T) fun <T: Service> read() {}",
        "context key must be a non-null managed reference type",
    );
}

#[test]
fn context_names_and_keys_are_distinct_and_entry_locals_are_immutable() {
    rejects(
        "context(value: String) fun read(value: Int) {}",
        "duplicate context or value parameter",
    );
    rejects(
        "context(a: String, b: String) fun read() {}",
        "duplicate context key",
    );
    rejects(
        "typealias Text = String\ncontext(a: String, b: Text) fun read() {}",
        "duplicate context key",
    );
    rejects(
        "context(value: String) fun read() { value = \"changed\" }",
        "immutable",
    );
    rejects(
        "context(value: String) fun read(text: String = value) {}",
        "value",
    );
}

#[test]
fn contexts_obey_signature_visibility_and_no_gc_effects() {
    rejects(
        "private class Secret\ncontext(secret: Secret) public fun read() {}",
        "exposes type `Secret`",
    );
    rejects(
        "context(value: String) @NoGC fun read() {}",
        "contextual declarations cannot be annotated",
    );
    rejects(
        "@NoGC fun read(value: String) { context(value) {} }",
        "NoGC",
    );
}
