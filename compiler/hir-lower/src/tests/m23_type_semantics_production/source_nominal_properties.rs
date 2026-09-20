use super::source_dispatch::with_hir_source as with_source;
use super::*;
use hir::{
    CanonicalNominalSourcePropertiesV1 as Table, NominalSupportPropertyPayloadV1 as Payload,
};
use scoop_identity::{DefinitionOriginSubject, PersistentPropertyId, SignatureTypeKey};
use scoop_wire::{decode_canonical, encode};

mod contracts;
mod wire;

const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-nominals/properties.scoop"
));
const COMBINED: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-dispatch/properties.scoop"
));

fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}

fn required(
    output: &hir::OrdinaryHirOutput<'_>,
) -> hir::CanonicalPersistentIdsV1<PersistentPropertyId> {
    let export = &output.output().export;
    let roots = hir::CanonicalSourceNominalIdsV1::from_export_hir(export, &mut meter()).unwrap();
    let sources =
        hir::CanonicalNominalSourceContractsV1::from_export_hir(export, &roots, &mut meter())
            .unwrap();
    hir::CanonicalPersistentIdsV1::try_new(
        sources
            .records()
            .iter()
            .flat_map(|source| {
                source
                    .members()
                    .values()
                    .iter()
                    .filter_map(|member| match member {
                        hir::NestedSourceMemberRefV1::Property(id) => Some(*id),
                        _ => None,
                    })
            })
            .collect(),
    )
    .unwrap()
}
fn table(output: &hir::OrdinaryHirOutput<'_>) -> Table {
    Table::from_export_hir(&output.output().export, &required(output), &mut meter()).unwrap()
}

#[test]
fn nominal_property_sources_preserve_nested_visibility_generics_and_constants() {
    with_source(SOURCE, |output, _| {
        let table = table(output);
        assert_eq!(table.records().len(), required(output).values().len());
        contracts::verify(output, &table);
        assert_eq!(
            contracts::render(output, &table),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m23-type-source-nominals/properties.snap"
            ))
        );
        let public =
            hir::CanonicalExportConstValuesV1::from_export_hir(output.output().export.module())
                .unwrap();
        assert_eq!(public.records().len(), 1);
        let record = table.get(public.records()[0].property()).unwrap();
        let Payload::Const { value } = record.payload() else {
            panic!("source const");
        };
        assert_eq!(value, &public.records()[0]);
    });
}

#[test]
fn nominal_and_inheritance_properties_reuse_the_same_runtime_contracts() {
    with_source(COMBINED, |output, _| {
        let table = table(output);
        let inheritance =
            hir::CanonicalInheritanceSourcePropertiesV1::from_ordinary_hir(output, &mut meter())
                .unwrap();
        for record in inheritance.records() {
            assert_eq!(table.get(record.declaration()), Some(record));
        }
        assert!(table.records().len() > inheritance.records().len());
        contracts::verify(output, &table);
    });
}

#[test]
fn nominal_property_source_producer_rejects_missing_or_top_level_declarations() {
    with_source(SOURCE, |output, _| {
        let missing = PersistentPropertyId::from_source_declaration(
            &scoop_identity::SourceDeclarationKey::property(
                scoop_identity::SourceDeclarationSite::new(
                    ConeIdentity::SINGLE_FILE,
                    scoop_identity::PackagePath::root(),
                    scoop_identity::DefinitionOwnerChain::top_level(),
                    scoop_identity::DeclarationScope::ConeWide,
                )
                .unwrap(),
                scoop_identity::CanonicalIdentifier::new("missing").unwrap(),
            ),
        )
        .unwrap();
        let required = hir::CanonicalPersistentIdsV1::try_new(vec![missing]).unwrap();
        assert!(
            matches!(Table::from_export_hir(&output.output().export, &required, &mut meter()),
            Err(hir::CrossConeTypeSemanticsProductionError::InvalidSourceDeclaration(message))
                if message == "required nominal source property has no sealed declaration")
        );
        let export = output.output().export.module();
        let (id, _) = export
            .properties
            .iter()
            .find(|(_, property)| property.name == "TOP")
            .unwrap();
        let hir::HirPropertyIdentity::Ordinary(identity) = &export.property_identities[id] else {
            panic!("ordinary property");
        };
        let required = hir::CanonicalPersistentIdsV1::try_new(vec![identity.id()]).unwrap();
        assert!(
            matches!(Table::from_export_hir(&output.output().export, &required, &mut meter()),
            Err(hir::CrossConeTypeSemanticsProductionError::InvalidSourceDeclaration(message))
                if message == "source property has no nominal owner")
        );
    });
}
