use super::m23_ordinary_core_only::support::{parsed_ordinary, trusted_core};
use super::*;

const INVALID_REFERENCES: &str = r#"
import currentSourceLocation as location
fun inferred() { val reference = ::currentSourceLocation }
fun managed() { val reference: () -> SourceLocation = ::location }
fun native() { val reference: FunPtr<() -> Int> = ::location }
"#;

fn check_diagnostics(source: &str, diagnostics: &[Diagnostic]) {
    assert_eq!(diagnostics.len(), 3, "{diagnostics:?}");
    let references = source.match_indices("::").collect::<Vec<_>>();
    for (diagnostic, (start, _)) in diagnostics.iter().zip(references) {
        assert_eq!(
            diagnostic.span.expect("reference span").start as usize,
            start
        );
        assert!(diagnostic.message.starts_with(
            "compiler intrinsic `currentSourceLocation` does not support callable references"
        ));
        assert!(
            diagnostic
                .message
                .contains("indirect calls do not provide it")
        );
        assert!(!diagnostic.message.contains("@NoGC"));
        assert!(!diagnostic.message.contains("no applicable candidate"));
    }
}

#[test]
fn source_location_references_reject_local_intrinsic_in_all_contexts() {
    let source = INVALID_REFERENCES
        .replace("import currentSourceLocation as location", "")
        .replace("::location", "::currentSourceLocation");
    let diagnostics = lower_user(scoop_parser::parse(&source).unwrap()).unwrap_err();
    check_diagnostics(&source, &diagnostics);
}

#[test]
fn source_location_references_reject_imported_intrinsic_in_all_contexts() {
    let core = trusted_core();
    let parsed = parsed_ordinary(scoop_parser::parse(INVALID_REFERENCES).unwrap());
    let world = core.world(parsed.cone());
    let input = CurrentConeSources::try_new(
        &parsed,
        core.foundation.import_core_inputs(&core.interface).unwrap(),
        &world,
    )
    .unwrap();
    let diagnostics = lower_current_cone(scoop_identity::RequestedConeKind::Library, &input)
        .err()
        .expect("intrinsic references must fail");
    check_diagnostics(INVALID_REFERENCES, &diagnostics);
}

#[test]
fn source_location_references_allow_explicit_direct_call_wrappers() {
    let source = scoop_parser::parse(
        "fun wrapper(): SourceLocation = currentSourceLocation()\n\
         fun main() {\n\
             val ordinary: () -> SourceLocation = ::wrapper\n\
             val closure: () -> SourceLocation = { currentSourceLocation() }\n\
         }",
    )
    .unwrap();
    lower_user(source).expect("direct calls retain their own evaluation origins");
}

#[test]
fn source_location_references_do_not_restrict_same_named_user_function() {
    let source = scoop_parser::parse(
        "fun currentSourceLocation(): Int = 42\n\
         fun main() { val reference: () -> Int = ::currentSourceLocation }",
    )
    .unwrap();
    lower_user(source).expect("reference restrictions follow typed intrinsic identity");
}
