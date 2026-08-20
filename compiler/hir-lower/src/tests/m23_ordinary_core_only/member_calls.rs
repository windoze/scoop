use super::*;
use scoop_hir as hir;

#[test]
fn imported_core_members_normalize_through_shared_source_candidates() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-imported-core-members/methods.scoop"
    ));
    constants::with_input(source, |input| {
        let output = lower_current_cone(scoop_identity::RequestedConeKind::Library, input)
            .unwrap_or_else(|errors| panic!("{errors:#?}"));
        let export = output.output().export.module();
        assert!(
            export
                .functions
                .iter()
                .all(|(_, function)| !matches!(function.kind, hir::FunctionKind::Intrinsic(_)))
        );
        assert!(output.imported_dependencies().is_empty());
    });
}

fn fixture(name: &str) -> String {
    std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/m23-imported-core-members")
            .join(name),
    )
    .unwrap()
}

#[test]
fn imported_managed_integer_operation_materializes_generic_exception_storage() {
    let source = fixture("managed.scoop");
    constants::with_input(&source, |input| {
        let output = lower_current_cone(scoop_identity::RequestedConeKind::Library, input)
            .unwrap_or_else(|errors| panic!("{errors:#?}"));
        let local = output.output().local.module();
        assert!(
            local
                .classes
                .iter()
                .any(|(_, class)| class.name == "ArithmeticException")
        );
        let (_, option) = local
            .enums
            .iter()
            .find(|(_, enumeration)| enumeration.name == "Option")
            .expect("the exception's message storage retains its generic application");
        assert_eq!(option.type_arguments.len(), 1);
        assert!(
            local
                .exact_type_identities
                .nominal_specialization(option.canonical_type)
                .is_some()
        );
    });
}
