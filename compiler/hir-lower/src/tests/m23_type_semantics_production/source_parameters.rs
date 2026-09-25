use super::source_dispatch::with_hir_source as with_source;
use super::*;
use hir::{
    CanonicalInheritanceSourceParameterProtocolsV1 as Table,
    DecodedCanonicalInheritanceSourceParameterProtocolsV1 as Decoded,
    InheritanceSourceParameterProtocolV1 as Record,
};
use scoop_identity::CallableTemplateOrigin;
use scoop_wire::{decode_canonical, encode};

mod varargs;
mod wire;

const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../tests/fixtures/m23-type-source-dispatch/parameter-protocols.scoop"
));

fn table(output: &hir::DependencyHirOutput) -> Table {
    Table::from_dependency_hir(output).unwrap()
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
        hir::ExportParameterOwner::VariantConstructor(_) => return None,
    })
}
fn render_and_verify(export: &hir::ExportHir, table: &Table) -> String {
    let mut lines = Vec::new();
    for interface in &export.source_parameter_interfaces {
        let Some(owner) = declaration(export, interface.owner) else {
            continue;
        };
        let Some(record) = table.get(owner) else {
            continue;
        };
        assert_eq!(record.parameters().len(), interface.parameters.len());
        for (actual, source) in record.parameters().iter().zip(&interface.parameters) {
            assert_eq!(actual.shape().name().as_str(), source.name);
            let origin = actual.definition_origin().origin();
            assert_eq!(
                origin.source(),
                &export.source_files[source.origin.file as usize].identity
            );
            assert_eq!(
                origin.span(),
                scoop_identity::SourceSpan::new(
                    source.origin.span.start.into(),
                    source.origin.span.end.into()
                )
                .unwrap()
            );
            let expected = match source.calling {
                hir::ExportParameterCalling::Required { .. } => {
                    hir::ProtectedParameterCallingKindV1::Required
                }
                hir::ExportParameterCalling::Default { .. } => {
                    hir::ProtectedParameterCallingKindV1::Default
                }
                hir::ExportParameterCalling::Vararg {
                    omission: hir::ExportVarargOmission::EmptyArray,
                    ..
                } => hir::ProtectedParameterCallingKindV1::VarargEmpty,
                hir::ExportParameterCalling::Vararg {
                    omission: hir::ExportVarargOmission::Default(_),
                    ..
                } => hir::ProtectedParameterCallingKindV1::VarargDefault,
            };
            assert_eq!(actual.calling_kind(), expected);
        }
        let name = match interface.owner {
            hir::ExportParameterOwner::Function(id) => export.functions[id].name.clone(),
            hir::ExportParameterOwner::ClassConstructor(id) => format!(
                "{}.constructor",
                export.classes[export.class_constructors[id].owner].name
            ),
            hir::ExportParameterOwner::StructConstructor(id) => format!(
                "{}.constructor",
                export.structs[export.struct_constructors[id].owner].name
            ),
            hir::ExportParameterOwner::VariantConstructor(_) => unreachable!(),
        };
        let parameters = record
            .parameters()
            .iter()
            .map(|p| format!("{}:{:?}", p.shape().name().as_str(), p.calling_kind()))
            .collect::<Vec<_>>()
            .join(", ");
        lines.push(format!("{name}({parameters})\n"));
    }
    assert_eq!(lines.len(), table.records().len());
    lines.sort();
    lines.concat()
}

#[test]
fn inheritance_parameter_sources_preserve_names_categories_origins_and_empty_records() {
    with_source(SOURCE, |output, _| {
        let table = table(output);
        assert_eq!(
            render_and_verify(output.output().export.module(), &table),
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/m23-type-source-dispatch/parameter-protocols.snap"
            ))
        );
        let inventory =
            hir::CanonicalSourceInheritanceInventoriesV1::from_dependency_hir(output).unwrap();
        let mut expected = BTreeSet::new();
        for owner in inventory.records() {
            expected.extend(
                owner
                    .constructors()
                    .values()
                    .iter()
                    .copied()
                    .map(CallableTemplateOrigin::Constructor),
            );
            for member in owner.protected_members().values() {
                if let hir::ProtectedDeclarationRefV1::Callable(callable) = member {
                    if !matches!(callable.declaration(), CallableTemplateOrigin::Accessor(_)) {
                        expected.insert(callable.declaration());
                    }
                }
            }
        }
        assert_eq!(
            table
                .records()
                .iter()
                .map(Record::owner)
                .collect::<BTreeSet<_>>(),
            expected
        );
    });
}

#[test]
fn inheritance_parameter_source_bytes_replay_and_are_deterministic() {
    for source in [
        SOURCE,
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/m23-type-source-dispatch/protected-callables.scoop"
        )),
    ] {
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
