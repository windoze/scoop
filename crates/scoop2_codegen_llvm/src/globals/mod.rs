//! 全局层：type descriptor / vtable / itable / string literal / closure 全局。
//!
//! 当前实现覆盖：
//! - **string literal**：immortal `ScoopString` 全局（off-heap，`SCOOP_GC_FLAG_IMMORTAL`，
//!   header 指向运行时 extern `__scoop_type_desc_runtime__ScoopString`）。
//!
//! 后续（W1-2）补充：type descriptor 全局 + trace bitmap、vtable/itable 全局、closure 布局。

use inkwell::module::Linkage;
use inkwell::values::PointerValue;
use inkwell::AddressSpace;

use sha2::{Digest, Sha256};

use crate::context::{native_address_space, CodegenContext, GC_ADDRSPACE};
use crate::error::CodegenResult;

/// `SCOOP_GC_FLAG_IMMORTAL`
const GC_FLAG_IMMORTAL: u64 = 0x8000_0000;
/// `SCOOP_GC_MARK_IMMORTAL`
const GC_MARK_IMMORTAL: u64 = 0xFFFF_FFFF;

impl<'ctx> CodegenContext<'ctx> {
    /// 取得（或创建）一个 immortal string literal 全局，返回指向 `ScoopString` 的 GC 指针。
    ///
    /// `ScoopString` 布局：`{ ScoopObjectHeader header; i64 byte_len; ptr data }`。
    /// data 是字节内容（常量字节数组全局）。
    pub fn get_or_create_string_literal(&self, s: &str) -> CodegenResult<PointerValue<'ctx>> {
        let key = string_literal_key(s);
        if let Some(cached) = self.lookup_string_literal(&key) {
            return Ok(cached);
        }

        let ctx = self.context;
        // 1. 字节数据全局：`@[N x i8]` 常量。
        let bytes = s.as_bytes();
        let i8_ty = ctx.i8_type();
        let arr_ty = i8_ty.array_type(bytes.len().max(1) as u32);
        let data_global = self
            .module
            .add_global(arr_ty, Some(AddressSpace::from(0u16)), &format!("__scoop_str_data_{key}"));
        data_global.set_linkage(Linkage::Internal);
        data_global.set_constant(true);
        let arr_const = if bytes.is_empty() {
            arr_ty.const_zero()
        } else {
            let vals: Vec<_> = bytes.iter().map(|b| i8_ty.const_int(*b as u64, false)).collect();
            i8_ty.const_array(&vals)
        };
        data_global.set_initializer(&arr_const);

        // 2. ScoopString header 类型：{ ptr next; ptr type_desc; i64 size; i32 flags; i32 mark; i64 len; ptr data }
        let native_ptr = ctx.ptr_type(native_address_space());
        let header_ty = ctx.struct_type(
            &[
                native_ptr.into(), // next
                native_ptr.into(), // type_desc
                ctx.i64_type().into(), // size_bytes
                ctx.i32_type().into(), // flags
                ctx.i32_type().into(), // mark
                ctx.i64_type().into(), // byte_len
                native_ptr.into(), // data ptr (native)
            ],
            false,
        );

        // 3. type_desc：运行时 extern `__scoop_type_desc_runtime__ScoopString`。
        let type_desc = self.get_or_declare_string_type_desc();

        // 4. ScoopString 全局（GC addrspace 1，immortal）。
        let str_global_name = format!("__scoop_str_{key}");
        let str_global = self.module.add_global(
            header_ty,
            Some(AddressSpace::from(GC_ADDRSPACE as u16)),
            &str_global_name,
        );
        str_global.set_linkage(Linkage::Internal);
        str_global.set_constant(true);
        let init = header_ty.const_named_struct(&[
            native_ptr.const_zero().into(), // next = null
            type_desc.into(),               // type_desc
            ctx.i64_type().const_int(0, false).into(), // size_bytes (运行时忽略 for immortal)
            ctx.i32_type().const_int(GC_FLAG_IMMORTAL as u64, false).into(), // flags
            ctx.i32_type().const_int(GC_MARK_IMMORTAL, false).into(), // mark
            ctx.i64_type().const_int(bytes.len() as u64, false).into(), // byte_len
            data_global.as_pointer_value().into(), // data
        ]);
        str_global.set_initializer(&init);

        let ptr = str_global.as_pointer_value();
        self.cache_string_literal(key.clone(), ptr);
        Ok(ptr)
    }

    /// 声明（或取得）运行时 extern `__scoop_type_desc_runtime__ScoopString`（native 指针）。
    pub fn get_or_declare_string_type_desc(&self) -> PointerValue<'ctx> {
        if let Some(gv) = self.module.get_global(crate::runtime_abi::sym::STRING_TYPE_DESC) {
            return gv.as_pointer_value();
        }
        let gv = self.module.add_global(
            self.context.ptr_type(native_address_space()),
            Some(AddressSpace::from(0u16)),
            crate::runtime_abi::sym::STRING_TYPE_DESC,
        );
        gv.set_linkage(Linkage::External);
        // 不设 initializer（由运行时提供定义）。
        gv.as_pointer_value()
    }
}

/// 计算 string literal 的稳定 key（内容 hash 前 16 hex）。
fn string_literal_key(s: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(s.as_bytes());
    let hash = hasher.finalize();
    let mut hex = String::new();
    for b in hash.iter().take(8) {
        hex.push_str(&format!("{:02x}", b));
    }
    hex
}

/// 占位：W1-2 其余全局声明（type_desc/vtable/itable/closure）。
impl<'ctx> CodegenContext<'ctx> {
    pub fn declare_all_globals(&self) -> CodegenResult<()> {
        Ok(())
    }
}
