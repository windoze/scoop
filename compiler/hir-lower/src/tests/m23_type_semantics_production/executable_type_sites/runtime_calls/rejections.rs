use super::*;
use hir::{HirDependencyCallSiteV1, HirRuntimeConstructorError as Error};
use scoop_identity::{CallableTemplateOrigin, CoreBuiltinNominal, ExactTypeKey, SignatureTypeKey};

#[test]
fn runtime_constructor_source_and_role_reject_wrong_provider_owner_and_target() {
    with_fixture(
        "runtime-cast-standalone",
        "runtime-cast-exception",
        |output, core| {
            let public = public_projection::public_interface_with_core(output, core);
            with_metadata(output, &public, core, |metadata, dependency| {
                let (reference, site) = calls(&public).next().unwrap();
                let check = |target, provider, site: &HirDependencyCallSiteV1| {
                    site.runtime_constructor_source(
                        target,
                        provider,
                        dependency.identities,
                        dependency.public,
                    )
                };
                assert_eq!(
                    check(reference.target(), metadata.provider, site),
                    Err(Error::Owner)
                );
                let unit = PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
                    CoreBuiltinNominal::Unit.identity_record().id(),
                ))
                .unwrap();
                let changed = HirDependencyCallSiteV1::try_new_with_reason(
                    site.position(),
                    site.origin().clone(),
                    vec![],
                    unit,
                    site.reason().clone(),
                    scoop_hir::SourceCallReceiver::NoReceiver,
                )
                .unwrap();
                assert_eq!(
                    check(reference.target(), dependency.provider, &changed),
                    Err(Error::Owner)
                );
                let other = dependency
                    .public
                    .callable_interfaces()
                    .all_declarations()
                    .find(|callable| {
                        matches!(
                            callable.declaration(),
                            CallableTemplateOrigin::Constructor(_)
                        ) && callable.parameters().parameters().is_empty()
                            && hir::ExternalHirTargetV1::Callable(callable.declaration())
                                != reference.target()
                    })
                    .unwrap();
                let SignatureTypeKey::Nominal(owner) = other.result() else {
                    panic!("constructor result")
                };
                let other_result =
                    PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(*owner)).unwrap();
                let changed = HirDependencyCallSiteV1::try_new_with_reason(
                    site.position(),
                    site.origin().clone(),
                    vec![],
                    other_result,
                    site.reason().clone(),
                    scoop_hir::SourceCallReceiver::NoReceiver,
                )
                .unwrap();
                let target = hir::ExternalHirTargetV1::Callable(other.declaration());
                check(target, dependency.provider, &changed).unwrap();
                assert_eq!(
                    changed.validate_runtime_constructor_role(target, *owner, &core.interface),
                    Err(Error::RoleTarget)
                );
                assert_eq!(
                    site.validate_runtime_constructor_role(
                        reference.target(),
                        *owner,
                        &core.interface
                    ),
                    Err(Error::RoleSignature)
                );
            });
        },
    );
}

#[test]
fn runtime_constructor_selection_rejects_missing_extra_and_wrong_construction_claims() {
    with_fixture(
        "runtime-cast-combined",
        "runtime-cast-adapter-exception",
        |output, core| {
            let public = public_projection::public_interface_with_core(output, core);
            with_metadata(output, &public, core, |metadata, dependency| {
                let selected = metadata.materialized_type_uses(&[dependency]).unwrap();
                let records = selected.records();
                let index = records
                    .iter()
                    .position(|record| {
                        matches!(record.usage(), SelectedTypeUseV1::Construct { .. })
                    })
                    .unwrap();
                let check = |records| {
                    let changed =
                        hir::CanonicalSelectedExternalTypeUsesV1::try_new(records).unwrap();
                    assert!(matches!(
                        metadata.validate_materialized_type_uses(&changed, &[dependency]),
                        Err(hir::SharedTypeMetadataError::TypeUseInventory)
                    ));
                };
                let mut missing = records.to_vec();
                missing.remove(index);
                check(missing);
                let mut wrong = records.to_vec();
                wrong[index] =
                    hir::SelectedExternalTypeUseV1::new(metadata.provider, records[index].usage());
                check(wrong);
                let mut extra = records.to_vec();
                extra.push(hir::SelectedExternalTypeUseV1::new(
                    metadata.provider,
                    records[index].usage(),
                ));
                check(extra);

                metadata.materialized_type_uses(&[dependency]).unwrap();
            });
        },
    );
}
