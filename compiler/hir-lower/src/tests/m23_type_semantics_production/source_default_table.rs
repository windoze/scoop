use super::source_dispatch::with_hir_source;
use super::source_inventory::identity_closure;
use super::*;
use hir::{
    CanonicalDefaultSourceTemplatesV1 as Table,
    DecodedCanonicalDefaultSourceTemplatesV1 as Decoded,
    NominalDefaultSourceProductionV1 as Production,
};
use scoop_identity::CallableTemplateOrigin;
use scoop_wire::{decode_canonical, encode};

mod coverage;
mod origins;
mod varargs;
mod wire;
const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-defaults/table.scoop"
));

fn bytes(value: &Table) -> Vec<u8> {
    encode(&value.index_locals().unwrap()).unwrap()
}
pub(super) fn declaration(
    export: &hir::ExportHir,
    owner: hir::ExportParameterOwner,
) -> CallableTemplateOrigin {
    match owner {
        hir::ExportParameterOwner::Function(id) => match &export.function_identities[id] {
            hir::HirFunctionIdentity::Source(hir::HirSourceFunctionIdentity::Plain(record)) => {
                CallableTemplateOrigin::Function(record.id())
            }
            hir::HirFunctionIdentity::Source(hir::HirSourceFunctionIdentity::Generic(record)) => {
                CallableTemplateOrigin::GenericFunction(record.id())
            }
            _ => panic!("source function required"),
        },
        hir::ExportParameterOwner::ClassConstructor(id) => CallableTemplateOrigin::Constructor(
            export.constructor_identities[id]
                .source_record()
                .unwrap()
                .id(),
        ),
        hir::ExportParameterOwner::StructConstructor(id) => {
            CallableTemplateOrigin::Constructor(export.constructor_identities[id].id())
        }
        hir::ExportParameterOwner::VariantConstructor(id) => {
            CallableTemplateOrigin::VariantConstructor(export.enum_member_identities[id].id())
        }
    }
}
fn function(export: &hir::ExportHir, name: &str) -> hir::ExportParameterOwner {
    hir::ExportParameterOwner::Function(
        export
            .functions
            .iter()
            .find(|(_, f)| f.name == name)
            .unwrap()
            .0,
    )
}
pub(super) fn owner_name(export: &hir::ExportHir, owner: hir::ExportParameterOwner) -> String {
    match owner {
        hir::ExportParameterOwner::Function(id) => export.functions[id].name.clone(),
        hir::ExportParameterOwner::ClassConstructor(id) => format!(
            "{}.constructor",
            export.classes[export.class_constructors[id].owner].name
        ),
        hir::ExportParameterOwner::StructConstructor(id) => format!(
            "{}.constructor",
            export.structs[export.struct_constructors[id].owner].name
        ),
        hir::ExportParameterOwner::VariantConstructor(id) => format!(
            "{}.variant{}",
            export.enums[id.enumeration()].name,
            id.local_index()
        ),
    }
}
fn restored(output: &hir::DependencyHirOutput, table: &Table) -> Table {
    let bytes = bytes(table);
    let input: Decoded = decode_canonical(&bytes).unwrap();
    assert_eq!(encode(&input).unwrap(), bytes);
    let table = input.resolve(&mut identity_closure(output)).unwrap();
    assert_eq!(encode(&table.index_locals().unwrap()).unwrap(), bytes);
    table
}

#[test]
fn complete_nominal_default_source_production_matches_raw_parameter_omissions() {
    with_hir_source(SOURCE, |output, _| {
        let export = output.output().export.module();
        let production = Production::from_dependency_hir(output).unwrap();
        let required = super::source_nominal_parameters::required(&output.output().export);
        let mut expected = BTreeSet::new();
        let mut summary = Vec::new();
        for interface in &export.source_parameter_interfaces {
            if matches!(interface.owner, hir::ExportParameterOwner::Function(id) if export.functions[id].method.is_none())
            {
                continue;
            }
            let owner = declaration(export, interface.owner);
            if !required.contains(&owner) {
                continue;
            }
            let protocol = production.parameters().get(owner).unwrap();
            assert_eq!(protocol.parameters().len(), interface.parameters.len());
            let mut defaults = Vec::new();
            for (position, parameter) in interface.parameters.iter().enumerate() {
                if matches!(
                    parameter.calling,
                    hir::ExportParameterCalling::Default { .. }
                        | hir::ExportParameterCalling::Vararg {
                            omission: hir::ExportVarargOmission::Default(_),
                            ..
                        }
                ) {
                    let key = hir::ProtectedDefaultTemplateKeyV1::try_new(owner, position as u32)
                        .unwrap();
                    expected.insert(key);
                    let body = production.templates().get(key).unwrap();
                    defaults.push(format!(
                        "{position}:inherited={}",
                        body.definition_root().declaration() != key.owner()
                    ));
                }
            }
            summary.push(format!(
                "{}: parameters={}, defaults=[{}]",
                owner_name(export, interface.owner),
                protocol.parameters().len(),
                defaults.join(",")
            ));
        }
        assert_eq!(
            expected,
            production
                .templates()
                .records()
                .iter()
                .map(|r| r.key())
                .collect()
        );
        assert!(
            production
                .parameters()
                .get(declaration(export, function(export, "topLevel")))
                .is_none()
        );
        summary.sort();
        assert_eq!(
            summary.join("\n") + "\n",
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m23-type-source-defaults/table.snap"
            ))
        );
        let parameters: hir::DecodedCanonicalNominalSourceParameterProtocolsV1 =
            decode_canonical(&encode(production.parameters()).unwrap()).unwrap();
        let parameters = parameters.resolve(&mut identity_closure(output)).unwrap();
        let templates = restored(output, production.templates());
        assert_eq!(&templates, production.templates());
        templates.validate_parameter_coverage(&parameters).unwrap();
        assert_eq!(
            Production::from_export_hir(&output.output().export).unwrap(),
            production
        );
        assert_eq!(Production::from_dependency_hir(output).unwrap(), production);
    });
}

#[test]
fn complete_default_sources_cover_nested_generic_static_and_variant_owners() {
    for source in [
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-type-source-nominals/parameters.scoop"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-type-source-defaults/inherited-generics.scoop"
        )),
    ] {
        with_hir_source(source, |output, _| {
            let production = Production::from_dependency_hir(output).unwrap();
            assert!(!production.templates().records().is_empty());
            let restored = restored(output, production.templates());
            assert_eq!(&restored, production.templates());
            restored
                .validate_parameter_coverage(production.parameters())
                .unwrap();
            assert!(
                production
                    .parameters()
                    .records()
                    .iter()
                    .any(|r| r.parameters().is_empty())
                    || production
                        .templates()
                        .records()
                        .iter()
                        .any(|r| r.key().owner() != r.definition_root().declaration())
            );
        });
    }
}

#[test]
fn nominal_source_production_retains_empty_protocols_without_fake_defaults() {
    with_hir_source(
        "public class Empty { public fun zero(): Int = 0 }\nprivate fun top(value: Int = 1): Int = value",
        |output, _| {
            let production = Production::from_dependency_hir(output).unwrap();
            assert!(!production.parameters().records().is_empty());
            assert!(
                production
                    .parameters()
                    .records()
                    .iter()
                    .all(|r| r.parameters().is_empty())
            );
            assert!(production.templates().records().is_empty());
            assert_eq!(bytes(production.templates()), [0x80]);
            let table = restored(output, production.templates());
            table
                .validate_parameter_coverage(production.parameters())
                .unwrap();
        },
    );
}
