use super::*;
use scoop_identity::{
    CallableTemplateOrigin as Origin, DependencyCallableDeclarationId as Declaration,
    SourceDeclarationKey,
};

#[test]
fn shared_callable_selection_uses_source_owners_signatures_visibility_and_abstract_overrides() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/m23-core-layout-exports");
    for (case, count) in [("standalone", 2), ("combined", 14)] {
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
            assert_eq!(ordinary.len(), if case == "standalone" { 3 } else { 12 });
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
                let generic = matches!(source.declaration(), Origin::GenericFunction(_));
                assert_eq!(
                    included,
                    !generic && source.declared_visibility() != hir::DeclaredVisibilityV1::Private,
                    "{case}: {key:?}",
                );
                assert_eq!(
                    old,
                    !generic && source.modality() != hir::CallableModalityV1::Abstract,
                    "{case}: {key:?}",
                );
                assert_eq!(
                    source.slot_relations().values().len(),
                    usize::from(source.modality() != hir::CallableModalityV1::Final),
                );
                let [parameter] = source.parameters().parameters() else {
                    panic!("the fixture functions have one source parameter");
                };
                assert_eq!(parameter.value_type(), source.result());
                assert_eq!(
                    source.effects().gc_effect(),
                    if declaration_dump::named(&key) == "keep" {
                        scoop_identity::GcEffect::NoGc
                    } else {
                        scoop_identity::GcEffect::Managed
                    },
                );
            }
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
