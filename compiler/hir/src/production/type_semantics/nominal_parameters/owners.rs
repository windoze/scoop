use super::*;

pub(in crate::production::type_semantics) fn identity(
    export: &ExportHir,
    owner: ExportParameterOwner,
) -> Option<(CallableTemplateOrigin, &SourceDeclarationKey)> {
    Some(match owner {
        ExportParameterOwner::Function(id) => match &export.function_identities[id] {
            HirFunctionIdentity::Source(HirSourceFunctionIdentity::Plain(record)) => {
                (CallableTemplateOrigin::Function(record.id()), record.key())
            }
            HirFunctionIdentity::Source(HirSourceFunctionIdentity::Generic(record)) => (
                CallableTemplateOrigin::GenericFunction(record.id()),
                record.key(),
            ),
            _ => return None,
        },
        ExportParameterOwner::ClassConstructor(id) => {
            let identity = export.constructor_identities[id].source_record()?;
            (
                CallableTemplateOrigin::Constructor(identity.id()),
                identity.key(),
            )
        }
        ExportParameterOwner::StructConstructor(id) => {
            let identity = &export.constructor_identities[id];
            (
                CallableTemplateOrigin::Constructor(identity.id()),
                identity.key(),
            )
        }
        ExportParameterOwner::VariantConstructor(reference) => {
            let source = export.nominal_identities[reference.enumeration()].source()?;
            let identity = &export.enum_member_identities[reference];
            (
                CallableTemplateOrigin::VariantConstructor(identity.id()),
                source.declaration(),
            )
        }
    })
}

pub(super) fn binders(
    export: &ExportHir,
    signatures: &HirInterfaceSignatureProjector<'_>,
    owner: ExportParameterOwner,
) -> Result<Vec<HirSignatureBinder>, Error> {
    match owner {
        ExportParameterOwner::Function(id) => signatures.function_binders(&export.functions[id]),
        ExportParameterOwner::ClassConstructor(id) => signatures.binder_frame(
            &export.classes[export.class_constructors[id].owner].type_params,
            0,
        ),
        ExportParameterOwner::StructConstructor(id) => signatures.binder_frame(
            &export.structs[export.struct_constructors[id].owner].type_params,
            0,
        ),
        ExportParameterOwner::VariantConstructor(reference) => {
            signatures.binder_frame(&export.enums[reference.enumeration()].type_params, 0)
        }
    }
    .map_err(invalid)
}
