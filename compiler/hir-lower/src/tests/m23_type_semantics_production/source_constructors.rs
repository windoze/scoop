use super::source_dispatch::with_source;
use super::*;
use scoop_identity::{DefinitionOriginSubject, DuplicateSignatureKey, SignatureTypeKey};

const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-dispatch/constructors.scoop"
));

#[test]
fn constructors_project_protected_public_defaults_and_struct_representation() {
    with_source(SOURCE, |output, _| {
        let production =
            produce_cross_cone_type_semantics(output, &public_interface(output)).unwrap();
        let records = production
            .inheritance()
            .records()
            .iter()
            .flat_map(|owner| owner.constructors().records())
            .map(|record| record.source())
            .collect::<Vec<_>>();
        let inventory = production.inheritance();
        let required = inventory
            .records()
            .iter()
            .flat_map(|owner| {
                owner
                    .constructors()
                    .records()
                    .iter()
                    .map(|record| record.declaration())
            })
            .collect::<BTreeSet<_>>();
        assert_eq!(
            records
                .iter()
                .map(|record| record.declaration())
                .collect::<BTreeSet<_>>(),
            required
        );
        assert_eq!(records.len(), 6);
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
        for record in &records {
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

fn verify_parameter_types(
    export: &hir::ExportHir,
    table: &[&hir::NominalSupportConstructorInterfaceV1],
) {
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
    for record in table {
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
fn protected_constructor_interfaces_are_complete_in_the_type_section() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-type-source-dispatch/protected-construction.scoop"
    ));
    with_source(source, |output, _| {
        let production =
            produce_cross_cone_type_semantics(output, &public_interface(output)).unwrap();
        let table = production
            .inheritance()
            .records()
            .iter()
            .flat_map(|record| record.constructors().records())
            .map(|record| record.source())
            .collect::<Vec<_>>();
        assert_eq!(table.len(), 2);
        assert_eq!(
            table
                .iter()
                .filter(|record| record.declaration_access().declared_visibility()
                    == hir::DeclaredVisibilityV1::Protected)
                .count(),
            1
        );
        assert_eq!(
            table
                .iter()
                .filter(|record| record.declaration_access().declared_visibility()
                    == hir::DeclaredVisibilityV1::Public)
                .count(),
            1
        );
        assert_eq!(
            production
                .inheritance()
                .records()
                .iter()
                .map(|record| record.constructors().records().len())
                .sum::<usize>(),
            2
        );
        assert_eq!(production.protected_declarations().records().len(), 1);
    });
}
