use super::*;
use scoop_hir as hir;

const ALIASES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/core-library/type-aliases.scoop"
));
const CONSUMER: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/core-library/alias-consumer.scoop"
));

fn with_aliases<R>(source: &str, run: impl FnOnce(&CurrentConeSources<'_, '_>) -> R) -> R {
    let mut core_source = crate::tests::complete_core_file();
    core_source
        .declarations
        .extend(scoop_parser::parse(ALIASES).unwrap().declarations);
    let core = support::trusted_core_from_source(core_source, ALIASES);
    let parsed = support::parsed_ordinary_text(source);
    let world = core.world(parsed.cone());
    let protocols = core.foundation.import_core_inputs(&core.interface).unwrap();
    let input = CurrentConeSources::try_new(&parsed, protocols, &world).unwrap();
    run(&input)
}

#[test]
fn imported_core_aliases_use_shared_type_selection_and_separate_value_names() {
    with_aliases(CONSUMER, |input| {
        let output = lower_current_cone(scoop_identity::RequestedConeKind::Library, input).unwrap();
        assert_eq!(output.imported_dependencies().type_alias_count(), 2);
        assert_eq!(output.imported_dependencies().callable_count(), 1);
        let [alias_witness] = output.binding_witness_uses() else {
            panic!("the public facade alias retains one imported alias route")
        };
        assert!(matches!(
            alias_witness.target(),
            hir::ExternalHirTargetV1::TypeAlias(_)
        ));
        assert_eq!(
            alias_witness.witness().route().immediate_provider(),
            ConeIdentity::CORE
        );
        assert_eq!(
            hir::dump(&output.output().export),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/core-library/alias-consumer.hir"
            ))
        );
    });
}

#[test]
fn current_type_alias_shadows_the_imported_core_type_namespace() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/core-library/alias-shadow.scoop"
    ));
    with_aliases(source, |input| {
        let output = lower_current_cone(scoop_identity::RequestedConeKind::Library, input).unwrap();
        assert_eq!(output.imported_dependencies().type_alias_count(), 0);
        let module = output.output().export.module();
        let alias = module.type_aliases.iter().next().unwrap().1;
        assert!(matches!(module.types[alias.target], hir::Type::Boolean));
    });
}

#[test]
fn imported_core_alias_rejects_type_arguments_at_its_source_name() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/core-library/alias-generic-error.scoop"
    ));
    with_aliases(source, |input| {
        let errors = lower_current_cone(scoop_identity::RequestedConeKind::Library, input)
            .err()
            .expect("generic alias use must fail");
        assert_eq!(errors.len(), 1);
        assert_eq!(
            errors[0].message,
            "typealias `UserCoreNumber` is not generic"
        );
        let start = source.find("UserCoreNumber").unwrap() as u32;
        assert_eq!(
            errors[0].span,
            Some(scoop_ast::Span {
                start,
                end: start + "UserCoreNumber".len() as u32
            })
        );
    });
}

#[test]
fn imported_builtin_type_rejects_type_arguments_at_its_source_name() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/core-library/builtin-generic-error.scoop"
    ));
    with_aliases(source, |input| {
        let errors = lower_current_cone(scoop_identity::RequestedConeKind::Library, input)
            .err()
            .expect("non-generic nominal use must fail");
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].message, "type `Int` is not generic");
        let start = source.find("Int<").unwrap() as u32;
        assert_eq!(
            errors[0].span,
            Some(scoop_ast::Span {
                start,
                end: start + 3
            })
        );
    });
}

#[test]
fn builtin_type_names_require_shared_public_bindings() {
    let core = trusted_core();
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/core-library/builtin-binding-error.scoop"
    ));
    let parsed = support::parsed_ordinary_text(source);
    let world =
        hir::ImportedSemanticWorld::from_dependencies(parsed.cone(), Vec::new(), Vec::new())
            .unwrap();
    let protocols = core.foundation.import_core_inputs(&core.interface).unwrap();
    let input = CurrentConeSources::try_new(&parsed, protocols, &world).unwrap();
    let errors = lower_current_cone(scoop_identity::RequestedConeKind::Library, &input)
        .err()
        .expect("a protocol identity does not publish a source-level name");
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].message, "unknown type `Int`");
    let start = source.find("Int").unwrap() as u32;
    assert_eq!(
        errors[0].span,
        Some(scoop_ast::Span {
            start,
            end: start + 3
        })
    );
}
