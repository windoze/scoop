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
        assert!(output.binding_witness_uses().is_empty());
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
