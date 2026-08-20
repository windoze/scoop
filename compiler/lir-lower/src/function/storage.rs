use super::*;

impl FunctionLowerer<'_> {
    fn native_storage_protocol(&mut self, global: mir::GlobalId) -> lir::NativeStorageProtocol {
        if self.gc_effect == mir::GcEffect::NoGc
            && matches!(
                self.module.globals[global].storage,
                mir::GlobalStorage::Extern {
                    thread_local: false,
                    ..
                }
            )
        {
            lir::NativeStorageProtocol::NoTransition
        } else {
            lir::NativeStorageProtocol::NativeSafe {
                safepoint: self.new_safepoint(lir::SafepointSiteRole::NativeSafeTransition),
                roots: lir::NativeSafeRootSet::default(),
            }
        }
    }

    pub(super) fn lower_global_read(
        &mut self,
        ty: &mir::Type,
        source: mir::GlobalId,
    ) -> lir::Value {
        match self.storage_globals[&source] {
            StorageGlobal::Local(global) => {
                let out_ty = self.value_type(ty);
                let out = self.new_temp(out_ty);
                self.push(lir::Instruction::GlobalLoad { out, global });
                lir::Value::Temp(out)
            }
            StorageGlobal::Native(global) => {
                let out = self.new_temp(self.c_storage_type(ty));
                let protocol = self.native_storage_protocol(source);
                self.push(lir::Instruction::NativeGlobalLoad {
                    out,
                    global,
                    protocol,
                });
                self.restore_c_value(ty, lir::Value::Temp(out))
            }
        }
    }

    pub(super) fn lower_global_address(&mut self, source: mir::GlobalId) -> lir::Value {
        let out = self.new_temp(lir::RAW_PTR);
        match self.storage_globals[&source] {
            StorageGlobal::Local(global) => {
                self.push(lir::Instruction::GlobalAddress { out, global });
            }
            StorageGlobal::Native(global) => {
                let protocol = self.native_storage_protocol(source);
                self.push(lir::Instruction::NativeGlobalAddress {
                    out,
                    global,
                    protocol,
                });
            }
        }
        lir::Value::Temp(out)
    }

    pub(super) fn lower_global_assign(
        &mut self,
        source: mir::GlobalId,
        expression: &mir::Expr,
    ) -> StorageResult<()> {
        let value = self.lower_expr(expression)?;
        match self.storage_globals[&source] {
            StorageGlobal::Local(global) => {
                self.push(lir::Instruction::GlobalStore { global, value });
            }
            StorageGlobal::Native(global) => {
                let value = self.project_c_value(&expression.ty, value);
                let protocol = self.native_storage_protocol(source);
                self.push(lir::Instruction::NativeGlobalStore {
                    global,
                    value,
                    protocol,
                });
            }
        }
        Ok(())
    }
}
