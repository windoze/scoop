use super::*;

#[test]
fn callable_keys_must_be_owned_even_when_the_shared_graph_resolves_them() {
    with_source(DIRECT, |output, core| {
        let mut fixture = Fixture::from_output(output);
        let sources = Sources::from_output(output, &mut fixture);
        let inputs = core.foundation.import_core_inputs(&core.interface).unwrap();
        for generic in [false, true] {
            let mut canonical = fixture.foundation.as_canonical().clone();
            if generic {
                canonical.set_generic_functions(vec![]).unwrap();
            } else {
                canonical.set_functions(vec![]).unwrap();
            }
            let incomplete = hir::OdrFreeHirFoundation::try_new(canonical).unwrap();
            let foundation = fixture
                .source
                .bind_to_foundation(&incomplete, &fixture.identities, &mut meter())
                .unwrap();
            assert!(matches!(
                (
                    generic,
                    sources
                        .bind(
                            &foundation,
                            inputs.protocols().fundamental_types(),
                            &mut meter()
                        )
                        .unwrap_err()
                ),
                (
                    true,
                    Error::MissingKey(CallableTemplateOrigin::GenericFunction(_))
                ) | (
                    false,
                    Error::MissingKey(CallableTemplateOrigin::Function(_))
                )
            ));
        }
    });
}

#[test]
fn protected_callable_inventory_rejects_missing_and_extra_real_declarations() {
    with_source(SOURCE, |output, core| {
        let mut fixture = Fixture::from_output(output);
        let sources = Sources::from_output(output, &mut fixture);
        let foundation = fixture.bind().unwrap();
        let inputs = core.foundation.import_core_inputs(&core.interface).unwrap();
        let unused = output
            .output()
            .export
            .module()
            .function_identities
            .iter()
            .find_map(|(_, identity)| match identity {
                hir::HirFunctionIdentity::Source(hir::HirSourceFunctionIdentity::Plain(record))
                    if sources
                        .callables
                        .get(CallableTemplateOrigin::Function(record.id()))
                        .is_none() =>
                {
                    Some(CallableTemplateOrigin::Function(record.id()))
                }
                _ => None,
            })
            .unwrap();
        for extra in [false, true] {
            let mut forged = sources.clone();
            let mut records = forged.callables.records().to_vec();
            if extra {
                let first = records
                    .iter()
                    .find(|r| matches!(r.declaration(), CallableTemplateOrigin::Function(_)))
                    .unwrap();
                let old = first.payload();
                records.push(
                    Record::try_new(
                        unused,
                        first.declaration_access().clone(),
                        hir::ProtectedCallablePayloadV1::try_new(
                            unused,
                            old.owner(),
                            old.type_parameters().clone(),
                            old.parameters().clone(),
                            old.result().clone(),
                            old.effects(),
                            old.modality(),
                            old.slot_relations().clone(),
                        )
                        .unwrap(),
                    )
                    .unwrap(),
                );
            } else {
                records.pop().unwrap();
            }
            forged.callables = Table::try_new(records, &mut meter()).unwrap();
            assert!(matches!(
                forged.bind(
                    &foundation,
                    inputs.protocols().fundamental_types(),
                    &mut meter()
                ),
                Err(Error::Inventory)
            ));
        }
    });
}

fn replace_members(
    owner: &hir::SourceInheritanceInventoryV1,
    members: Vec<hir::ProtectedDeclarationRefV1>,
) -> hir::SourceInheritanceInventoryV1 {
    hir::SourceInheritanceInventoryV1::try_new(
        owner.owner(),
        owner.constructors().clone(),
        hir::CanonicalProtectedDeclarationRefsV1::try_new(members).unwrap(),
        owner.slot_schemas().clone(),
        &mut meter(),
    )
    .unwrap()
}

#[test]
fn protected_methods_cannot_move_or_repeat_between_inventory_owners() {
    with_source(DIRECT, |output, core| {
        let mut fixture = Fixture::from_output(output);
        let sources = Sources::from_output(output, &mut fixture);
        let foundation = fixture.bind().unwrap();
        let inputs = core.foundation.import_core_inputs(&core.interface).unwrap();
        let inventory = &sources.properties.dispatch.inventory;
        let (index, declaration) = inventory.records().iter().enumerate().find_map(|(index, owner)| {
            owner.protected_members().values().iter().find(|member| matches!(member,
                hir::ProtectedDeclarationRefV1::Callable(callable) if matches!(callable.declaration(),
                    CallableTemplateOrigin::Function(_)))).copied().map(|member| (index, member))
        }).unwrap();
        let hir::ProtectedDeclarationRefV1::Callable(callable) = declaration else {
            unreachable!()
        };
        let other = (index + 1) % inventory.records().len();
        for duplicate in [false, true] {
            let mut forged = sources.clone();
            let mut owners = inventory.records().to_vec();
            let mut members = owners[other].protected_members().values().to_vec();
            members.push(declaration);
            owners[other] = replace_members(&owners[other], members);
            if !duplicate {
                let members = owners[index]
                    .protected_members()
                    .values()
                    .iter()
                    .copied()
                    .filter(|member| *member != declaration)
                    .collect();
                owners[index] = replace_members(&owners[index], members);
            }
            forged.properties.dispatch.inventory =
                hir::CanonicalSourceInheritanceInventoriesV1::try_new(owners, &mut meter())
                    .unwrap();
            assert!(matches!((duplicate, forged.bind(&foundation,
                inputs.protocols().fundamental_types(), &mut meter()).unwrap_err()),
                (false, Error::Owner(actual)) | (true, Error::RepeatedOwner(actual))
                    if actual == callable.declaration()));
        }
    });
}
