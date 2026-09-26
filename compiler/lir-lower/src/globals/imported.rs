use super::*;

pub(super) fn lower(
    inputs: &GlobalLoweringInputs<'_>,
    global: &mir::Global,
    provider: scoop_identity::ConeIdentity,
    storage: scoop_identity::PersistentStaticStorageId,
) -> Result<lir::Global, StrongLirLoweringError> {
    let error = || StrongLirLoweringError::DependencyStorageBinding { provider, storage };
    let definition = inputs
        .selected_layout
        .and_then(|selected| {
            selected
                .physical_imports()
                .records()
                .iter()
                .find(|definition| {
                    definition.provider() == provider
                        && definition.subject()
                            == lir::ExternalStrongShapeSubjectV1::StaticStorage(storage)
                })
        })
        .ok_or_else(error)?;
    let lir::ShapeLinkContractV1::StaticStorage { storage_projection } = definition.contract()
    else {
        return Err(error());
    };
    let ty = lir_type(&global.ty);
    let scan = safepoints::root_scan(inputs.context, &ty, inputs.structs, inputs.enums, 0)?;
    if storage_projection.value_layout().layout_key().exact_type()
        != exact_type_record(inputs.module, &global.ty).id()
        || storage_projection.scan_program() != &scan
    {
        return Err(error());
    }
    Ok(lir::Global {
        address_kind: lir::PointerKind::Raw,
        scan,
        init: lir::GlobalInit::ImportedStorage {
            definition: Box::new(definition.clone()),
            ty,
        },
    })
}
