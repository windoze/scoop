use super::source_dispatch::with_hir_source as with_source;
use super::*;
use hir::CanonicalNominalSourceConstructorsV1 as Table;
use scoop_identity::{
    CallableTemplateOrigin, DefinitionOriginSubject, PersistentConstructorId, SignatureTypeKey,
};
use scoop_wire::{decode_canonical, encode};

mod contracts;
mod rejection;
mod wire;

const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-nominals/constructors.scoop"
));
const PARAMETERS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-nominals/parameters.scoop"
));
const INHERITANCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-dispatch/constructors.scoop"
));
const EFFECTS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m19-constructor-nogc/scalar.scoop"
));

fn meter() -> BudgetMeter {
    BudgetMeter::new(DecodeLimits::default())
}
fn required(output: &hir::DependencyHirOutput) -> BTreeSet<PersistentConstructorId> {
    let export = &output.output().export;
    let roots = hir::CanonicalSourceNominalIdsV1::from_export_hir(export, &mut meter()).unwrap();
    let nominals =
        hir::CanonicalNominalSourceContractsV1::from_export_hir(export, &roots, &mut meter())
            .unwrap();
    nominals
        .records()
        .iter()
        .flat_map(|n| n.constructors().values().iter().copied())
        .collect()
}
fn table(output: &hir::DependencyHirOutput) -> Table {
    Table::from_export_hir(&output.output().export, &required(output), &mut meter()).unwrap()
}

#[test]
fn nominal_constructor_sources_preserve_all_visibilities_generic_binders_and_effects() {
    for input in [SOURCE, PARAMETERS, INHERITANCE, EFFECTS] {
        with_source(input, |output, _| {
            let source = table(output);
            assert_eq!(source.records().len(), required(output).len());
            contracts::verify(output, &source);
        });
    }
}

#[test]
fn complete_and_inheritance_constructor_sources_share_exact_contracts() {
    for input in [SOURCE, INHERITANCE, EFFECTS] {
        with_source(input, |output, _| {
            let complete = table(output);
            let inheritance = hir::CanonicalInheritanceSourceConstructorsV1::from_dependency_hir(
                output,
                &mut meter(),
            )
            .unwrap();
            for record in inheritance.records() {
                let source = complete.get(record.declaration()).unwrap();
                assert_eq!(source, record);
                assert_eq!(encode(source).unwrap(), encode(record).unwrap());
            }
            assert!(!inheritance.records().is_empty());
            if input != EFFECTS {
                assert!(complete.records().len() > inheritance.records().len());
            }
        });
    }
}

#[test]
fn complete_nominal_constructor_dump_and_bytes_are_stable() {
    let first = with_source(SOURCE, |output, _| {
        let source = table(output);
        assert_eq!(
            contracts::verify(output, &source),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m23-type-source-nominals/constructors.snap"
            ))
        );
        encode(&source).unwrap()
    });
    assert_eq!(
        first,
        with_source(SOURCE, |output, _| encode(&table(output)).unwrap())
    );
}
