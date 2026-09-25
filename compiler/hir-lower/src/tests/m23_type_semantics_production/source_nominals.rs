use super::source_dispatch::with_hir_source as with_source;
use super::*;
use hir::{
    CanonicalNominalSourceContractsV1 as Table,
    DecodedCanonicalNominalSourceContractsV1 as Decoded, NominalSourceContractV1 as Record,
};
use scoop_identity::{DeclarationName, SignatureTypeKey, SourceDeclarationKey};
use scoop_wire::{decode_canonical, encode};

mod policy;
mod rejection;
mod render;
mod roots;

const DECLARATIONS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-nominals/declarations.scoop"
));
const NESTED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-nominals/nested.scoop"
));

fn sources<'a>(
    export: &'a hir::ExportHir,
) -> BTreeMap<hir::SourceNominalId, &'a SourceDeclarationKey> {
    let mut records = BTreeMap::new();
    let mut add = |identity: &'a hir::HirNominalIdentity| {
        if let Some(source) = identity.source() {
            let key = source.declaration();
            if key.origin() == export.cone {
                records.insert(
                    hir::SourceNominalId::from_source_declaration(key).unwrap(),
                    key,
                );
            }
        }
    };
    for (id, _) in export.classes.iter() {
        add(&export.nominal_identities[id]);
    }
    for (id, _) in export.interfaces.iter() {
        add(&export.nominal_identities[id]);
    }
    for (id, _) in export.structs.iter() {
        add(&export.nominal_identities[id]);
    }
    for (id, _) in export.enums.iter() {
        add(&export.nominal_identities[id]);
    }
    for (id, _) in export.objects.iter() {
        add(&export.nominal_identities[id]);
    }
    records
}
fn table(output: &hir::DependencyHirOutput) -> Table {
    let required = hir::CanonicalSourceNominalIdsV1::try_new(
        sources(output.output().export.module())
            .into_keys()
            .collect(),
    )
    .unwrap();
    Table::from_export_hir(&output.output().export, &required).unwrap()
}
fn name(key: &SourceDeclarationKey) -> &str {
    let DeclarationName::Named(name) = key.name() else {
        panic!("nominal or member source name")
    };
    name.as_str()
}

#[test]
fn nominal_source_contracts_project_all_kinds_and_restricted_nested_shapes() {
    for (source, expected) in [
        (
            DECLARATIONS,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m23-type-source-nominals/declarations.snap"
            )),
        ),
        (
            NESTED,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m23-type-source-nominals/nested.snap"
            )),
        ),
    ] {
        with_source(source, |output, _| {
            let table = table(output);
            assert_eq!(
                render::render(output.output().export.module(), &table),
                expected
            );
            for record in table.records() {
                if let hir::NominalSourceShapeV1::Enum(shape) = record.source_shape() {
                    assert_eq!(
                        shape
                            .variants()
                            .iter()
                            .map(|v| v.style())
                            .collect::<Vec<_>>(),
                        vec![
                            hir::EnumSourceVariantStyleV1::Unit,
                            hir::EnumSourceVariantStyleV1::Positional,
                            hir::EnumSourceVariantStyleV1::Named,
                            hir::EnumSourceVariantStyleV1::Constructor
                        ]
                    );
                    for variant in &shape.variants()[1..] {
                        assert_eq!(
                            variant.fields()[0].value_type(),
                            &SignatureTypeKey::Binder { depth: 0, index: 0 }
                        );
                    }
                }
                if let hir::NominalSourceShapeV1::Struct(shape) = record.source_shape() {
                    assert_eq!(
                        shape.fields()[0].value_type(),
                        &SignatureTypeKey::Binder { depth: 0, index: 0 }
                    );
                }
            }
        });
    }
}

#[test]
fn nominal_source_contracts_replay_real_foundation_bytes_and_are_deterministic() {
    for source in [DECLARATIONS, NESTED] {
        let produce = || {
            with_source(source, |output, _| {
                let table = table(output);
                let bytes = encode(&table).unwrap();
                let decoded: Decoded = decode_canonical(&bytes).unwrap();
                assert_eq!(encode(&decoded).unwrap(), bytes);
                let mut identities = source_inventory::identity_closure(output);
                assert_eq!(decoded.resolve(&mut identities).unwrap(), table);
                bytes
            })
        };
        assert_eq!(produce(), produce());
    }
}

#[test]
fn nominal_source_contracts_select_exact_required_roots_without_adding_children() {
    with_source(NESTED, |output, _| {
        let export = output.output().export.module();
        let sources = sources(export);
        let owner = *sources
            .iter()
            .find(|(_, key)| name(key) == "Envelope")
            .unwrap()
            .0;
        let required = hir::CanonicalSourceNominalIdsV1::try_new(vec![owner]).unwrap();
        let table = Table::from_export_hir(&output.output().export, &required).unwrap();
        assert_eq!(
            table
                .records()
                .iter()
                .map(Record::owner)
                .collect::<Vec<_>>(),
            vec![owner]
        );
        assert_eq!(table.get(owner).unwrap().children().values().len(), 9);
        assert!(table.get(owner).unwrap().members().values().is_empty());
        assert!(
            Table::from_export_hir(
                &output.output().export,
                &hir::CanonicalSourceNominalIdsV1::default()
            )
            .unwrap()
            .records()
            .is_empty()
        );
    });
}

#[test]
fn nominal_source_contracts_keep_private_and_internal_constructors() {
    with_source(DECLARATIONS, |output, _| {
        let export = output.output().export.module();
        let (id, _) = export
            .classes
            .iter()
            .find(|(_, d)| d.name == "Defaults")
            .unwrap();
        let owner = hir::SourceNominalId::from_source_declaration(
            export.nominal_identities[id]
                .source()
                .unwrap()
                .declaration(),
        )
        .unwrap();
        let table = table(output);
        assert_eq!(table.get(owner).unwrap().constructors().values().len(), 3);
    });
}

#[test]
fn nominal_source_contracts_exclude_real_core_constructor_adapters() {
    let parsed = super::super::m23_ordinary_core_only::support::parsed_core(complete_core_file());
    let input = crate::CoreBootstrapSources::try_new(&parsed).unwrap();
    let output = crate::lower_core_bootstrap(&input).unwrap();
    let export = output.export.module();
    let (adapter, constructor) = export
        .class_constructors
        .iter()
        .find(|(_, declaration)| {
            matches!(
                declaration.identity_kind,
                hir::ClassConstructorIdentityKind::ZeroArgumentAdapter { .. }
            )
        })
        .unwrap();
    let owner = hir::SourceNominalId::from_source_declaration(
        export.nominal_identities[constructor.owner]
            .source()
            .unwrap()
            .declaration(),
    )
    .unwrap();
    let required = hir::CanonicalSourceNominalIdsV1::try_new(vec![owner]).unwrap();
    let table = Table::from_export_hir(&output.export, &required).unwrap();
    assert!(
        export.classes[constructor.owner]
            .constructors
            .contains(&adapter)
    );
    assert_eq!(table.get(owner).unwrap().constructors().values().len(), 1);
}
