use super::*;
use crate::shape_definitions::emit_dispatch_view_v1;
use std::collections::BTreeMap;

pub(super) fn emit<'ctx>(
    emission: &TypeDescriptorEmission<'_, 'ctx>,
    descriptor: &TypeDescriptor,
    shapes: &mut EmittedStrongShapeDefinitionsV1<'ctx>,
) -> Result<Vec<PointerValue<'ctx>>, CodegenError> {
    let mut pointers = BTreeMap::new();
    for table in &descriptor.itables {
        let id = table.identity_record().id();
        if table.layout().storage() != id {
            continue;
        }
        if table.layout().offset() != 0 {
            return Err(CodegenError(
                "owning interface table has a nonzero offset".into(),
            ));
        }
        let values = dispatch_values(
            emission.llvm,
            table.physical_slots(),
            &emission.module.functions,
            &emission.module.meta.external_callables,
        )?;
        let pointer = emit_dispatch_definition_v1(
            emission.context,
            emission.llvm,
            emission.surface,
            id,
            &values,
            shapes,
        )?
        .runtime_pointer()
        .unwrap_or_else(|| ptr_ty(emission.context).const_null());
        pointers.insert(id, pointer);
    }
    for table in &descriptor.itables {
        let id = table.identity_record().id();
        if table.layout().storage() == id {
            continue;
        }
        let storage = descriptor
            .itables
            .iter()
            .find(|storage| storage.identity_record().id() == table.layout().storage())
            .ok_or_else(|| CodegenError("missing interface table array".into()))?;
        let start = table.layout().offset() as usize;
        let end = start
            .checked_add(table.physical_slots().len())
            .ok_or_else(|| CodegenError("interface table view size overflow".into()))?;
        if table.physical_slots().is_empty()
            || storage.physical_slots().get(start..end) != Some(table.physical_slots())
        {
            return Err(CodegenError(
                "interface table view does not match its storage".into(),
            ));
        }
        let pointer = emit_dispatch_view_v1(
            emission.context,
            emission.llvm,
            emission.surface,
            table,
            shapes,
        )?
        .runtime_pointer()
        .expect("a nonempty interface view has a pointer");
        pointers.insert(id, pointer);
    }
    Ok(descriptor
        .itables
        .iter()
        .map(|table| pointers[&table.identity_record().id()])
        .collect())
}
