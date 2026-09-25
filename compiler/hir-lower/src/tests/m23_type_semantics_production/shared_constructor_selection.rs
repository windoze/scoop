use super::*;
use scoop_identity::CallableTemplateOrigin;

#[test]
fn shared_constructor_selection_keeps_only_required_source_constructor_bodies() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m23-core-layout-exports");
    for (case, count) in [("standalone", 4), ("combined", 12)] {
        let source =
            std::fs::read_to_string(directory.join(format!("shared-constructors-{case}.scoop")))
                .unwrap();
        source_dispatch::with_hir_source(&source, |output, _| {
            let public = public_interface(output);
            let identities = source_inventory::identity_closure(output);
            let select = |provider| {
                hir::select_param_free_source_constructors(provider, &public, &identities)
            };
            let selected = select(output.output().export.cone).unwrap();
            assert_eq!(selected.len(), count, "{case}");
            let mut rows = Vec::new();
            for source in public.callable_interfaces().all_declarations() {
                let CallableTemplateOrigin::Constructor(id) = source.declaration() else {
                    continue;
                };
                let owner =
                    declaration_dump::nominal(source.owner().nominal_owner().unwrap(), &identities);
                let parameters = source
                    .parameters()
                    .parameters()
                    .iter()
                    .map(|parameter| declaration_dump::ty(parameter.value_type(), &identities))
                    .collect::<Vec<_>>()
                    .join(",");
                rows.push(format!(
                    "{owner}({parameters}): {:?} {:?} exported={}\n",
                    source.declared_visibility(),
                    source.effects().gc_effect(),
                    selected.contains_key(&id)
                ));
            }
            rows.sort();
            let snapshot = directory.join(format!("shared-constructors-{case}.hir.snap"));
            if std::env::var_os("SCOOP_UPDATE_SHARED_CONSTRUCTOR_SELECTION").is_some() {
                std::fs::write(&snapshot, rows.concat()).unwrap();
            }
            assert_eq!(rows.concat(), std::fs::read_to_string(snapshot).unwrap());
            assert!(select(ConeIdentity::CORE).unwrap().is_empty());
        });
    }
}
