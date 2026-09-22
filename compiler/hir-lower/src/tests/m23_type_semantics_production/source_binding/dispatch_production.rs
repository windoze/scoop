use super::super::source_dispatch::{INTERFACES, with_hir_source, with_hir_sources};
use super::dispatch_binding::Sources;
use super::*;
use hir::InheritanceSlotImplementationV1 as Implementation;

const VIRTUAL: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-slot-production/virtual.scoop"
));

#[test]
fn produced_dispatch_contracts_roundtrip_and_validate_against_source_choices() {
    for source in [VIRTUAL, INTERFACES] {
        with_hir_source(source, |output, _| {
            let production =
                produce_cross_cone_type_semantics(output, &public_interface(output)).unwrap();
            let mut fixture = Fixture::from_output(output);
            let sources = Sources::from_output(output, &mut fixture);
            let mut restored = Vec::new();
            for nominal in production.section().inheritance().records() {
                let expected = sources.inventory.get(nominal.owner()).unwrap();
                assert_eq!(nominal.slot_schemas(), expected.slot_schemas());
                let required: BTreeSet<_> = nominal
                    .slot_schemas()
                    .records()
                    .iter()
                    .flat_map(|schema| schema.slots().iter().copied())
                    .collect();
                assert_eq!(
                    required,
                    nominal
                        .slots()
                        .records()
                        .iter()
                        .map(|record| record.slot())
                        .collect()
                );
                for record in nominal.slots().records() {
                    let bytes = encode(record).unwrap();
                    let decoded: hir::DecodedInheritanceSlotContractV1 =
                        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
                    let decoded = decoded
                        .resolve(&mut fixture.identities, &mut meter())
                        .unwrap();
                    assert_eq!(&decoded, record);
                    assert_eq!(encode(&decoded).unwrap(), bytes);
                    restored.push((nominal.owner(), decoded));
                }
            }
            let foundation = fixture.bind().unwrap();
            let dispatch = sources.bind(&foundation, &mut meter()).unwrap();

            let slots = dispatch.bind_slot_sources(&mut meter()).unwrap();
            for (owner, record) in &restored {
                slots
                    .validate_contract(*owner, record, &mut meter())
                    .unwrap();
                assert!(
                    production
                        .section()
                        .definition_sources()
                        .contains(record.declaration_access().definition_origin())
                );
                if let Some(target) = record.implementation().target() {
                    assert!(
                        production
                            .section()
                            .definition_sources()
                            .contains(target.declaration_access().definition_origin())
                    );
                }
            }
            assert!(
                restored
                    .iter()
                    .any(|(_, record)| matches!(record.implementation(), Implementation::Abstract))
            );
            let dump = render(output, &production);
            let expected = if source == VIRTUAL {
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../tests/fixtures/m23-type-slot-production/virtual.scoop.snap"
                ))
            } else {
                include_str!(concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../../tests/fixtures/m23-type-slot-production/interfaces.scoop.snap"
                ))
            };
            if std::env::var_os("SCOOP_UPDATE_SLOT_PRODUCTION_SNAPSHOTS").is_some() {
                let name = if source == VIRTUAL {
                    "virtual"
                } else {
                    "interfaces"
                };
                let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
                    "../../tests/fixtures/m23-type-slot-production/{name}.scoop.snap"
                ));
                std::fs::write(path, dump).unwrap();
            } else {
                assert_eq!(dump, expected);
            }
        });
    }
}

#[test]
fn produced_dispatch_is_deterministic_and_obeys_resource_limits() {
    let produce = |sources: &[(&str, &str)]| {
        with_hir_sources(sources, |output, _| {
            let public = public_interface(output);
            for limits in [
                DecodeLimits {
                    validation_work_units: 0,
                    ..DecodeLimits::default()
                },
                DecodeLimits {
                    logical_heap_bytes: 0,
                    ..DecodeLimits::default()
                },
                DecodeLimits {
                    semantic_table_entries: 0,
                    ..DecodeLimits::default()
                },
            ] {
                assert!(
                    crate::produce_cross_cone_type_semantics(
                        output,
                        &public,
                        &mut BudgetMeter::new(limits)
                    )
                    .is_err()
                );
            }
            let production = produce_cross_cone_type_semantics(output, &public).unwrap();
            encode(production.section().inheritance()).unwrap()
        })
    };
    // Perturb allocation without changing the exported declarations' source spans.
    let first = produce(&[("src/main.scoop", INTERFACES)]);
    let second = produce(&[
        ("src/a-unused.scoop", "private fun unrelated(): Int = 0"),
        ("src/main.scoop", INTERFACES),
    ]);
    assert!(
        first == second,
        "dispatch bytes changed with unrelated arena allocation"
    );
}

#[test]
fn produced_contracts_reject_a_different_reachable_default_and_narrowed_domain() {
    with_hir_source(INTERFACES, |output, _| {
        let production =
            produce_cross_cone_type_semantics(output, &public_interface(output)).unwrap();
        let mut fixture = Fixture::from_output(output);
        let sources = Sources::from_output(output, &mut fixture);
        let foundation = fixture.bind().unwrap();
        let dispatch = sources.bind(&foundation, &mut meter()).unwrap();

        let slots = dispatch.bind_slot_sources(&mut meter()).unwrap();
        let mut checked = 0;
        for nominal in production.section().inheritance().records() {
            for record in nominal.slots().records() {
                let narrowed =
                    hir::InheritanceSlotContractV1::try_new(
                        record.slot(),
                        record.declaration_owner(),
                        record.declaration(),
                        record.signature().clone(),
                        hir::PersistentSlotContractDomainV1::new(
                            hir::PersistentAccessDomainV1::empty(),
                        ),
                        record.implementation().clone(),
                        record.declaration_access().clone(),
                    )
                    .unwrap();
                assert!(matches!(
                    slots.validate_contract(nominal.owner(), &narrowed, &mut meter()),
                    Err(hir::InheritanceInterfaceSemanticError::Slot(
                        hir::InheritanceSlotContractSemanticError::Domain
                    ))
                ));
                let Implementation::InterfaceDefault(selected) = record.implementation() else {
                    continue;
                };
                if selected.declaration() == record.declaration() {
                    continue;
                }
                let root = sources.callables.get(record.declaration()).unwrap();
                let wrong = hir::InheritanceSlotContractV1::try_new(
                    record.slot(),
                    record.declaration_owner(),
                    record.declaration(),
                    record.signature().clone(),
                    record.domain().clone(),
                    Implementation::InterfaceDefault(
                        hir::InheritanceSlotTargetV1::try_new(
                            root.declaration(),
                            record.declaration_owner(),
                            root.signature().clone(),
                            root.modality(),
                            root.declaration_access().clone(),
                        )
                        .unwrap(),
                    ),
                    record.declaration_access().clone(),
                )
                .unwrap();
                slots
                    .graph()
                    .validate_slot_contract(nominal.owner(), &wrong, &slots, &mut meter())
                    .unwrap();
                assert!(matches!(
                    slots.validate_contract(nominal.owner(), &wrong, &mut meter()),
                    Err(hir::InheritanceInterfaceSemanticError::SlotSelection)
                ));
                checked += 1;
            }
        }
        assert!(checked > 0);
    });
}

fn render(
    output: &hir::DependencyHirOutput,
    production: &hir::CrossConeTypeSemanticsProductionV1,
) -> String {
    let export = output.output().export.module();
    let mut names = BTreeMap::new();
    for (id, function) in export.functions.iter() {
        let declaration = match &export.function_identities[id] {
            hir::HirFunctionIdentity::Source(hir::HirSourceFunctionIdentity::Plain(record)) => {
                hir::InheritanceCallableDeclarationV1::Function(record.id())
            }
            hir::HirFunctionIdentity::PropertyAccessor(
                hir::HirPropertyAccessorFunction::Getter(id),
            ) => hir::InheritanceCallableDeclarationV1::Getter(
                export.property_accessor_identities[*id].id(),
            ),
            hir::HirFunctionIdentity::PropertyAccessor(
                hir::HirPropertyAccessorFunction::Setter(id),
            ) => hir::InheritanceCallableDeclarationV1::Setter(
                export.property_accessor_identities[*id].id(),
            ),
            _ => continue,
        };
        names.insert(declaration, function.name.as_str());
    }
    let foundation = production.foundation();
    let mut lines = Vec::new();
    for nominal in production.section().inheritance().records() {
        let scoop_identity::ExactTypeKey::Nominal(owner) =
            foundation.exact_type_key(nominal.owner()).unwrap()
        else {
            panic!("source nominal")
        };
        let scoop_identity::DeclarationName::Named(owner) = foundation
            .nominal_declaration_key(hir::SourceNominalId::Concrete(*owner))
            .unwrap()
            .name()
        else {
            panic!("named nominal")
        };
        let owner = owner.as_str();
        for record in nominal.slots().records() {
            let target = match record.implementation() {
                Implementation::Abstract => "abstract".to_owned(),
                Implementation::Concrete(target) => {
                    format!("{} ({:?})", names[&target.declaration()], target.modality())
                }
                Implementation::InterfaceDefault(target) => {
                    format!("{} (default)", names[&target.declaration()])
                }
            };
            lines.push(format!(
                "{owner}: {} -> {target}\n",
                names[&record.declaration()]
            ));
        }
    }
    lines.sort();
    lines.concat()
}
