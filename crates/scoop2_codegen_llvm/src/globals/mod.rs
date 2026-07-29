//! 全局层：type descriptor / itable / vtable / string literal / closure 全局。

use inkwell::module::Linkage;
use inkwell::types::BasicTypeEnum;
use inkwell::values::{AsValueRef, PointerValue};
use inkwell::AddressSpace;

use sha2::{Digest, Sha256};

use crate::context::{native_address_space, CodegenContext, GC_ADDRSPACE};
use crate::error::CodegenResult;

/// `SCOOP_GC_FLAG_IMMORTAL`
const GC_FLAG_IMMORTAL: u64 = 0x8000_0000;
/// `SCOOP_GC_MARK_IMMORTAL`
const GC_MARK_IMMORTAL: u64 = 0xFFFF_FFFF;

impl<'ctx> CodegenContext<'ctx> {
    /// 取得（或创建）一个 immortal string literal 全局。
    pub fn get_or_create_string_literal(&self, s: &str) -> CodegenResult<PointerValue<'ctx>> {
        let key = string_literal_key(s);
        if let Some(cached) = self.lookup_string_literal(&key) {
            return Ok(cached);
        }
        let ctx = self.context;
        let i8_ty = ctx.i8_type();
        let bytes = s.as_bytes();
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

        let native_ptr = ctx.ptr_type(native_address_space());
        let header_ty = ctx.struct_type(
            &[
                native_ptr.into(),        // next
                native_ptr.into(),        // type_desc
                ctx.i64_type().into(),    // size_bytes
                ctx.i32_type().into(),    // flags
                ctx.i32_type().into(),    // mark
                ctx.i64_type().into(),    // byte_len
                native_ptr.into(),        // data
            ],
            false,
        );

        // 使用 codegen 生成的 String type descriptor（含 itable），而非运行时提供的（无 itable）。
        let type_desc = self.get_or_create_type_descriptor("scoop.core.String");
        let str_global_name = format!("__scoop_str_{key}");
        let str_global = self.module.add_global(
            header_ty,
            Some(AddressSpace::from(GC_ADDRSPACE as u16)),
            &str_global_name,
        );
        str_global.set_linkage(Linkage::Internal);
        str_global.set_constant(true);
        let init = header_ty.const_named_struct(&[
            native_ptr.const_null().into(),
            type_desc.into(),
            ctx.i64_type().const_int(0, false).into(),
            ctx.i32_type().const_int(GC_FLAG_IMMORTAL as u64, false).into(),
            ctx.i32_type().const_int(GC_MARK_IMMORTAL, false).into(),
            ctx.i64_type().const_int(bytes.len() as u64, false).into(),
            data_global.as_pointer_value().into(),
        ]);
        str_global.set_initializer(&init);
        let ptr = str_global.as_pointer_value();
        self.cache_string_literal(key.clone(), ptr);
        Ok(ptr)
    }

    /// 声明（或取得）运行时 extern `__scoop_type_desc_runtime__ScoopString`。
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
        gv.as_pointer_value()
    }

    /// ScoopTypeDescriptor LLVM 类型（14 字段，与 runtime C struct 对齐）。
    pub fn type_descriptor_type(&self) -> inkwell::types::StructType<'ctx> {
        let ctx = self.context;
        let ptr = ctx.ptr_type(native_address_space());
        ctx.struct_type(
            &[
                ctx.i32_type().into(), // abi_version
                ctx.i32_type().into(), // flags
                ctx.i64_type().into(), // size_bytes
                ctx.i64_type().into(), // align_bytes
                ctx.i64_type().into(), // trace_start_offset_bytes
                ctx.i32_type().into(), // trace_bitmap_u64_len
                ctx.i32_type().into(), // _reserved_u32
                ptr.into(),            // trace_bitmap
                ptr.into(),            // trace_fn
                ptr.into(),            // release_fn
                ctx.i64_type().into(), // type_id
                ptr.into(),            // parent_type_desc
                ptr.into(),            // itable
                ptr.into(),            // vtable
            ],
            false,
        )
    }

    /// itable entry LLVM 类型：`{ u64 interface_id; ptr methods }`。
    /// methods 指向 `[N x ptr]`（N = 该 interface 的 slot 数）。
    fn itable_entry_type(&self) -> inkwell::types::StructType<'ctx> {
        let ctx = self.context;
        let ptr = ctx.ptr_type(native_address_space());
        ctx.struct_type(&[ctx.i64_type().into(), ptr.into()], false)
    }

    /// itable 容器 LLVM 类型：`{ i32 count; [count x itable_entry] }`。
    /// 但 LLVM struct 的 array 大小需固定。用 `{ i32 count; i32 _pad; ptr entries }` 让 entries 指向数组。
    fn itable_container_type(&self) -> inkwell::types::StructType<'ctx> {
        let ctx = self.context;
        let ptr = ctx.ptr_type(native_address_space());
        ctx.struct_type(&[ctx.i32_type().into(), ctx.i32_type().into(), ptr.into()], false)
    }

    /// itable entry 类型（public，供 call.rs 使用）。
    pub fn itable_entry_type_pub(&self) -> inkwell::types::StructType<'ctx> {
        self.itable_entry_type()
    }

    /// itable 容器类型（public，供 call.rs 使用）。
    pub fn itable_container_type_pub(&self) -> inkwell::types::StructType<'ctx> {
        self.itable_container_type()
    }

    /// ScoopObjectHeader LLVM 类型：`{ ptr next; ptr type_desc; i64 size; i32 flags; i32 mark }`。
    pub fn object_header_type(&self) -> inkwell::types::StructType<'ctx> {
        let ctx = self.context;
        let ptr = ctx.ptr_type(native_address_space());
        ctx.struct_type(
            &[
                ptr.into(),
                ptr.into(),
                ctx.i64_type().into(),
                ctx.i32_type().into(),
                ctx.i32_type().into(),
            ],
            false,
        )
    }

    /// 为一个 class 生成 type descriptor 全局（含 itable 引用）。
    /// 返回 type descriptor 的 native 指针。
    pub fn get_or_create_type_descriptor(&self, class_fqn: &str) -> PointerValue<'ctx> {
        let name = format!("__scoop_type_desc_{class_fqn}");
        let name = name.replace('.', "_");
        if let Some(gv) = self.module.get_global(&name) {
            return gv.as_pointer_value();
        }
        // 先创建占位（opaque global），防止递归。
        let td_ty = self.type_descriptor_type();
        let gv = self
            .module
            .add_global(td_ty, Some(AddressSpace::from(0u16)), &name);
        gv.set_linkage(Linkage::Internal);

        // 构建 itable 容器（此 class 实现的所有 interface）。
        let itable_ptr = self.build_class_itable(class_fqn);

        // type_id：用 FQN hash 的低 64 位。
        let type_id = stable_hash_u64(class_fqn);

        let ctx = self.context;
        let native_ptr = ctx.ptr_type(native_address_space());
        let init = td_ty.const_named_struct(&[
            ctx.i32_type().const_int(0, false).into(), // abi_version
            ctx.i32_type().const_int(0, false).into(), // flags
            ctx.i64_type().const_int(0, false).into(), // size_bytes（后续完善）
            ctx.i64_type().const_int(8, false).into(), // align_bytes
            ctx.i64_type().const_int(0, false).into(), // trace_start_offset_bytes
            ctx.i32_type().const_int(0, false).into(), // trace_bitmap_u64_len
            ctx.i32_type().const_int(0, false).into(), // _reserved_u32
            native_ptr.const_null().into(),            // trace_bitmap
            native_ptr.const_null().into(),            // trace_fn
            native_ptr.const_null().into(),            // release_fn
            ctx.i64_type().const_int(type_id, false).into(), // type_id
            native_ptr.const_null().into(),            // parent_type_desc
            itable_ptr.into(),                         // itable
            native_ptr.const_null().into(),            // vtable
        ]);
        gv.set_initializer(&init);
        gv.as_pointer_value()
    }

    /// 为一个 class 构建 itable 容器全局。
    /// 从 LirProgram.class_itables 收集该 class 实现的所有 interface，
    /// 每个 interface 一个 entry `{ interface_id; methods[] }`。
    fn build_class_itable(&self, class_fqn: &str) -> PointerValue<'ctx> {
        let ctx = self.context;
        let native_ptr = ctx.ptr_type(native_address_space());

        // 收集该 class 的 class_itables 条目。
        let entries: Vec<(u64, Vec<PointerValue>)> = self
            .program_itables_for_class(class_fqn);

        if entries.is_empty() {
            // 无 interface 实现：返回 null itable。
            return native_ptr.const_null();
        }

        // 每个 entry：`{ u64 interface_id; ptr methods }`。methods 指向 `[N x ptr]`。
        let entry_ty = self.itable_entry_type();
        let entry_arr_ty = entry_ty.array_type(entries.len() as u32);

        // 构建 methods 数组全局（每个 interface 一个）+ entry 常量。
        let mut entry_vals: Vec<inkwell::values::BasicValueEnum> = Vec::new();
        for (iface_id, method_ptrs) in &entries {
            // methods 数组全局。
            let methods_arr_ty = native_ptr.array_type(method_ptrs.len().max(1) as u32);
            let methods_name = format!(
                "__scoop_itable_methods_{}_{}",
                class_fqn.replace('.', "_"),
                iface_id
            );
            let methods_gv = self
                .module
                .add_global(methods_arr_ty, Some(AddressSpace::from(0u16)), &methods_name);
            methods_gv.set_linkage(Linkage::Internal);
            methods_gv.set_constant(true);
            let methods_init = if method_ptrs.is_empty() {
                methods_arr_ty.const_zero()
            } else {
                let vals: Vec<_> = method_ptrs.iter().map(|p| (*p).into()).collect();
                native_ptr.const_array(&vals)
            };
            methods_gv.set_initializer(&methods_init);

            let entry = entry_ty.const_named_struct(&[
                ctx.i64_type().const_int(*iface_id, false).into(),
                methods_gv.as_pointer_value().into(),
            ]);
            entry_vals.push(entry.into());
        }

        // entries 数组全局。
        let entries_name = format!("__scoop_itable_entries_{}", class_fqn.replace('.', "_"));
        let entries_gv = self
            .module
            .add_global(entry_arr_ty, Some(AddressSpace::from(0u16)), &entries_name);
        entries_gv.set_linkage(Linkage::Internal);
        entries_gv.set_constant(true);
        let entries_init = entry_ty.const_array(
            &entry_vals
                .iter()
                .map(|v| v.into_struct_value())
                .collect::<Vec<_>>(),
        );
        entries_gv.set_initializer(&entries_init);

        // itable 容器：`{ i32 count; i32 _pad; ptr entries }`。
        let container_ty = self.itable_container_type();
        let container_name = format!("__scoop_itable_{}", class_fqn.replace('.', "_"));
        let container_gv = self.module.add_global(
            container_ty,
            Some(AddressSpace::from(0u16)),
            &container_name,
        );
        container_gv.set_linkage(Linkage::Internal);
        container_gv.set_constant(true);
        let container_init = container_ty.const_named_struct(&[
            ctx.i32_type().const_int(entries.len() as u64, false).into(),
            ctx.i32_type().const_int(0, false).into(),
            entries_gv.as_pointer_value().into(),
        ]);
        container_gv.set_initializer(&container_init);
        container_gv.as_pointer_value()
    }

    /// 从 class_itables_data 获取某 class 实现的所有 interface → 方法指针列表。
    fn program_itables_for_class(&self, class_fqn: &str) -> Vec<(u64, Vec<PointerValue<'ctx>>)> {
        let itables = self.class_itables_data.borrow();
        let native_ptr = self.context.ptr_type(native_address_space());
        // 收集所有已定义的函数符号（用于前缀匹配）。
        let all_fn_names: Vec<String> = self
            .module
            .get_functions()
            .map(|fv| fv.get_name().to_string_lossy().into_owned())
            .collect();
        let mut result = Vec::new();
        for ci in itables.iter().filter(|ci| ci.class_fqn == class_fqn) {
            let method_ptrs: Vec<PointerValue<'ctx>> = ci
                .method_impls
                .iter()
                .map(|sym| match sym {
                    Some(s) => {
                        // 先精确匹配。
                        if let Some(fv) = self.lookup_callable_fn(s).or_else(|| self.module.get_function(s)) {
                            return unsafe { PointerValue::new(fv.as_value_ref()) };
                        }
                        // itable 符号 = mangle_target_symbol 的输出，如 "scoop_core_String_toString"。
                        // 实际函数符号 = mangle_symbol 的输出，如 "scoop_core_String_toString_<hash>"。
                        // 由于 owner-qualified FQN 后，itable 符号已是正确前缀。
                        // 直接用 itable 符号做前缀匹配（后跟 _<hash>）。
                        for name in &all_fn_names {
                            if name.starts_with(s) && name.len() > s.len() {
                                if let Some(fv) = self.module.get_function(name) {
                                    return unsafe { PointerValue::new(fv.as_value_ref()) };
                                }
                            }
                        }
                        native_ptr.const_null()
                    }
                    None => native_ptr.const_null(),
                })
                .collect();
            let method_ptrs = if method_ptrs.is_empty() {
                vec![native_ptr.const_null()]
            } else {
                method_ptrs
            };
            result.push((ci.interface_id, method_ptrs));
        }
        result
    }
}

/// 计算 string literal 的稳定 key。
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

/// 用 FNV-1a 计算 64 位 hash（用于 type_id / interface_id）。
fn stable_hash_u64(s: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in s.as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// 为所有 class 生成 type descriptor 全局。
impl<'ctx> CodegenContext<'ctx> {
    pub fn declare_all_globals(&self) -> CodegenResult<()> {
        // 从 class_itables_data 收集所有 class FQN，为每个生成 type descriptor。
        let class_fqns: Vec<String> = {
            let data = self.class_itables_data.borrow();
            data.iter().map(|ci| ci.class_fqn.clone()).collect()
        };
        for fqn in &class_fqns {
            self.get_or_create_type_descriptor(fqn);
        }
        Ok(())
    }

    /// type descriptor cache helper（供 cache_type_desc / lookup_type_desc）。
    pub fn cache_type_desc_pub(&self, fqn: String, ptr: PointerValue<'ctx>) {
        self.cache_type_desc(fqn, ptr);
    }
}
