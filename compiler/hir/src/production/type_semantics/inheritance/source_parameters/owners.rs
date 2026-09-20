use super::*;

pub(super) fn identity(
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
        ExportParameterOwner::VariantConstructor(_) => return None,
    })
}

pub(super) fn binders(
    export: &ExportHir,
    signatures: &HirInterfaceSignatureProjector<'_>,
    owner: ExportParameterOwner,
    meter: &mut BudgetMeter,
) -> Result<Vec<HirSignatureBinder>, Error> {
    let count = match owner {
        ExportParameterOwner::Function(id) => export.functions[id].type_param_count(),
        ExportParameterOwner::ClassConstructor(id) => export.classes
            [export.class_constructors[id].owner]
            .type_params
            .len(),
        ExportParameterOwner::StructConstructor(id) => export.structs
            [export.struct_constructors[id].owner]
            .type_params
            .len(),
        ExportParameterOwner::VariantConstructor(_) => {
            return Err(invalid(
                "variant constructor is outside inheritance parameter protocols",
            ));
        }
    };
    let path = WirePath::root();
    meter
        .check_table_entries(count as u64, &path)
        .map_err(resource)?;
    meter
        .charge_collection_slots(count as u64, &path)
        .map_err(resource)?;
    meter.charge_work(count as u64, &path).map_err(resource)?;
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
        ExportParameterOwner::VariantConstructor(_) => {
            return Err(invalid(
                "variant constructor is outside inheritance parameter protocols",
            ));
        }
    }
    .map_err(invalid)
}
