use super::source_dispatch::with_hir_source as with_source;
use super::*;
use hir::{
    CanonicalNominalSourceParameterProtocolsV1 as Table,
    DecodedCanonicalNominalSourceParameterProtocolsV1 as Decoded,
    NominalSourceParameterProtocolV1 as Record,
};
use scoop_identity::CallableTemplateOrigin;
use scoop_wire::{decode_canonical, encode};

pub(super) mod contracts;
mod rejection;
mod varargs;
mod wire;

const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-nominals/parameters.scoop"
));
const CALLABLES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-nominals/callables.scoop"
));

pub(super) fn required(export: &hir::ExportHirOutput) -> BTreeSet<CallableTemplateOrigin> {
    let roots = hir::CanonicalSourceNominalIdsV1::from_export_hir(export).unwrap();
    let nominals = hir::CanonicalNominalSourceContractsV1::from_export_hir(export, &roots).unwrap();
    let mut required = BTreeSet::new();
    for nominal in nominals.records() {
        required.extend(
            nominal
                .constructors()
                .values()
                .iter()
                .copied()
                .map(CallableTemplateOrigin::Constructor),
        );
        for member in nominal.members().values() {
            match member {
                hir::NestedSourceMemberRefV1::Function(id) => {
                    required.insert(CallableTemplateOrigin::Function(*id));
                }
                hir::NestedSourceMemberRefV1::GenericFunction(id) => {
                    required.insert(CallableTemplateOrigin::GenericFunction(*id));
                }
                hir::NestedSourceMemberRefV1::Property(_) => continue,
            }
        }
        if let hir::NominalSourceShapeV1::Enum(shape) = nominal.source_shape() {
            required.extend(
                shape
                    .variants()
                    .iter()
                    .map(|variant| CallableTemplateOrigin::VariantConstructor(variant.variant())),
            );
        }
    }
    required
}
fn table(export: &hir::ExportHirOutput) -> Table {
    Table::from_export_hir(export, &required(export)).unwrap()
}
fn declaration(
    export: &hir::ExportHir,
    owner: hir::ExportParameterOwner,
) -> Option<CallableTemplateOrigin> {
    Some(match owner {
        hir::ExportParameterOwner::Function(id) => match &export.function_identities[id] {
            hir::HirFunctionIdentity::Source(hir::HirSourceFunctionIdentity::Plain(record)) => {
                CallableTemplateOrigin::Function(record.id())
            }
            hir::HirFunctionIdentity::Source(hir::HirSourceFunctionIdentity::Generic(record)) => {
                CallableTemplateOrigin::GenericFunction(record.id())
            }
            _ => return None,
        },
        hir::ExportParameterOwner::ClassConstructor(id) => CallableTemplateOrigin::Constructor(
            export.constructor_identities[id].source_record()?.id(),
        ),
        hir::ExportParameterOwner::StructConstructor(id) => {
            CallableTemplateOrigin::Constructor(export.constructor_identities[id].id())
        }
        hir::ExportParameterOwner::VariantConstructor(reference) => {
            CallableTemplateOrigin::VariantConstructor(
                export.enum_member_identities[reference].id(),
            )
        }
    })
}

#[test]
fn nominal_parameter_sources_include_private_generic_constructors_and_variant_defaults() {
    with_source(SOURCE, |output, _| {
        let export = &output.output().export;
        let table = table(export);
        assert_eq!(table.records().len(), required(export).len());
        let dump = contracts::verify(export, &table);
        assert_eq!(
            dump,
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m23-type-source-nominals/parameters.snap"
            ))
        );
    });
}

#[test]
fn complete_parameter_sources_keep_legacy_records_and_bytes_unchanged() {
    let source = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/m23-type-source-dispatch/parameter-protocols.scoop"
    ));
    with_source(source, |output, _| {
        let full = table(&output.output().export);
        let old = hir::CanonicalInheritanceSourceParameterProtocolsV1::from_dependency_hir(output)
            .unwrap();
        assert!(full.records().len() >= old.records().len());
        for record in old.records() {
            let complete = full.get(record.owner()).unwrap();
            assert_eq!(encode(record).unwrap(), encode(complete).unwrap());
            assert_eq!(
                hir::InheritanceSourceParameterProtocolV1::try_from(complete.clone()).unwrap(),
                *record
            );
        }
    });
}
