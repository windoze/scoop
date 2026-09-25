use super::*;
use hir::{HirDependencyCallSiteV1, HirRuntimeConstructorError as Error};
use scoop_identity::{
    CallableTemplateOrigin, CborIdentityRecord, ExactTypeKey, GeneratedCallableKey,
    PendingIdentityValidation, PersistentGeneratedCallableId, SignatureTypeKey,
};

#[test]
fn runtime_adapter_requires_complete_defaultable_parameters_and_its_generated_role() {
    with_fixture(
        "runtime-cast-standalone",
        "runtime-cast-adapter-exception",
        |output, core| {
            let public = public_projection::public_interface_with_core(output, core);
            with_metadata(output, &public, core, |_, dependency| {
                let (reference, site) = calls(&public).next().unwrap();
                let resolve = |target,
                               site: &HirDependencyCallSiteV1,
                               graph: &scoop_identity::ValidatedIdentityGraph,
                               meter: &mut BudgetMeter| {
                    site.runtime_constructor_source(
                        target,
                        dependency.provider,
                        graph,
                        dependency.public,
                        meter,
                    )
                };
                let (constructor, _) = resolve(
                    reference.target(),
                    site,
                    dependency.identities,
                    &mut meter(),
                )
                .unwrap();
                let direct = hir::ExternalHirTargetV1::Callable(
                    CallableTemplateOrigin::Constructor(constructor),
                );
                assert_eq!(
                    resolve(direct, site, dependency.identities, &mut meter()),
                    Err(Error::Parameters)
                );
                let required = dependency
                    .public
                    .source_interfaces()
                    .records()
                    .iter()
                    .find(|source| {
                        matches!(source.owner(), CallableTemplateOrigin::Constructor(_))
                            && source.parameters().parameters().iter().any(|parameter| {
                                matches!(
                                    parameter.calling(),
                                    hir::CallableParameterCallingV1::Required
                                )
                            })
                    })
                    .unwrap();
                let CallableTemplateOrigin::Constructor(constructor) = required.owner() else {
                    unreachable!("the selected source is a constructor")
                };
                let declaration = dependency
                    .public
                    .callable_interfaces()
                    .declaration(required.owner())
                    .unwrap();
                let SignatureTypeKey::Nominal(owner) = declaration.result() else {
                    panic!("source constructor result")
                };
                let result =
                    PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(*owner)).unwrap();
                let changed = HirDependencyCallSiteV1::try_new_with_reason(
                    site.position(),
                    site.origin().clone(),
                    vec![],
                    result,
                    site.reason().clone(),
                    scoop_hir::SourceCallReceiver::NoReceiver,
                )
                .unwrap();
                let adapter = CborIdentityRecord::<PersistentGeneratedCallableId, _>::from_key(
                    GeneratedCallableKey::ZeroArgumentConstructorAdapter { constructor },
                )
                .unwrap();
                let unrelated = CborIdentityRecord::<PersistentGeneratedCallableId, _>::from_key(
                    GeneratedCallableKey::DerivedEquality {
                        exact_owner: result,
                    },
                )
                .unwrap();
                let mut pending = PendingIdentityValidation::new();
                pending
                    .register_external_graph_authorities(dependency.identities)
                    .unwrap();
                pending
                    .register_external_canonical_authority(adapter.clone())
                    .unwrap();
                pending
                    .register_external_canonical_authority(unrelated.clone())
                    .unwrap();
                let graph = pending.finish().unwrap();
                assert_eq!(
                    resolve(
                        hir::ExternalHirTargetV1::GeneratedCallable(adapter.id()),
                        &changed,
                        &graph,
                        &mut meter()
                    ),
                    Err(Error::Parameters)
                );
                let target = hir::ExternalHirTargetV1::GeneratedCallable(unrelated.id());
                assert_eq!(
                    resolve(target, &changed, &graph, &mut meter()),
                    Err(Error::Target(target))
                );
                let mut measured = meter();
                resolve(
                    reference.target(),
                    site,
                    dependency.identities,
                    &mut measured,
                )
                .unwrap();
                let mut bounded = BudgetMeter::new(DecodeLimits {
                    validation_work_units: measured.usage().validation_work_units,
                    ..DecodeLimits::default()
                });
                resolve(
                    reference.target(),
                    site,
                    dependency.identities,
                    &mut bounded,
                )
                .unwrap();
                assert!(matches!(
                    resolve(
                        reference.target(),
                        site,
                        dependency.identities,
                        &mut bounded
                    ),
                    Err(Error::Resource(_))
                ));
            });
        },
    );
}
