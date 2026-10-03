use super::*;
use scoop_identity::CallableTemplateOrigin;

#[test]
fn shared_constructor_selection_includes_materializable_support_bodies() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m23-core-layout-exports");
    for (case, count) in [("standalone", 4), ("combined", 17)] {
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
            let mut visibilities = [0; 4];
            let mut no_gc = 0;
            for source in public.callable_interfaces().all_declarations() {
                let CallableTemplateOrigin::Constructor(id) = source.declaration() else {
                    continue;
                };
                let owner = source.owner().nominal_owner().unwrap();
                assert_eq!(
                    selected.contains_key(&id),
                    matches!(owner, hir::SourceNominalId::Concrete(_)),
                );
                if selected.contains_key(&id) {
                    assert_eq!(selected[&id], source);
                    visibilities[match source.declared_visibility() {
                        hir::DeclaredVisibilityV1::Public => 0,
                        hir::DeclaredVisibilityV1::Protected => 1,
                        hir::DeclaredVisibilityV1::Internal => 2,
                        hir::DeclaredVisibilityV1::Private => 3,
                    }] += 1;
                    no_gc +=
                        usize::from(source.effects().gc_effect() == scoop_identity::GcEffect::NoGc);
                }
            }
            assert_eq!(no_gc, 1);
            assert_eq!(
                visibilities,
                if case == "standalone" {
                    [4, 0, 0, 0]
                } else {
                    [13, 1, 1, 2]
                }
            );
            assert!(select(ConeIdentity::CORE).unwrap().is_empty());
        });
    }
}
