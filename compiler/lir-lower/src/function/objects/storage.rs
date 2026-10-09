//! Heap operands are evaluated before storage classification elides a payload.

use super::*;

impl FunctionLowerer<'_> {
    pub(in crate::function) fn lower_release_field(
        &mut self,
        class: mir::ClassId,
        index: u32,
        ty: &mir::Type,
    ) -> StorageResult<lir::Value> {
        let storage = self.value_type(ty);
        match abi::classify_storage(self.context, storage, self.structs, self.enums)? {
            abi::ValueStorage::ZeroSized(representation) => {
                Ok(self.logical_zst_value(ty, representation))
            }
            abi::ValueStorage::NonZero(value) => {
                let (offsets, _, _) = class_shape(
                    self.context,
                    self.module,
                    self.enums,
                    &self.module.classes[class],
                )?;
                let out = self.new_temp(value.storage_type().clone());
                self.push(lir::Instruction::ReleaseFieldLoad {
                    out,
                    offset: offsets[index as usize],
                });
                Ok(lir::Value::Temp(out))
            }
        }
    }
    pub(in crate::function) fn load_heap_value(
        &mut self,
        object: lir::Value,
        offset: u64,
        ty: &mir::Type,
    ) -> StorageResult<lir::Value> {
        let storage = self.value_type(ty);
        match abi::classify_storage(self.context, storage, self.structs, self.enums)? {
            abi::ValueStorage::ZeroSized(representation) => {
                Ok(self.logical_zst_value(ty, representation))
            }
            abi::ValueStorage::NonZero(value) => {
                let out = self.load_at_offset(object, offset, value.storage_type().clone());
                Ok(lir::Value::Temp(out))
            }
        }
    }

    pub(in crate::function) fn store_heap_value(
        &mut self,
        object: lir::Value,
        offset: u64,
        value: lir::Value,
        ty: &mir::Type,
    ) -> StorageResult<()> {
        let storage = self.value_type(ty);
        match abi::classify_storage(self.context, storage, self.structs, self.enums)? {
            abi::ValueStorage::ZeroSized(_) => Ok(()),
            abi::ValueStorage::NonZero(storage) => {
                self.store_at_offset(object, offset, value, storage.storage_type().clone());
                Ok(())
            }
        }
    }
}
