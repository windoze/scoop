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
            if source == SELECTIONS {
                assert_eq!(
                    render::render(output, selected),
                    include_str!(concat!(
                        env!("CARGO_MANIFEST_DIR"),
                        "/../../tests/fixtures/m23-type-source-dispatch/selections.choices.snap"
                    ))
                );
            }
            let public = public_interface(output);
            let mut identities = super::super::super::source_inventory::identity_closure(output);
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
                            hir::InheritanceSlotImplementationV1::Abstract => Selection::Abstract,
                            hir::InheritanceSlotImplementationV1::Concrete(target) => {
                                Selection::Concrete(target.declaration())
                            }
                            hir::InheritanceSlotImplementationV1::InterfaceDefault(target) => {
                                Selection::InterfaceDefault(target.declaration())
                            }
                        };
                        (record.slot(), selection)
                    })
                    .collect::<Vec<_>>();
                assert_eq!(
                    choices
                        .records()
                        .iter()
                        .map(|record| (record.slot(), record.selection()))
                        .collect::<Vec<_>>(),
                    expected
                );
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
        let (callables, slots) = render::labels(output);
        let mut rows = Vec::new();
        let mut identities = super::super::super::source_inventory::identity_closure(output);
        for nominal in public.nominal_interfaces().all_records() {
            let choices = nominal.declaration_details().dispatch_selections();
            assert!(!choices.records().is_empty());
            for record in choices.records() {
                let choice = match record.selection() {
                    Selection::Abstract => "abstract".into(),
                    Selection::Concrete(id) => format!("concrete {}", callables[&id]),
                    Selection::InterfaceDefault(id) => format!("default {}", callables[&id]),
                };
                rows.push(format!(
                    "{}: {} -> {choice}\n",
                    names[&nominal.declaration()],
                    slots[&record.slot()]
                ));
            }
            let decoded: DecodedCanonicalNominalDispatchSelectionsV1 =
                decode_canonical(&encode(choices).unwrap()).unwrap();
            assert_eq!(decoded.resolve(&mut identities).unwrap(), *choices);
        }
        rows.sort();
        assert_eq!(
            rows.concat(),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m23-type-source-dispatch/shared-selections.choices.snap"
            ))
        );
        wire::verify(output, &public);
    });
}
