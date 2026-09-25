use super::*;

#[test]
fn runtime_cast_relations_require_both_actual_type_sites_and_the_original_origin() {
    with_fixture(
        "runtime-cast-combined",
        "runtime-cast-exception",
        |output, core| {
            let public = public_projection::public_interface_with_core(output, core);
            let sites = calls(&public).collect::<Vec<_>>();
            assert!(sites.len() >= 3);
            let original = sites[0].1;
            let different = sites
                .iter()
                .find(|(_, site)| site.origin() != original.origin())
                .unwrap()
                .1;
            let position = original.position();
            with_metadata(output, &public, core, |metadata, dependency| {
                let check = |references| {
                    let references =
                        hir::CanonicalExternalHirReferencesV1::try_new(references).unwrap();
                    assert!(matches!(references.validate_type_site_relations(
                    metadata.provider, metadata.identities, &[dependency.provider],
                ), Err(hir::HirDependencyTypeRelationError::CallResult(actual)) if actual == position));
                };
                for role in [
                    HirExpressionTypeRoleV1::Value,
                    HirExpressionTypeRoleV1::TypeTest,
                ] {
                    let references = public
                        .external_references()
                        .records()
                        .iter()
                        .map(|reference| {
                            let types = reference
                                .type_sites()
                                .records()
                                .iter()
                                .filter(|site| {
                                    !site.as_expression().is_some_and(|site| {
                                        site.position() == position && site.role() == role
                                    })
                                })
                                .cloned()
                                .collect();
                            hir::ExternalHirReferenceV1::try_new(
                                reference.origin(),
                                reference.target(),
                                reference.roles().clone(),
                                reference.witnesses().clone(),
                                reference.call_sites().clone(),
                                hir::CanonicalHirDependencyTypeSitesV1::try_new(types).unwrap(),
                            )
                            .unwrap()
                        })
                        .collect();
                    check(references);
                }
                let references = public
                    .external_references()
                    .records()
                    .iter()
                    .map(|reference| {
                        let sites = reference
                            .call_sites()
                            .records()
                            .iter()
                            .map(|site| {
                                if site.position() == position {
                                    hir::HirDependencyCallSiteV1::try_new_with_reason(
                                        position,
                                        different.origin().clone(),
                                        vec![],
                                        site.result(),
                                        site.reason().clone(),
                                        scoop_hir::SourceCallReceiver::NoReceiver,
                                    )
                                    .unwrap()
                                } else {
                                    site.clone()
                                }
                            })
                            .collect();
                        hir::ExternalHirReferenceV1::try_new(
                            reference.origin(),
                            reference.target(),
                            reference.roles().clone(),
                            reference.witnesses().clone(),
                            hir::CanonicalHirDependencyCallSitesV1::try_new(sites).unwrap(),
                            reference.type_sites().clone(),
                        )
                        .unwrap()
                    })
                    .collect();
                check(references);
            });
        },
    );
}
