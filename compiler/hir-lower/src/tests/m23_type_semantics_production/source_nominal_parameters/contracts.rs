use super::*;

pub(in crate::tests::m23_type_semantics_production) fn verify(
    export: &hir::ExportHirOutput,
    table: &Table,
) -> String {
    let callable_ids = table
        .records()
        .iter()
        .map(Record::owner)
        .filter(|id| !matches!(id, CallableTemplateOrigin::Constructor(_)))
        .collect();
    let callables = hir::CanonicalNominalSourceCallablesV1::from_export_hir(
        export,
        &callable_ids,
        &mut meter(),
    )
    .unwrap();
    let module = export.module();
    let mut rows = Vec::new();
    for interface in &module.source_parameter_interfaces {
        let Some(id) = declaration(module, interface.owner) else {
            continue;
        };
        let Some(record) = table.get(id) else {
            continue;
        };
        assert_eq!(record.parameters().len(), interface.parameters.len());
        for (actual, source) in record.parameters().iter().zip(&interface.parameters) {
            assert_eq!(actual.shape().name().as_str(), source.name);
            let origin = actual.definition_origin().origin();
            assert_eq!(
                origin.source(),
                &module.source_files[source.origin.file as usize].identity
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
        if let Some(callable) = callables.get(id) {
            assert_eq!(
                record
                    .parameters()
                    .iter()
                    .map(|p| p.shape())
                    .collect::<Vec<_>>(),
                callable
                    .payload()
                    .parameters()
                    .parameters()
                    .iter()
                    .collect::<Vec<_>>()
            );
        } else {
            let key = match interface.owner {
                hir::ExportParameterOwner::ClassConstructor(id) => module.constructor_identities
                    [id]
                    .source_record()
                    .unwrap()
                    .key(),
                hir::ExportParameterOwner::StructConstructor(id) => {
                    module.constructor_identities[id].key()
                }
                _ => panic!("constructor source"),
            };
            let scoop_identity::DuplicateSignatureKey::Constructor { parameters } =
                key.duplicate_signature()
            else {
                panic!("constructor signature");
            };
            assert_eq!(
                record
                    .parameters()
                    .iter()
                    .map(|p| p.shape().value_type())
                    .collect::<Vec<_>>(),
                parameters.iter().collect::<Vec<_>>()
            );
        }
        let name = match interface.owner {
            hir::ExportParameterOwner::Function(id) => module.functions[id].name.clone(),
            hir::ExportParameterOwner::ClassConstructor(id) => format!(
                "{}.constructor",
                module.classes[module.class_constructors[id].owner].name
            ),
            hir::ExportParameterOwner::StructConstructor(id) => format!(
                "{}.constructor",
                module.structs[module.struct_constructors[id].owner].name
            ),
            hir::ExportParameterOwner::VariantConstructor(reference) => {
                let enumeration = &module.enums[reference.enumeration()];
                format!(
                    "{}.{}",
                    enumeration.name,
                    enumeration.variants[reference.local_index() as usize].name
                )
            }
        };
        let parameters = record
            .parameters()
            .iter()
            .map(|p| format!("{}:{:?}", p.shape().name().as_str(), p.calling_kind()))
            .collect::<Vec<_>>()
            .join(", ");
        rows.push(format!("{name}({parameters})\n"));
    }
    assert_eq!(rows.len(), table.records().len());
    rows.sort();
    rows.concat()
}
