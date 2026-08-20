use super::*;
use hir::{CanonicalNominalDispatchSelectionsV1, DecodedCanonicalNominalDispatchSelectionsV1};

mod wire;

const GENERIC: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-dispatch/shared-selections.scoop"
));

#[test]
fn shared_nominals_preserve_actual_dispatch_choices_for_all_concrete_owner_kinds() {
    for source in [SELECTIONS, INTERFACES, VIRTUAL] {
        with_source(source, |output, _| {
            let section =
                produce_cross_cone_type_semantics(output, &public_interface(output)).unwrap();
            let selected = section.inheritance();
            let public = public_interface(output);
            let mut identities = super::super::super::source_inventory::identity_closure(output);
            let foundation =
                hir::CanonicalHirFoundation::from_type_semantics_output(output).unwrap();
            let mut count = 0;
            for nominal in public.nominal_interfaces().all_records() {
                let scoop_identity::NominalDeclarationOwner::Concrete(id) = nominal.declaration()
                else {
                    continue;
                };
                let exact =
                    PersistentExactTypeId::from_key(&scoop_identity::ExactTypeKey::Nominal(id))
                        .unwrap();
                let choices = nominal.declaration_details().dispatch_selections();
                let expected = selected
                    .get(exact)
                    .unwrap()
                    .slots()
                    .records()
                    .iter()
                    .map(|record| {
                        let selection = match record.implementation() {
                            hir::InheritanceSlotImplementationV1::Abstract(target) => {
                                Selection::Abstract(target.declaration())
                            }
                            hir::InheritanceSlotImplementationV1::Concrete(target) => {
                                Selection::Concrete(target.declaration())
                            }
                            hir::InheritanceSlotImplementationV1::InterfaceDefault(target) => {
                                Selection::InterfaceDefault(target.declaration())
                            }
                        };
                        (record.key(), selection)
                    })
                    .collect::<BTreeMap<_, _>>();
                let actual = {
                    let metadata = hir::SharedTypeMetadataV1 {
                        provider: output.output().export.cone,
                        identities: &identities,
                        foundation: &foundation,
                        public: &public,
                    };
                    choices
                        .records()
                        .iter()
                        .map(|record| {
                            let role = match record.role() {
                                hir::NominalDispatchSelectionRoleV1::ClassVtable => {
                                    hir::InheritanceSlotSchemaRoleV1::ClassVtable
                                }
                                hir::NominalDispatchSelectionRoleV1::Interface { interface } => {
                                    hir::InheritanceSlotSchemaRoleV1::Interface {
                                        interface_exact: metadata
                                            .signature_exact_type(interface)
                                            .unwrap(),
                                    }
                                }
                            };
                            ((role, record.slot()), record.selection())
                        })
                        .collect::<BTreeMap<_, _>>()
                };
                assert_eq!(actual, expected);
                count += expected.len();
                let decoded: DecodedCanonicalNominalDispatchSelectionsV1 =
                    decode_canonical(&encode(choices).unwrap()).unwrap();
                assert_eq!(decoded.resolve(&mut identities).unwrap(), *choices);
            }
            assert_eq!(
                count,
                selected
                    .records()
                    .iter()
                    .map(|record| record.slots().records().len())
                    .sum::<usize>()
            );
        });
    }
}

#[test]
fn shared_generic_choices_keep_source_identity_without_materializing_exact_types() {
    super::super::support::with_hir_source(GENERIC, |output, _| {
        let public = public_interface(output);
        let machine = project(output);
        assert!(machine.records().is_empty());
        let export = output.output().export.module();
        let mut names = BTreeMap::new();
        let mut add = |identity: &hir::HirNominalIdentity, name: &str| {
            let hir::HirSourceNominalIdentity::Generic(source) = identity.source().unwrap() else {
                panic!("all fixture owners are source-only generic declarations");
            };
            names.insert(
                scoop_identity::NominalDeclarationOwner::GenericTemplate(source.id()),
                name.to_owned(),
            );
        };
        for id in &export.public_surface.classes {
            add(&export.nominal_identities[*id], &export.classes[*id].name);
        }
        for id in &export.public_surface.interfaces {
            add(
                &export.nominal_identities[*id],
                &export.interfaces[*id].name,
            );
        }
        let callables = export
            .functions
            .iter()
            .filter_map(|(id, function)| match &export.function_identities[id] {
                hir::HirFunctionIdentity::Source(hir::HirSourceFunctionIdentity::Plain(record)) => {
                    Some((function.name.as_str(), Callable::Function(record.id())))
                }
                _ => None,
            })
            .collect::<BTreeMap<_, _>>();
        let mut identities = super::super::super::source_inventory::identity_closure(output);
        assert_eq!(names.len(), 4);
        for nominal in public.nominal_interfaces().all_records() {
            let choices = nominal.declaration_details().dispatch_selections();
            let name = names[&nominal.declaration()].as_str();
            assert_eq!(choices.records().len(), if name == "Leaf" { 3 } else { 1 });
            for record in choices.records() {
                let expected = match (name, record.role()) {
                    ("Base", hir::NominalDispatchSelectionRoleV1::ClassVtable) => {
                        Selection::Concrete(callables["Base.compute"])
                    }
                    ("Leaf", hir::NominalDispatchSelectionRoleV1::ClassVtable) => {
                        Selection::Concrete(callables["Leaf.compute"])
                    }
                    ("Generic", hir::NominalDispatchSelectionRoleV1::Interface { .. }) => {
                        Selection::InterfaceDefault(callables["Generic.value"])
                    }
                    ("Leaf" | "Refined", hir::NominalDispatchSelectionRoleV1::Interface { .. }) => {
                        Selection::InterfaceDefault(callables["Refined.value"])
                    }
                    _ => panic!("unexpected generic dispatch owner or role"),
                };
                assert_eq!(record.selection(), expected);
            }
            let decoded: DecodedCanonicalNominalDispatchSelectionsV1 =
                decode_canonical(&encode(choices).unwrap()).unwrap();
            assert_eq!(decoded.resolve(&mut identities).unwrap(), *choices);
        }
        wire::verify(output, &public);
    });
}
