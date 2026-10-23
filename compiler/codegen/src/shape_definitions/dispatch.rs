use super::*;

pub(crate) fn emit_dispatch_definition_v1<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    surface: &ObjectSymbolSurfaceV1,
    table: PersistentDispatchTableId,
    values: &[PointerValue<'ctx>],
    emitted: &mut EmittedStrongShapeDefinitionsV1<'ctx>,
) -> Result<EmittedDispatchDefinitionV1<'ctx>, CodegenError> {
    let definition = require_definition(
        surface,
        StrongDefinitionEntity::dispatch_table(table),
        StrongDefinitionRole::DispatchTable,
    )?;
    let symbol = definition.primary_symbol().symbol();

    let (global, is_empty) = if values.is_empty() {
        let global = llvm.add_global(context.i8_type(), None, symbol.as_str());
        global.set_initializer(&context.i8_type().const_zero());
        (global, true)
    } else {
        let value = context
            .ptr_type(inkwell::AddressSpace::default())
            .const_array(values);
        let global = llvm.add_global(value.get_type(), None, symbol.as_str());
        global.set_initializer(&value);
        (global, false)
    };
    global.set_constant(true);
    crate::emission::apply_persistent_linkage(&global, definition.primary_symbol(), true)?;
    let result = EmittedDispatchDefinitionV1 { global, is_empty };
    if emitted.dispatch_tables.insert(table, result).is_some() {
        return Err(CodegenError(format!(
            "dispatch-table definition {table} was emitted more than once"
        )));
    }
    emitted.record_atom(definition.primary_atom(), global);
    Ok(result)
}

pub(crate) fn emit_dispatch_view_v1<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    surface: &ObjectSymbolSurfaceV1,
    table: &scoop_lir::ItableRecord,
    emitted: &mut EmittedStrongShapeDefinitionsV1<'ctx>,
) -> Result<EmittedDispatchDefinitionV1<'ctx>, CodegenError> {
    use inkwell::{types::AsTypeRef, values::AsValueRef};
    let id = table.identity_record().id();
    let definition = require_definition(
        surface,
        StrongDefinitionEntity::dispatch_table(id),
        StrongDefinitionRole::DispatchTable,
    )?;
    let storage = emitted
        .dispatch_tables
        .get(&table.layout().storage())
        .ok_or_else(|| {
            CodegenError(format!(
                "missing interface table storage {}",
                table.layout().storage()
            ))
        })?;
    let offset = context
        .i64_type()
        .const_int(u64::from(table.layout().offset()) * 8, false);
    let mut indices = [offset.as_value_ref()];
    let pointer = unsafe {
        llvm_sys::core::LLVMConstGEP2(
            context.i8_type().as_type_ref(),
            storage.global.as_value_ref(),
            indices.as_mut_ptr(),
            1,
        )
    };
    let ty = context
        .ptr_type(inkwell::AddressSpace::default())
        .array_type(
            u32::try_from(table.physical_slots().len())
                .map_err(|_| CodegenError("interface table is too large".into()))?,
        );
    let symbol = definition.primary_symbol().symbol();
    let name = std::ffi::CString::new(symbol.as_str())
        .map_err(|_| CodegenError("interface table symbol contains NUL".into()))?;
    let alias = unsafe {
        llvm_sys::core::LLVMAddAlias2(
            llvm.as_mut_ptr(),
            ty.as_type_ref(),
            0,
            pointer,
            name.as_ptr(),
        )
    };
    let global = unsafe { GlobalValue::new(alias) };
    // Like atom-boundary aliases, Mach-O weak definitions are materialized
    // after LLVM emission. The alias itself owns no additional bytes.
    global.set_linkage(inkwell::module::Linkage::External);
    let result = EmittedDispatchDefinitionV1 {
        global,
        is_empty: false,
    };
    if emitted.dispatch_tables.insert(id, result).is_some() {
        return Err(CodegenError(format!("duplicate interface view {id}")));
    }
    emitted.record_atom(definition.primary_atom(), global);
    Ok(result)
}
