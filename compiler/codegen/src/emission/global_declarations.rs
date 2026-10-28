//! Emit only the ordinary globals selected for this LLVM module.

use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn emit<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    module: &Module,
    surface: &scoop_lir::ObjectSymbolSurfaceV1,
    target_data: &inkwell::targets::TargetData,
    profile: ValidatedBackendProfile,
    selection: StrongObjectEmissionSelection<'_>,
    references: &GlobalReferences,
    descriptors: TypeDescriptorGlobals<'_, 'ctx>,
) -> Result<BTreeMap<scoop_lir::GlobalId, GlobalValue<'ctx>>, CodegenError> {
    let managed_address_space = profile.managed_address_space_contract();
    let i8_ty = context.i8_type();
    // Ordinary globals are disjoint from descriptor identities.
    let mut globals = BTreeMap::new();
    for &id in &references.globals {
        let global = &module.globals[id];
        match &global.init {
            GlobalInit::ImportedImmortal { definition } => {
                let llvm_global = llvm.add_global(
                    i8_ty,
                    Some(managed_address_space.inkwell()),
                    definition.symbol(),
                );
                llvm_global.set_constant(true);
                globals.insert(id, llvm_global);
            }
            GlobalInit::ImportedStorage { definition, ty } => {
                let scoop_lir::ShapeLinkContractV1::StaticStorage { storage_projection } =
                    definition.contract()
                else {
                    return Err(CodegenError(format!(
                        "imported storage `{}` has a non-storage contract",
                        definition.symbol()
                    )));
                };
                let logical_ty = basic_ty(
                    context,
                    &module.structs,
                    &module.enums,
                    managed_address_space,
                    ty,
                )?;
                let logical_size = target_data.get_store_size(&logical_ty);
                let logical_alignment = target_data.get_abi_alignment(&logical_ty);
                if logical_size != storage_projection.byte_size()
                    || u64::from(logical_alignment) != storage_projection.required_alignment()
                {
                    return Err(CodegenError(format!(
                        "imported storage `{}` disagrees with its value layout",
                        definition.symbol()
                    )));
                }
                let storage_ty = if logical_size == 0 {
                    i8_ty.into()
                } else {
                    logical_ty
                };
                let llvm_global = llvm.add_global(storage_ty, None, definition.symbol());
                llvm_global.set_alignment(logical_alignment);
                globals.insert(id, llvm_global);
            }
            GlobalInit::StringConst { .. }
            | GlobalInit::ByteArrayConst { .. }
            | GlobalInit::EmbeddedFileArrayConst { .. } => {
                let emitter = immortals::ImmortalEmitter {
                    context,
                    llvm,
                    module,
                    target_data,
                    descriptors,
                    managed_address_space,
                    globals: &globals,
                    define: selection.defines_non_callable(),
                };
                globals.insert(id, emitter.emit(global)?);
            }
            GlobalInit::CString { identity, value } => {
                // [N+1 x i8] c"...\00" (e.g. trap messages), emitted as a
                // callable-owned support atom with a stable boundary symbol.
                let bytes = value.as_bytes();
                let ty = i8_ty.array_type(bytes.len() as u32 + 1);
                let llvm_global = llvm.add_global(ty, None, global.symbol());
                llvm_global.set_constant(true);
                apply_persistent_linkage(
                    &llvm_global,
                    identity.symbol_request(),
                    selection.defines_callable(identity.owner()),
                )?;
                if selection.defines_callable(identity.owner()) {
                    llvm_global.set_initializer(&context.const_string(bytes, true));
                    crate::metadata_sections::set_section(llvm_global, profile.c_string_section());
                }
                globals.insert(id, llvm_global);
            }
            GlobalInit::Storage { .. } | GlobalInit::RawStorage { .. } => {
                let emitter = storage::StorageEmitter {
                    context,
                    llvm,
                    module,
                    target_data,
                    managed_address_space,
                    globals: &globals,
                    surface,
                    profile,
                    define: selection.defines_non_callable(),
                };
                globals.insert(id, emitter.emit(global)?);
            }
        }
    }
    atom_boundaries::emit_global_atom_boundaries_v1(
        llvm,
        surface,
        references.globals.iter().filter_map(|id| {
            let global = &module.globals[*id];
            let GlobalInit::CString { identity, .. } = &global.init else {
                return None;
            };
            if !selection.defines_callable(identity.owner()) {
                return None;
            }
            let owner = globals[id];
            Some(atom_boundaries::GlobalAtomMaterializationV1::new(
                identity.atom_record().id(),
                owner,
            ))
        }),
    )?;
    Ok(globals)
}
