//! 全局层：type descriptor / itable / vtable / string literal / closure 全局。

use inkwell::AddressSpace;
use inkwell::module::Linkage;
use inkwell::types::BasicTypeEnum;
use inkwell::values::{AsValueRef, PointerValue};

use sha2::{Digest, Sha256};

use crate::context::{CodegenContext, GC_ADDRSPACE, native_address_space};
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
        let data_global = self.module.add_global(
            arr_ty,
            Some(AddressSpace::from(0u16)),
            &format!("__scoop_str_data_{key}"),
        );
        data_global.set_linkage(Linkage::Internal);
        data_global.set_constant(true);
        let arr_const = if bytes.is_empty() {
            arr_ty.const_zero()
        } else {
            let vals: Vec<_> = bytes
                .iter()
                .map(|b| i8_ty.const_int(*b as u64, false))
                .collect();
            i8_ty.const_array(&vals)
        };
        data_global.set_initializer(&arr_const);

        let native_ptr = ctx.ptr_type(native_address_space());
        let header_ty = ctx.struct_type(
            &[
                native_ptr.into(),     // next
                native_ptr.into(),     // type_desc
                ctx.i64_type().into(), // size_bytes
                ctx.i32_type().into(), // flags
                ctx.i32_type().into(), // mark
                ctx.i64_type().into(), // byte_len
                native_ptr.into(),     // data
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
            ctx.i32_type()
                .const_int(GC_FLAG_IMMORTAL as u64, false)
                .into(),
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
        if let Some(gv) = self
            .module
            .get_global(crate::runtime_abi::sym::STRING_TYPE_DESC)
        {
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
        ctx.struct_type(
            &[ctx.i32_type().into(), ctx.i32_type().into(), ptr.into()],
            false,
        )
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

    /// 为一个 class 生成 type descriptor 全局（含 itable 引用 + 正确的 GC 元数据）。
    /// 返回 type descriptor 的 native 指针。
    ///
    /// size_bytes / trace_start_offset_bytes / trace_bitmap 从 class 字段布局
    /// （class_inits + type_layouts）计算，使 GC 能正确扫描引用字段。
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

        // 计算 GC trace 元数据：对象布局 = { header; field0; field1; ... }。
        // 字段按 ptr-sized slot 打包（与 class_ctor codegen 对齐）。
        // trace_bitmap 每位对应一个 word slot；该 slot 为 GC 引用则置 1。
        let header_ty = self.object_header_type();
        let header_size = self.target_data.get_store_size(&header_ty);
        let ptr_size = self.pointer_byte_size;

        // 从 class_inits 查找字段列表。
        let class_init = self.class_inits.iter().find(|ci| ci.class_fqn == class_fqn);
        let (size_bytes, trace_start, trace_bitmap_words) = if let Some(ci) = class_init {
            // 字段数（含超类字段——超类字段在 class_init.field_inits 中是否包含取决于 LIR；
            // 当前 LIR 只列本类声明的属性初始化，超类字段由 super_init 单独处理）。
            let field_count = ci.field_inits.len();
            let total_size = header_size + (field_count as u64) * ptr_size;
            // 构造 bitmap：第 i 个 slot = field_inits[i].ty 是否 GC-traceable。
            let mut bitmap_words: Vec<u64> = Vec::new();
            for i in 0..field_count {
                let word_idx = i / 64;
                while bitmap_words.len() <= word_idx {
                    bitmap_words.push(0);
                }
                let field_ty = ci.field_inits[i].ty;
                if scoop2_lir::gc::is_gc_traceable_type(field_ty, &self.type_layouts) {
                    bitmap_words[word_idx] |= 1u64 << (i % 64);
                }
            }
            (total_size, header_size, bitmap_words)
        } else {
            // 无 class_init（如 String 等 runtime 内建类型，或无字段的 class）：
            // size = header_size（无 payload），无 trace bitmap。
            (header_size, header_size, Vec::new())
        };

        // trace_bitmap 全局数组（仅当有非零位时创建；否则留 null）。
        let (trace_bitmap_ptr, trace_bitmap_len) = if trace_bitmap_words.is_empty() {
            (native_ptr.const_null(), 0u32)
        } else {
            let bm_len = trace_bitmap_words.len() as u32;
            let bm_arr_ty = ctx.i64_type().array_type(bm_len);
            let bm_name = format!("__scoop_trace_bitmap_{class_fqn}").replace('.', "_");
            let bm_global =
                self.module
                    .add_global(bm_arr_ty, Some(AddressSpace::from(0u16)), &bm_name);
            bm_global.set_linkage(Linkage::Internal);
            bm_global.set_constant(true);
            let bm_vals: Vec<_> = trace_bitmap_words
                .iter()
                .map(|&w| ctx.i64_type().const_int(w, false))
                .collect();
            let bm_init = ctx.i64_type().const_array(&bm_vals);
            bm_global.set_initializer(&bm_init);
            (bm_global.as_pointer_value(), bm_len)
        };

        // class vtable（虚方法分发；无虚方法的 class 为 null）。
        let vtable_ptr = self.build_class_vtable(class_fqn);

        let init = td_ty.const_named_struct(&[
            ctx.i32_type().const_int(0, false).into(), // abi_version
            ctx.i32_type().const_int(0, false).into(), // flags
            ctx.i64_type().const_int(size_bytes, false).into(), // size_bytes
            ctx.i64_type().const_int(ptr_size, false).into(), // align_bytes
            ctx.i64_type().const_int(trace_start, false).into(), // trace_start_offset_bytes
            ctx.i32_type()
                .const_int(trace_bitmap_len as u64, false)
                .into(), // trace_bitmap_u64_len
            ctx.i32_type().const_int(0, false).into(), // _reserved_u32
            trace_bitmap_ptr.into(),                   // trace_bitmap
            native_ptr.const_null().into(),            // trace_fn
            native_ptr.const_null().into(),            // release_fn
            ctx.i64_type().const_int(type_id, false).into(), // type_id
            native_ptr.const_null().into(),            // parent_type_desc
            itable_ptr.into(),                         // itable
            vtable_ptr.into(),                         // vtable
        ]);
        gv.set_initializer(&init);
        gv.as_pointer_value()
    }

    /// 闭包对象的 type descriptor：布局 `{ header; env_ptr; invoke_fn_ptr }`。
    /// env_ptr 是 GC 引用（指向 env blob），fn_ptr 是代码地址——trace bitmap = 0b01，
    /// 使 GC 能经闭包对象 trace 到 env blob（env blob 的 descriptor 见
    /// [`CodegenContext::get_or_create_env_blob_type_descriptor`]）。
    pub fn get_or_create_closure_type_descriptor(&self) -> PointerValue<'ctx> {
        let header_size = self.target_data.get_store_size(&self.object_header_type());
        let ptr_size = self.pointer_byte_size;
        self.emit_simple_type_descriptor(
            "__scoop_type_desc_scoop_core_Closure",
            stable_hash_u64("scoop.core.Closure"),
            header_size + 2 * ptr_size,
            header_size,
            vec![0b01],
        )
    }

    /// 闭包 env blob 的 type descriptor（按 env 类型一物一 descriptor）。
    ///
    /// blob 布局 = object header 之后按 env struct/tuple 布局存放字段值。
    /// trace bitmap 通过递归枚举 env 布局中的 GC 指针叶子（Reference/Function
    /// 字段、Option niche 指针、嵌套 struct/tuple 内的指针字段）得到——
    /// 只标记确定为指针对齐 word 的槽位，非指针 word 不会被误 trace。
    pub fn get_or_create_env_blob_type_descriptor(
        &self,
        env_ty: scoop2_hir::ty::TypeId,
        payload_size: u64,
    ) -> PointerValue<'ctx> {
        let header_size = self.target_data.get_store_size(&self.object_header_type());
        let mut word_offsets: Vec<u64> = Vec::new();
        collect_gc_word_offsets(&self.type_layouts, env_ty, 0, &mut word_offsets);
        let mut bitmap_words: Vec<u64> = Vec::new();
        for w in word_offsets {
            let word_idx = (w / 64) as usize;
            while bitmap_words.len() <= word_idx {
                bitmap_words.push(0);
            }
            bitmap_words[word_idx] |= 1u64 << (w % 64);
        }
        // bitmap 全零时传空（trace_bitmap = null，表示无引用字段）。
        if bitmap_words.iter().all(|&w| w == 0) {
            bitmap_words.clear();
        }
        let name = format!("__scoop_type_desc_closure_env_{}", env_ty.0);
        self.emit_simple_type_descriptor(
            &name,
            stable_hash_u64(&name),
            header_size + payload_size,
            header_size,
            bitmap_words,
        )
    }

    /// EffectStep frame 对象的 type descriptor（按函数一物一 descriptor）。
    ///
    /// frame 布局 = object header 之后按 frame tuple 布局存放 state/参数槽/live 槽。
    /// trace bitmap 通过 `collect_gc_word_offsets` 枚举 tuple 布局中的 GC 指针
    /// 叶子得到（与 env blob descriptor 同一机制）。
    pub fn get_or_create_frame_type_descriptor(
        &self,
        fn_symbol: &str,
        frame_ty: scoop2_hir::ty::TypeId,
        payload_size: u64,
    ) -> PointerValue<'ctx> {
        let header_size = self.target_data.get_store_size(&self.object_header_type());
        let mut word_offsets: Vec<u64> = Vec::new();
        collect_gc_word_offsets(&self.type_layouts, frame_ty, 0, &mut word_offsets);
        let mut bitmap_words: Vec<u64> = Vec::new();
        for w in word_offsets {
            let word_idx = (w / 64) as usize;
            while bitmap_words.len() <= word_idx {
                bitmap_words.push(0);
            }
            bitmap_words[word_idx] |= 1u64 << (w % 64);
        }
        if bitmap_words.iter().all(|&w| w == 0) {
            bitmap_words.clear();
        }
        let name = format!("__scoop_type_desc_frame_{}", fn_symbol.replace('.', "_"));
        self.emit_simple_type_descriptor(
            &name,
            stable_hash_u64(&name),
            header_size + payload_size,
            header_size,
            bitmap_words,
        )
    }

    /// canonical continuation 对象的 type descriptor（全程序共享一个）。
    ///
    /// 布局见 `scoop2_lir::effect`（header 32B | resumed@32 | state@40 |
    /// frame@48 | step_fn@56 | resume_value@64，共 72B）。trace bitmap = 0b100：
    /// 只 trace frame 指针（word 2）；resume_value 写入后立即调 step_fn、
    /// 中间无 safepoint，不需要 trace。
    pub fn get_or_create_continuation_type_descriptor(&self) -> PointerValue<'ctx> {
        let header_size = self.target_data.get_store_size(&self.object_header_type());
        self.emit_simple_type_descriptor(
            "__scoop_type_desc_continuation",
            stable_hash_u64("__scoop_type_desc_continuation"),
            scoop2_lir::effect::CONT_SIZE_BYTES,
            header_size,
            vec![0b100],
        )
    }

    /// resume 复合值 box 的 type descriptor（按值类型一物一 descriptor）。
    ///
    /// 复合（struct/tuple 等按值）resume payload 经 GC box 传递：box 布局 =
    /// object header 之后按值类型布局存放载荷。trace bitmap 由
    /// `collect_gc_word_offsets` 枚举 GC 指针叶子得到（与 env blob 同一机制）。
    /// box 只在 resume word 写入 → step_fn 投递的窗口内被 continuation 的
    /// resume_value 字段引用，该窗口无 safepoint（与 continuation descriptor
    /// 不 trace resume_value 同一假设），bitmap 为正确性兜底。
    pub fn get_or_create_resume_box_type_descriptor(
        &self,
        value_ty: scoop2_hir::ty::TypeId,
        payload_size: u64,
    ) -> PointerValue<'ctx> {
        let header_size = self.target_data.get_store_size(&self.object_header_type());
        let mut word_offsets: Vec<u64> = Vec::new();
        collect_gc_word_offsets(&self.type_layouts, value_ty, 0, &mut word_offsets);
        let mut bitmap_words: Vec<u64> = Vec::new();
        for w in word_offsets {
            let word_idx = (w / 64) as usize;
            while bitmap_words.len() <= word_idx {
                bitmap_words.push(0);
            }
            bitmap_words[word_idx] |= 1u64 << (w % 64);
        }
        // bitmap 全零时传空（trace_bitmap = null，表示无引用字段）。
        if bitmap_words.iter().all(|&w| w == 0) {
            bitmap_words.clear();
        }
        let name = format!("__scoop_type_desc_resume_box_{}", value_ty.0);
        self.emit_simple_type_descriptor(
            &name,
            stable_hash_u64(&name),
            header_size + payload_size,
            header_size,
            bitmap_words,
        )
    }

    /// chain link 对象的 type descriptor（全程序共享一个）。
    ///
    /// 布局见 `scoop2_lir::effect`（header 32B | frame@32 | step_fn@40，共
    /// 48B）。trace bitmap = 0b01：只 trace frame 指针（word 0，GC 堆对象
    /// 引用）；step_fn 是代码地址，不需要 trace。
    pub fn get_or_create_chain_link_type_descriptor(&self) -> PointerValue<'ctx> {
        let header_size = self.target_data.get_store_size(&self.object_header_type());
        self.emit_simple_type_descriptor(
            "__scoop_type_desc_chain_link",
            stable_hash_u64("__scoop_type_desc_chain_link"),
            scoop2_lir::effect::LINK_SIZE_BYTES,
            header_size,
            vec![0b01],
        )
    }

    /// 构建一个无 itable/vtable/parent 的 type descriptor 全局（内部链接）。
    /// `bitmap_words` 为空时 trace_bitmap = null（无引用字段）。
    fn emit_simple_type_descriptor(
        &self,
        global_name: &str,
        type_id: u64,
        size_bytes: u64,
        trace_start: u64,
        bitmap_words: Vec<u64>,
    ) -> PointerValue<'ctx> {
        if let Some(gv) = self.module.get_global(global_name) {
            return gv.as_pointer_value();
        }
        let ctx = self.context;
        let native_ptr = ctx.ptr_type(native_address_space());
        let td_ty = self.type_descriptor_type();
        let gv = self
            .module
            .add_global(td_ty, Some(AddressSpace::from(0u16)), global_name);
        gv.set_linkage(Linkage::Internal);

        let (trace_bitmap_ptr, trace_bitmap_len) = if bitmap_words.is_empty() {
            (native_ptr.const_null(), 0u32)
        } else {
            let bm_len = bitmap_words.len() as u32;
            let bm_arr_ty = ctx.i64_type().array_type(bm_len);
            let bm_name = format!("{global_name}_trace_bitmap");
            let bm_global =
                self.module
                    .add_global(bm_arr_ty, Some(AddressSpace::from(0u16)), &bm_name);
            bm_global.set_linkage(Linkage::Internal);
            bm_global.set_constant(true);
            let bm_vals: Vec<_> = bitmap_words
                .iter()
                .map(|&w| ctx.i64_type().const_int(w, false))
                .collect();
            bm_global.set_initializer(&ctx.i64_type().const_array(&bm_vals));
            (bm_global.as_pointer_value(), bm_len)
        };

        let init = td_ty.const_named_struct(&[
            ctx.i32_type().const_int(0, false).into(), // abi_version
            ctx.i32_type().const_int(0, false).into(), // flags
            ctx.i64_type().const_int(size_bytes, false).into(), // size_bytes
            ctx.i64_type()
                .const_int(self.pointer_byte_size, false)
                .into(), // align_bytes
            ctx.i64_type().const_int(trace_start, false).into(), // trace_start_offset_bytes
            ctx.i32_type()
                .const_int(trace_bitmap_len as u64, false)
                .into(), // trace_bitmap_u64_len
            ctx.i32_type().const_int(0, false).into(), // _reserved_u32
            trace_bitmap_ptr.into(),                   // trace_bitmap
            native_ptr.const_null().into(),            // trace_fn
            native_ptr.const_null().into(),            // release_fn
            ctx.i64_type().const_int(type_id, false).into(), // type_id
            native_ptr.const_null().into(),            // parent_type_desc
            native_ptr.const_null().into(),            // itable
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
        let entries: Vec<(u64, Vec<PointerValue>)> = self.program_itables_for_class(class_fqn);

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
            let methods_gv = self.module.add_global(
                methods_arr_ty,
                Some(AddressSpace::from(0u16)),
                &methods_name,
            );
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
        let entries_gv =
            self.module
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

    /// 为一个 class 构建 vtable 全局（`[N x ptr]`，按 slot_index 排列的虚方法函数指针）。
    /// 无 vtable 布局（非 open class / 无虚方法）→ null。
    fn build_class_vtable(&self, class_fqn: &str) -> PointerValue<'ctx> {
        let native_ptr = self.context.ptr_type(native_address_space());
        let vtables = self.vtables_data.borrow();
        let Some(vt) = vtables.iter().find(|vt| vt.class_fqn == class_fqn) else {
            return native_ptr.const_null();
        };
        // 收集所有已定义的函数符号（用于前缀匹配，同 itable 解析逻辑）。
        let all_fn_names: Vec<String> = self
            .module
            .get_functions()
            .map(|fv| fv.get_name().to_string_lossy().into_owned())
            .collect();
        let resolve = |sym: &str| -> PointerValue<'ctx> {
            // 先精确匹配。
            if let Some(fv) = self
                .lookup_callable_fn(sym)
                .or_else(|| self.module.get_function(sym))
            {
                return unsafe { PointerValue::new(fv.as_value_ref()) };
            }
            // vtable 符号 = mangle_target_symbol 输出（无 hash 后缀）；
            // 实际函数符号可能带 `_<hash>` 后缀 → 前缀匹配。
            for name in &all_fn_names {
                if name.starts_with(sym) && name.len() > sym.len() {
                    if let Some(fv) = self.module.get_function(name) {
                        return unsafe { PointerValue::new(fv.as_value_ref()) };
                    }
                }
            }
            native_ptr.const_null()
        };
        let slot_count = vt
            .slots
            .iter()
            .map(|s| s.slot_index)
            .max()
            .map(|m| m + 1)
            .unwrap_or(0);
        if slot_count == 0 {
            return native_ptr.const_null();
        }
        let mut vals: Vec<PointerValue<'ctx>> = vec![native_ptr.const_null(); slot_count as usize];
        for slot in &vt.slots {
            vals[slot.slot_index as usize] = resolve(&slot.target_symbol);
        }
        let arr_ty = native_ptr.array_type(slot_count);
        let name = format!("__scoop_vtable_{}", class_fqn.replace('.', "_"));
        let gv = self
            .module
            .add_global(arr_ty, Some(AddressSpace::from(0u16)), &name);
        gv.set_linkage(Linkage::Internal);
        gv.set_constant(true);
        let init = native_ptr.const_array(&vals.iter().map(|p| (*p).into()).collect::<Vec<_>>());
        gv.set_initializer(&init);
        gv.as_pointer_value()
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
                        if let Some(fv) = self
                            .lookup_callable_fn(s)
                            .or_else(|| self.module.get_function(s))
                        {
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
/// FNV-1a 64 位哈希（与 type_desc.type_id 同公式）。
/// 公开供 TypeTest/Cast 计算 target type_id。
pub fn stable_hash_u64_pub(s: &str) -> u64 {
    stable_hash_u64(s)
}

fn stable_hash_u64(s: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in s.as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

/// 递归枚举某值类型布局中所有 GC 指针对齐 word 的偏移（单位：word，相对 base）。
///
/// 用于闭包 env blob 的 trace bitmap：
/// - Reference(gc_traceable) / Function：整个值就是一个指针 word；
/// - Struct / Tuple：按字段偏移递归（只收集指针对齐 word，非指针对齐的引用
///   不可能出现——指针字段恒 8 字节对齐）；
/// - Option：Pointer niche 整体即指针；Tagged 的 payload 若含指针按 payload
///   偏移递归；U8 niche 纯标量；
/// - 其他（标量 / Enum / Nothing）：无指针叶子。
fn collect_gc_word_offsets(
    layouts: &scoop2_lir::TypeLayoutTable,
    ty: scoop2_hir::ty::TypeId,
    base_bytes: u64,
    out: &mut Vec<u64>,
) {
    use scoop2_lir::{NicheStorage, TypeLayoutKind};
    let Some(layout) = layouts.get(ty) else {
        return;
    };
    let word = 8u64;
    match &layout.kind {
        TypeLayoutKind::Reference {
            gc_traceable: true, ..
        }
        | TypeLayoutKind::Function => {
            if base_bytes % word == 0 {
                out.push(base_bytes / word);
            }
        }
        TypeLayoutKind::Struct { fields } | TypeLayoutKind::Tuple { elements: fields } => {
            for f in fields {
                collect_gc_word_offsets(layouts, f.ty, base_bytes + f.offset, out);
            }
        }
        TypeLayoutKind::Option {
            storage,
            payload_ty,
            ..
        } => match storage {
            NicheStorage::Pointer => {
                if base_bytes % word == 0 {
                    out.push(base_bytes / word);
                }
            }
            NicheStorage::Tagged => {
                // Tagged 布局 `{ i8 tag; payload }`：payload 偏移 = align_up(1, payload_align)。
                let payload_align = layouts.get(*payload_ty).map(|l| l.align).unwrap_or(1);
                let payload_off = if payload_align <= 1 { 1 } else { payload_align };
                collect_gc_word_offsets(layouts, *payload_ty, base_bytes + payload_off, out);
            }
            NicheStorage::U8 { .. } => {}
        },
        _ => {}
    }
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

    /// 声明顶层 val/var 的全局 backing slot（每个 global_init entry 一个）。
    ///
    /// 全局以「零初始化的 mutable global」形式声明，entry main 在用户 main 之前
    /// 调用各 init_callable 把初值写入。TopLevelRef 读这些 global。
    /// GC 引用类型的全局额外经 `scoop_gc_register_global_root` 注册为 GC root。
    pub fn declare_top_level_globals(
        &self,
        program: &scoop2_lir::LirProgram,
    ) -> CodegenResult<()> {
        for entry in &program.global_init.entries {
            let llvm_ty = self.lower_type(entry.ty, &program.type_layouts)?;
            // 用 [N x i8] 形式声明以避免 LLVM global 必须是常量初始化的限制；
            // 实际存取按 entry.ty 的 LLVM 类型 load/store（bitcast 后）。
            let store_size = self.target_data.get_store_size(&llvm_ty).max(1);
            let arr_ty = self.context.i8_type().array_type(store_size as u32);
            let gv = self
                .module
                .add_global(arr_ty, None, &format!("__scoop_toplevel_{}", sanitize_global_name(&entry.fqn)));
            gv.set_linkage(Linkage::Internal);
            // 零初始化：[store_size x i8] zeroinitializer。
            gv.set_initializer(&arr_ty.const_zero());
            // 缓存为 native ptr（global 的地址）。
            self.cache_global(entry.fqn.clone(), gv.as_pointer_value());
        }
        Ok(())
    }

    /// type descriptor cache helper（供 cache_type_desc / lookup_type_desc）。
    pub fn cache_type_desc_pub(&self, fqn: String, ptr: PointerValue<'ctx>) {
        self.cache_type_desc(fqn, ptr);
    }
}

/// 把 FQN 转成合法的 LLVM 符号后缀（替换 `.` / `<` / `>` 等）。
fn sanitize_global_name(fqn: &str) -> String {
    fqn.replace('.', "_")
        .replace('<', "L")
        .replace('>', "R")
        .replace(',', "_")
        .replace(' ', "_")
}
