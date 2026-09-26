use super::*;
use scoop_identity::{
    CallableTemplateOrigin as Origin, DependencyCallableDeclarationId as Declaration,
    SourceDeclarationKey,
};

#[test]
fn shared_callable_selection_uses_source_owners_signatures_visibility_and_abstract_overrides() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m23-core-layout-exports");
    for (case, count) in [("standalone", 2), ("combined", 12)] {
        let source =
            std::fs::read_to_string(directory.join(format!("shared-callables-{case}.scoop")))
                .unwrap();
        source_dispatch::with_hir_source(&source, |output, core| {
            let public = public_interface(output);
            let types = produce_cross_cone_type_semantics(output, &public).unwrap();
            let identities = source_inventory::identity_closure(output);
            let provider = output.output().export.cone;
            let selected =
                hir::select_param_free_source_callables(provider, &public, &types, &identities)
                    .unwrap();
            assert_eq!(selected.len(), count, "{case}");
            let world = core.world(provider);
            let classifier = world
                .nominal_exact_leaf_classifier(public.nominal_interfaces())
                .unwrap();
            let ordinary =
                hir::select_ordinary_source_callables(provider, &public, &classifier, &identities)
                    .unwrap();
            assert_eq!(ordinary.len(), if case == "standalone" { 3 } else { 11 });
            let mut rows = Vec::new();
            for source in public.callable_interfaces().all_declarations() {
                let (key, included, old) = match source.declaration() {
                    Origin::Function(id) => (
                        identities
                            .canonical_key::<_, SourceDeclarationKey>(id)
                            .unwrap(),
                        selected.contains_key(&Declaration::Function(id)),
                        ordinary.contains_key(&Declaration::Function(id)),
                    ),
                    Origin::GenericFunction(id) => (
                        identities
                            .canonical_key::<_, SourceDeclarationKey>(id)
                            .unwrap(),
                        false,
                        false,
                    ),
                    _ => continue,
                };
                let owner = match source.owner().nominal_owner() {
                    Some(owner) => declaration_dump::nominal(owner, &identities),
                    None => format!("{:?}", source.owner()),
                };
                rows.push(format!(
                    "{owner}.{}: {:?} {:?} {:?} slots={} layout-eligible={included} ordinary={old} ({}) -> {}\n",
                    declaration_dump::named(&key),
                    source.declared_visibility(),
                    source.modality(),
                    source.effects().gc_effect(),
                    source.slot_relations().values().len(),
                    source
                        .parameters()
                        .parameters()
                        .iter()
                        .map(|parameter| declaration_dump::ty(parameter.value_type(), &identities))
                        .collect::<Vec<_>>()
                        .join(","),
                    declaration_dump::ty(source.result(), &identities),
                ));
            }
            rows.sort();
            let snapshot = directory.join(format!("shared-callables-{case}.hir.snap"));
            if std::env::var_os("SCOOP_UPDATE_SHARED_CALLABLE_SELECTION").is_some() {
                std::fs::write(&snapshot, rows.concat()).unwrap();
            }
            assert_eq!(rows.concat(), std::fs::read_to_string(snapshot).unwrap());
            let foreign = hir::select_param_free_source_callables(
                ConeIdentity::CORE,
                &public,
                &types,
                &identities,
            )
            .unwrap();
            assert!(
                foreign.is_empty(),
                "a foreign source declaration cannot become a local body"
            );
        });
    }
}
