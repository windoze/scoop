//! Emit managed storage and explicit raw globals with their distinct lifetimes.

use super::*;

mod tls;

pub(super) struct StorageEmitter<'a, 'ctx> {
    pub(super) context: &'ctx Context,
    pub(super) llvm: &'a LlvmModule<'ctx>,
    pub(super) module: &'a Module,
    pub(super) target_data: &'a inkwell::targets::TargetData,
    pub(super) managed_address_space: ManagedAddressSpace,
    pub(super) globals: &'a [Option<GlobalValue<'ctx>>],
    pub(super) surface: &'a scoop_lir::ObjectSymbolSurfaceV1,
    pub(super) profile: ValidatedBackendProfile,
    pub(super) define: bool,
}

impl<'ctx> StorageEmitter<'_, 'ctx> {
    pub(super) fn emit(&self, global: &Global) -> Result<GlobalValue<'ctx>, CodegenError> {
        let (identity, ty, thread_local) = match &global.init {
            GlobalInit::Storage { identity, ty, .. } => (identity, ty, false),
            GlobalInit::RawStorage {
                identity,
                ty,
                thread_local,
                ..
            } => (identity, ty, *thread_local),
            _ => unreachable!("storage emission only receives storage globals"),
        };
        let logical_ty = basic_ty(
            self.context,
            &self.module.structs,
            &self.module.enums,
            self.managed_address_space,
            ty,
        )?;
        let size = self.target_data.get_store_size(&logical_ty);
        let alignment = self.target_data.get_abi_alignment(&logical_ty);
        let storage_ty = if size == 0 {
            self.context.i8_type().into()
        } else {
            logical_ty
        };
        let value = self.llvm.add_global(storage_ty, None, global.symbol());
        value.set_alignment(alignment);
        value.set_thread_local(thread_local);
        apply_persistent_linkage(&value, identity.symbol_request(), self.define)?;
        if !self.define {
            return Ok(value);
        }
        let initializer = if size == 0 {
            self.context.i8_type().const_zero().into()
        } else {
            match &global.init {
                GlobalInit::Storage {
                    initial_state: LirStaticInitialState::ZeroedForRuntimeUnit,
                    ..
                } => storage_ty.const_zero(),
                GlobalInit::Storage {
                    initial_state: LirStaticInitialState::EncodedStaticValue { payload },
                    ..
                }
                | GlobalInit::RawStorage {
                    initializer: payload,
                    ..
                } => llvm_constant(
                    self.context,
                    &self.module.structs,
                    &self.module.enums,
                    self.globals,
                    self.managed_address_space,
                    ty,
                    payload,
                )?,
                _ => unreachable!("storage emission only receives storage globals"),
            }
        };
        value.set_initializer(&initializer);
        if !thread_local {
            let zeroed = matches!(
                &global.init,
                GlobalInit::Storage {
                    initial_state: LirStaticInitialState::ZeroedForRuntimeUnit,
                    ..
                }
            );
            value.set_section(Some(if zeroed {
                self.profile.zero_fill_storage_section()
            } else {
                self.profile.writable_storage_section()
            }));
        }
        if matches!(global.init, GlobalInit::RawStorage { .. }) {
            let definition = self
                .surface
                .plans()
                .iter()
                .find(|plan| {
                    plan.owner()
                        == scoop_lir::StrongDefinitionEntity::static_storage(
                            identity.identity_record().id(),
                        )
                        && plan.definition_role() == scoop_lir::StrongDefinitionRole::StaticStorage
                })
                .ok_or_else(|| {
                    CodegenError(format!(
                        "raw storage @{} has no definition plan",
                        global.symbol()
                    ))
                })?;
            if thread_local {
                tls::boundaries(
                    self.llvm,
                    definition,
                    global.symbol(),
                    size.max(1),
                    self.module.meta.target_profile,
                )?;
            } else {
                atom_boundaries::emit_global_atom_boundaries_v1(
                    self.llvm,
                    self.target_data,
                    self.surface,
                    [atom_boundaries::GlobalAtomMaterializationV1::new(
                        definition.primary_atom(),
                        value,
                    )],
                )?;
            }
        }
        Ok(value)
    }
}
