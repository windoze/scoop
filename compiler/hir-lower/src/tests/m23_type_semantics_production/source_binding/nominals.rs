use super::super::source_dispatch::with_hir_source as with_source;
use super::*;
use hir::{
    CanonicalNominalSourceContractsV1 as Table, NominalSourceBindingError as Error,
    NominalSourceContractV1 as Record,
};
use scoop_identity::{
    DeclarationName, PersistentEnumVariantFieldId, PersistentEnumVariantId, PersistentFieldId,
    PersistentObjectValueId,
};

mod combinations;
mod corruption;
mod origins;
mod resources;
mod root_closure;
mod semantics;

const DECLARATIONS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-nominals/declarations.scoop"
));
const NESTED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-nominals/nested.scoop"
));

fn sources(output: &hir::OrdinaryHirOutput<'_>, fixture: &mut Fixture) -> Table {
    let table = Table::from_export_hir(
        &output.output().export,
        &fixture.source.entries().source_roots,
        &mut meter(),
    )
    .unwrap();
    let bytes = encode(&table).unwrap();
    let decoded: hir::DecodedCanonicalNominalSourceContractsV1 =
        decode_canonical(&bytes, DecodeLimits::default()).unwrap();
    let restored = decoded
        .resolve(&mut fixture.identities, &mut meter())
        .unwrap();
    assert_eq!(encode(&restored).unwrap(), bytes);
    restored
}
fn named<'a>(fixture: &Fixture, table: &'a Table, name: &str) -> &'a Record {
    let foundation = fixture.bind().unwrap();
    table.records().iter().find(|record| {
        matches!(foundation.nominal_key(record.owner()).unwrap().name(), DeclarationName::Named(actual) if actual.as_str() == name)
    }).unwrap()
}
fn replace(table: &Table, record: Record) -> Table {
    Table::try_new(
        table
            .records()
            .iter()
            .map(|existing| {
                if existing.owner() == record.owner() {
                    record.clone()
                } else {
                    existing.clone()
                }
            })
            .collect(),
        &mut meter(),
    )
    .unwrap()
}
fn rebuild(
    source: &Record,
    constructors: hir::CanonicalPersistentIdsV1<scoop_identity::PersistentConstructorId>,
    members: hir::CanonicalNestedMemberRefsV1,
    children: hir::CanonicalNestedNominalRefsV1,
    shape: hir::NominalSourceShapeV1,
) -> Record {
    Record::try_new(
        source.owner(),
        source.modality(),
        source.type_parameters().clone(),
        source.supertypes().clone(),
        constructors,
        members,
        children,
        shape,
    )
    .unwrap()
}

#[test]
fn byte_restored_nominal_sources_bind_complete_declarations_and_variant_origins() {
    for input in [DECLARATIONS, NESTED] {
        with_source(input, |output, _| {
            let mut fixture = Fixture::from_output(output);
            let table = sources(output, &mut fixture);
            let foundation = fixture.bind().unwrap();
            let bound = foundation
                .bind_nominal_sources(&table, &mut meter())
                .unwrap();
            assert_eq!(bound.table(), &table);
            for record in table.records() {
                assert_eq!(bound.nominal_source(record.owner()).unwrap(), record);
                match record.source_shape() {
                    hir::NominalSourceShapeV1::Struct(shape) => {
                        for field in shape.fields() {
                            assert_eq!(
                                PersistentFieldId::from_key(
                                    bound.struct_field_key(field.field()).unwrap()
                                )
                                .unwrap(),
                                field.field()
                            );
                        }
                    }
                    hir::NominalSourceShapeV1::Enum(shape) => {
                        for variant in shape.variants() {
                            assert_eq!(
                                bound.enum_variant_shape(variant.variant()).unwrap(),
                                variant
                            );
                            assert_eq!(
                                PersistentEnumVariantId::from_key(
                                    bound.enum_variant_key(variant.variant()).unwrap()
                                )
                                .unwrap(),
                                variant.variant()
                            );
                            let origin = bound.enum_variant_origin(variant.variant()).unwrap();
                            assert_eq!(
                                origin.origin(),
                                fixture
                                    .foundation
                                    .definition_origin(
                                        scoop_identity::DefinitionOriginSubject::EnumVariant(
                                            variant.variant()
                                        )
                                    )
                                    .unwrap()
                                    .origin()
                            );
                            assert_eq!(
                                origin.origin().source(),
                                foundation
                                    .nominal_source(record.owner())
                                    .unwrap()
                                    .access()
                                    .definition_origin()
                                    .origin()
                                    .source()
                            );
                            for field in variant.fields() {
                                assert_eq!(
                                    PersistentEnumVariantFieldId::from_key(
                                        bound.enum_variant_field_key(field.field()).unwrap()
                                    )
                                    .unwrap(),
                                    field.field()
                                );
                            }
                        }
                    }
                    hir::NominalSourceShapeV1::Object(shape) => {
                        assert_eq!(
                            PersistentObjectValueId::from_source_object(
                                bound.object_value_key(shape.value()).unwrap()
                            )
                            .unwrap(),
                            shape.value()
                        );
                    }
                    hir::NominalSourceShapeV1::Class | hir::NominalSourceShapeV1::Interface => {}
                }
            }
            if input == NESTED {
                let envelope = named(&fixture, &table, "Envelope");
                assert_eq!(envelope.children().values().len(), 9);
                assert!(
                    envelope
                        .children()
                        .values()
                        .iter()
                        .any(|child| table.get(*child).is_none())
                );
            }
        });
    }
}
