//! Cast helpers (cast_int / cast_float), int_type, global-bytes cache, entry alloca primitives.

#![allow(dead_code)]

use super::*;
use sha2::{Digest as _, Sha256};

fn string_byte_data_global_name(bytes: &[u8]) -> String {
    format!("__scoop_str_data_{}", string_byte_data_hash(bytes))
}

fn string_byte_data_global_name_for_hash(hash: &str, collision_index: usize) -> String {
    let base = format!("__scoop_str_data_{hash}");
    if collision_index == 0 {
        base
    } else {
        format!("{base}_{collision_index}")
    }
}

fn string_byte_data_hash(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";

    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(32);
    for byte in &digest[..16] {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

impl StringByteDataGlobalRegistry {
    fn reserve(&mut self, bytes: &[u8]) -> String {
        self.reserve_for_hash(&string_byte_data_hash(bytes), bytes)
    }

    fn reserve_for_hash(&mut self, hash: &str, bytes: &[u8]) -> String {
        if let Some(name) = self.names_by_bytes.get(bytes) {
            return name.clone();
        }

        let next_collision_index = self
            .next_collision_index_by_hash
            .entry(hash.to_string())
            .or_insert(0);
        let name = string_byte_data_global_name_for_hash(hash, *next_collision_index);
        *next_collision_index += 1;
        self.names_by_bytes.insert(bytes.to_vec(), name.clone());
        name
    }
}

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(in crate::llvm::codegen) fn cast_int(
        &mut self,
        value: IntValue<'ctx>,
        from: IntTy,
        to: IntTy,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        if from.bits == to.bits {
            return Ok(value);
        }

        let to_ty = self.int_type(to);
        if to.bits > from.bits {
            if from.signed {
                Ok(self.builder.build_int_s_extend(value, to_ty, "sext")?)
            } else {
                Ok(self.builder.build_int_z_extend(value, to_ty, "zext")?)
            }
        } else {
            Ok(self.builder.build_int_truncate(value, to_ty, "trunc")?)
        }
    }

    pub(in crate::llvm::codegen) fn cast_float(
        &mut self,
        value: FloatValue<'ctx>,
        from: CgTy,
        to: CgTy,
    ) -> Result<FloatValue<'ctx>, LlvmEmitError> {
        match (from, to) {
            (CgTy::Float64, CgTy::Float64) | (CgTy::Float32, CgTy::Float32) => Ok(value),
            (CgTy::Float32, CgTy::Float64) => {
                Ok(self
                    .builder
                    .build_float_ext(value, self.context.f64_type(), "fpext")?)
            }
            (CgTy::Float64, CgTy::Float32) => {
                Ok(self
                    .builder
                    .build_float_trunc(value, self.context.f32_type(), "fptrunc")?)
            }
            _ => unreachable!("cast_float only accepts Float64/Float32"),
        }
    }

    pub(in crate::llvm::codegen) fn int_type(&self, ty: IntTy) -> IntType<'ctx> {
        self.context.custom_width_int_type(ty.bits)
    }

    pub(in crate::llvm::codegen) fn get_or_create_global_bytes(
        &self,
        bytes: &[u8],
    ) -> GlobalValue<'ctx> {
        let name = self
            .shared
            .shared_caches
            .string_byte_data_globals
            .borrow_mut()
            .reserve(bytes);
        if let Some(existing) = self.module.get_global(&name) {
            existing.set_unnamed_addr(true);
            return existing;
        }

        let arr_ty = self.context.i8_type().array_type(bytes.len() as u32);
        let gv = self.module.add_global(arr_ty, None, &name);
        let init = self.context.const_string(bytes, false);
        gv.set_initializer(&init);
        gv.set_constant(true);
        gv.set_unnamed_addr(true);
        gv
    }

    pub(in crate::llvm::codegen) fn create_entry_alloca(
        &mut self,
        at: crate::span::Span,
        name: &str,
        ty: CgTy,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let alloca_ty = self.llvm_basic_type_of(at, ty)?;
        let ptr = self.create_entry_alloca_raw(at, name, alloca_ty)?;
        self.apply_alloca_alignment_for_ty(at, ptr, ty)?;
        Ok(ptr)
    }

    pub(in crate::llvm::codegen) fn create_entry_alloca_raw(
        &mut self,
        at: crate::span::Span,
        name: &str,
        alloca_ty: BasicTypeEnum<'ctx>,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let frame_slots = self.reserve_explicit_frame_leaf_slots_for_storage_type(at, alloca_ty)?;
        let alloca_builder = self.context.create_builder();
        let func = self.expect_current_function("create_entry_alloca_raw");
        let entry = self.expect_entry_block(func, "create_entry_alloca_raw");

        match entry.get_first_instruction() {
            Some(inst) => alloca_builder.position_before(&inst),
            None => alloca_builder.position_at_end(entry),
        }

        let slot = alloca_builder.build_alloca(alloca_ty, name)?;
        self.record_explicit_frame_slot_mirrors(slot, frame_slots);
        Ok(slot)
    }

    pub(in crate::llvm::codegen) fn create_entry_scratch_alloca_raw(
        &self,
        _at: crate::span::Span,
        name: &str,
        alloca_ty: BasicTypeEnum<'ctx>,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let alloca_builder = self.context.create_builder();
        let func = self.expect_current_function("create_entry_scratch_alloca_raw");
        let entry = self.expect_entry_block(func, "create_entry_scratch_alloca_raw");

        match entry.get_first_instruction() {
            Some(inst) => alloca_builder.position_before(&inst),
            None => alloca_builder.position_at_end(entry),
        }

        Ok(alloca_builder.build_alloca(alloca_ty, name)?)
    }

    pub(in crate::llvm::codegen) fn apply_alloca_alignment_for_ty(
        &self,
        _at: crate::span::Span,
        ptr: PointerValue<'ctx>,
        ty: CgTy,
    ) -> Result<(), LlvmEmitError> {
        // `@CLayout(aligned = N)`：显式对齐仅对 struct 有意义，其它类型保持默认 ABI 对齐。
        let CgTy::Struct(struct_ty) = ty else {
            return Ok(());
        };
        let Some(aligned) = self.struct_clayout(struct_ty).and_then(|c| c.aligned) else {
            return Ok(());
        };

        let inst = ptr
            .as_instruction_value()
            .expect("alloca pointer must be an instruction before alignment is applied");
        inst.set_alignment(aligned)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        StringByteDataGlobalRegistry, string_byte_data_global_name, string_byte_data_hash,
    };

    #[test]
    fn string_byte_data_hash_is_sha256_prefix_hex() {
        assert_eq!(
            string_byte_data_hash(b"hello"),
            "2cf24dba5fb0a30e26e83b2ac5b9e29e"
        );
    }

    #[test]
    fn string_byte_data_global_name_is_content_keyed() {
        let same_bytes = "hello".as_bytes().to_vec();
        assert_eq!(
            string_byte_data_global_name(b"hello"),
            string_byte_data_global_name(&same_bytes)
        );
        assert_ne!(
            string_byte_data_global_name(b"hello"),
            string_byte_data_global_name(b"hello!")
        );
    }

    #[test]
    fn string_byte_data_registry_disambiguates_hash_collisions() {
        let mut registry = StringByteDataGlobalRegistry::default();

        let first = registry.reserve_for_hash("same_hash", b"first");
        let second = registry.reserve_for_hash("same_hash", b"second");
        let second_again = registry.reserve_for_hash("same_hash", b"second");

        assert_eq!(first, "__scoop_str_data_same_hash");
        assert_eq!(second, "__scoop_str_data_same_hash_1");
        assert_eq!(second_again, second);
    }
}
