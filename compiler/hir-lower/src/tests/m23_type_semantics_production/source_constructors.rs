use super::source_dispatch::with_source;
use super::*;
use hir::CanonicalInheritanceSourceConstructorsV1 as Table;
use scoop_identity::{DefinitionOriginSubject, DuplicateSignatureKey, SignatureTypeKey};
use scoop_wire::{decode_canonical, encode};

mod wire;

const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-dispatch/constructors.scoop"
));

fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}

fn table(output: &hir::DependencyHirOutput) -> Table {
    Table::from_dependency_hir(output, &mut meter()).unwrap()
}

#[test]
fn constructors_project_protected_public_defaults_and_struct_representation() {
    with_source(SOURCE, |output, _| {
        let records = table(output);
        let inventory =
            hir::CanonicalSourceInheritanceInventoriesV1::from_dependency_hir(output, &mut meter())
                .unwrap();
        let required = inventory
            .records()
            .iter()
            .flat_map(|owner| owner.constructors().values().iter().copied())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            records
                .records()
                .iter()
                .map(|record| record.declaration())
                .collect::<BTreeSet<_>>(),
            required
        );
        assert_eq!(records.records().len(), 6);
        let export = output.output().export.module();
        let mut names = BTreeMap::new();
        for (id, class) in export.classes.iter() {
            if let Some(owner) = export.nominal_identities[id]
                .source()
                .and_then(|source| source.concrete_id())
            {
                names.insert(owner, class.name.as_str());
            }
        }
        for (id, structure) in export.structs.iter() {
            let owner = export.nominal_identities[id]
                .source()
                .unwrap()
                .concrete_id()
                .unwrap();
            names.insert(owner, structure.name.as_str());
        }
        let mut lines = Vec::new();
        for record in records.records() {
            let payload = record.payload();
            let hir::SourceNominalId::Concrete(owner) = payload.owner() else {
                panic!("param-free owner")
            };
            assert_eq!(payload.result(), &SignatureTypeKey::Nominal(owner));
            assert!(payload.type_parameters().is_empty());
            assert!(!payload.receiver().is_present());
            assert!(payload.slot_relations().is_empty());
            assert_eq!(
                payload.effects().execution(),
                scoop_identity::Effect::Ordinary
            );
            assert_eq!(
                payload.effects().implementation(),
                hir::CallableImplementationV1::Scoop
            );
            assert_eq!(
                record.declaration_access().definition_origin().origin(),
                export
                    .export_definition_origins
                    .get(DefinitionOriginSubject::Constructor(record.declaration()))
                    .unwrap()
                    .origin()
            );
            let parameters = payload.parameters().parameters();
            lines.push(format!(
                "{}({}): {:?}, {:?}\n",
                names[&owner],
                parameters
                    .iter()
                    .map(|parameter| parameter.name().as_str())
                    .collect::<Vec<_>>()
                    .join(","),
                record.declaration_access().declared_visibility(),
                payload.modality()
            ));
            assert_eq!(records.get(record.declaration()), Some(record));
        }
        lines.sort();
        assert_eq!(
            lines.concat(),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m23-type-source-dispatch/constructors.contracts.snap"
            ))
        );
        verify_parameter_types(export, &records);
    });
}

fn verify_parameter_types(export: &hir::ExportHir, table: &Table) {
    let mut keys = BTreeMap::new();
    for (id, _) in export.class_constructors.iter() {
        if let Some(record) = export.constructor_identities[id].source_record() {
            keys.insert(record.id(), record.key());
        }
    }
    for (id, _) in export.struct_constructors.iter() {
        let record = &export.constructor_identities[id];
        keys.insert(record.id(), record.key());
    }
    for record in table.records() {
        let DuplicateSignatureKey::Constructor { parameters } =
            keys[&record.declaration()].duplicate_signature()
        else {
            panic!("typed constructor key")
        };
        assert_eq!(
            record
                .payload()
                .parameters()
                .parameters()
                .iter()
                .map(|parameter| parameter.value_type())
                .collect::<Vec<_>>(),
            parameters.iter().collect::<Vec<_>>()
        );
    }
}

#[test]
fn constructor_source_bytes_are_deterministic_and_resolve_without_candidate_tables() {
    let first = with_source(SOURCE, |output, _| {
        let table = table(output);
        let bytes = encode(&table).unwrap();
        let decoded: hir::DecodedCanonicalInheritanceSourceConstructorsV1 =
            decode_canonical(&bytes, DecodeLimits::default()).unwrap();
        assert_eq!(encode(&decoded).unwrap(), bytes);
        let mut identities = source_inventory::identity_closure(output);
        assert_eq!(
            decoded.resolve(&mut identities, &mut meter()).unwrap(),
            table
        );
        bytes
    });
    let second = with_source(SOURCE, |output, _| encode(&table(output)).unwrap());
    assert_eq!(first, second);
}

#[test]
fn constructor_projection_obeys_the_shared_resource_budget() {
    with_source(SOURCE, |output, _| {
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
                semantic_recursion: 0,
                ..DecodeLimits::default()
            },
        ] {
            assert!(matches!(
                Table::from_dependency_hir(output, &mut BudgetMeter::new(limits)),
                Err(hir::CrossConeTypeSemanticsProductionError::SourceInventory(
                    hir::SourceInventoryError::Resource(_)
                ))
            ));
        }
    });
}

#[test]
fn protected_constructor_sources_are_available_before_candidate_construction() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-type-source-dispatch/protected-construction.scoop"
    ));
    with_source(source, |output, _| {
        let table = table(output);
        assert_eq!(table.records().len(), 2);
        assert_eq!(
            table
                .records()
                .iter()
                .filter(|record| record.declaration_access().declared_visibility()
                    == hir::DeclaredVisibilityV1::Protected)
                .count(),
            1
        );
        assert_eq!(
            table
                .records()
                .iter()
                .filter(|record| record.declaration_access().declared_visibility()
                    == hir::DeclaredVisibilityV1::Public)
                .count(),
            1
        );
        let production =
            produce_cross_cone_type_semantics(output, &public_interface(output)).unwrap();
        assert_eq!(
            production
                .section()
                .inheritance()
                .records()
                .iter()
                .map(|record| record.constructors().records().len())
                .sum::<usize>(),
            2
        );
        assert_eq!(
            production
                .section()
                .protected_declarations()
                .records()
                .len(),
            1
        );
    });
}
