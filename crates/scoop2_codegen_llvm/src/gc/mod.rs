//! explicit root frame GC 方案（NEW-LLVM-CODEGEN.md §3.2）。
//!
//! 每个含 GC 操作的函数在入口：
//! 1. `alloca { ScoopRootFrameHeader; [slot_count x ptr] }`（header = { ptr prev; ptr desc }）；
//! 2. push：`header.prev = __scoop_explicit_root_frame_top`；`header.desc = <desc 全局>`；
//!    清零所有 slot；`__scoop_explicit_root_frame_top = %frame`；
//! 3. 每个 ret/unreachable 前 pop：清零 slot；`__scoop_explicit_root_frame_top = %frame.prev`。
//!
//! TLS 全局 `__scoop_explicit_root_frame_top`（thread_local）。
//! 每函数 desc 全局 `ScoopRootFrameDesc { u32 slot_count; ptr slot_offsets }` + offsets `i32[]`。
//!
//! 含 GC 指针的 local 的 slot 镜像：store 时同步写 frame slot；use 时优先从 frame slot load。

use inkwell::AddressSpace;
use inkwell::module::Linkage;
use inkwell::values::PointerValue;

use scoop2_hir::ty::TypeId;
use scoop2_lir::{LirBody, TypeLayoutKind, TypeLayoutTable};

use crate::context::{CodegenContext, native_address_space};
use crate::error::CodegenResult;

/// `ScoopRootFrameHeader` 大小（字节）：{ ptr prev(8); ptr desc(8) }。
pub const HEADER_SIZE_BYTES: u64 = 16;

impl<'ctx> CodegenContext<'ctx> {
    /// 声明/获取 TLS 全局 `__scoop_explicit_root_frame_top`（thread_local，native ptr）。
    pub fn explicit_root_frame_top_global(&self) -> PointerValue<'ctx> {
        let name = crate::runtime_abi::sym::EXPLICIT_ROOT_FRAME_TOP;
        if let Some(gv) = self.module.get_global(name) {
            return gv.as_pointer_value();
        }
        let gv = self.module.add_global(
            self.context.ptr_type(native_address_space()),
            Some(AddressSpace::from(0u16)),
            name,
        );
        gv.set_linkage(Linkage::External);
        // thread_local 标记。
        gv.set_thread_local_mode(Some(inkwell::ThreadLocalMode::GeneralDynamicTLSModel));
        gv.as_pointer_value()
    }

    /// 声明/获取 TLS 全局 `__scoop_effect_chain`（thread_local，native ptr）。
    ///
    /// 与 `__scoop_explicit_root_frame_top` 不同：本符号只在编译产物的单个
    /// LLVM 模块内使用（C runtime 不感知），因此直接在模块内发定义
    /// （Internal 链接 + null 初始化），不依赖 libscooprt。
    pub fn effect_chain_global(&self) -> PointerValue<'ctx> {
        let name = crate::runtime_abi::sym::EFFECT_CHAIN;
        if let Some(gv) = self.module.get_global(name) {
            return gv.as_pointer_value();
        }
        let ptr_ty = self.context.ptr_type(native_address_space());
        let gv = self.module.add_global(ptr_ty, Some(AddressSpace::from(0u16)), name);
        gv.set_linkage(Linkage::Internal);
        gv.set_initializer(&ptr_ty.const_null());
        // thread_local 标记。
        gv.set_thread_local_mode(Some(inkwell::ThreadLocalMode::GeneralDynamicTLSModel));
        gv.as_pointer_value()
    }

    /// 构造 `ScoopRootFrameHeader` LLVM 类型：`{ ptr prev; ptr desc }`。
    pub fn root_frame_header_type(&self) -> inkwell::types::StructType<'ctx> {
        let ptr = self.context.ptr_type(native_address_space());
        self.context.struct_type(&[ptr.into(), ptr.into()], false)
    }

    /// 构造 `ScoopRootFrameDesc` LLVM 类型：`{ i32 slot_count; ptr slot_offsets }`。
    pub fn root_frame_desc_type(&self) -> inkwell::types::StructType<'ctx> {
        let i32_ty = self.context.i32_type();
        let ptr = self.context.ptr_type(native_address_space());
        // 对齐 padding：i32 后跟 ptr 需要补 4 字节。用 packed=false 让 LLVM 自然对齐。
        self.context
            .struct_type(&[i32_ty.into(), ptr.into()], false)
    }

    /// 为一个函数创建 desc 全局（`ScoopRootFrameDesc` + offsets 数组）。
    /// 返回 desc 全局的指针。
    pub fn create_root_frame_desc_global(
        &self,
        fn_symbol: &str,
        slot_count: u32,
    ) -> PointerValue<'ctx> {
        let desc_name = format!("__scoop_root_desc_{fn_symbol}");
        if let Some(gv) = self.module.get_global(&desc_name) {
            return gv.as_pointer_value();
        }
        let i32_ty = self.context.i32_type();
        let offsets_name = format!("__scoop_root_offsets_{fn_symbol}");
        // offsets 数组：每个 slot 的字节偏移 = HEADER_SIZE + slot_index * ptr_size。
        let ptr_size = self.pointer_byte_size as u32;
        let offset_vals: Vec<_> = (0..slot_count)
            .map(|i| i32_ty.const_int((HEADER_SIZE_BYTES + i as u64 * ptr_size as u64), false))
            .collect();
        let offsets_arr_ty = i32_ty.array_type(slot_count.max(1));
        let offsets_global = self.module.add_global(
            offsets_arr_ty,
            Some(AddressSpace::from(0u16)),
            &offsets_name,
        );
        offsets_global.set_linkage(Linkage::Internal);
        offsets_global.set_constant(true);
        let offsets_init = if offset_vals.is_empty() {
            offsets_arr_ty.const_zero()
        } else {
            i32_ty.const_array(&offset_vals)
        };
        offsets_global.set_initializer(&offsets_init);

        let desc_ty = self.root_frame_desc_type();
        let desc_global =
            self.module
                .add_global(desc_ty, Some(AddressSpace::from(0u16)), &desc_name);
        desc_global.set_linkage(Linkage::Internal);
        desc_global.set_constant(true);
        let desc_init = desc_ty.const_named_struct(&[
            i32_ty.const_int(slot_count as u64, false).into(),
            offsets_global.as_pointer_value().into(),
        ]);
        desc_global.set_initializer(&desc_init);
        desc_global.as_pointer_value()
    }
}

/// per-function GC root frame 状态。
pub struct RootFrameState<'ctx> {
    /// frame alloca 指针（指向 header）；None = 无 GC（不建 frame）。
    pub frame_ptr: Option<PointerValue<'ctx>>,
    /// frame 的 LLVM struct 类型（{ header; [N x ptr] }）。
    pub frame_ty: Option<inkwell::types::StructType<'ctx>>,
    /// slot 数量。
    pub slot_count: u32,
    /// GC local id → frame slot index。
    pub local_to_slot: std::collections::HashMap<u32, u32>,
}

impl<'ctx> RootFrameState<'ctx> {
    /// 计算一个函数体需要的 GC slot 数，并建立 local id → slot index 映射。
    /// 仅对 gc_traceable 的 local 各分配 1 个 slot（当前 Scoop 的 GC local 都是基指针）。
    pub fn compute(body: &LirBody, layouts: &TypeLayoutTable) -> Self {
        let mut local_to_slot = std::collections::HashMap::new();
        let mut next_slot = 0u32;
        for d in &body.locals {
            if d.gc_traceable || is_gc_traceable_type(d.ty, layouts) {
                local_to_slot.insert(d.id, next_slot);
                next_slot += 1;
            }
        }
        RootFrameState {
            frame_ptr: None,
            frame_ty: None,
            slot_count: next_slot,
            local_to_slot,
        }
    }

    /// 是否需要 root frame（有 GC local）。
    pub fn needs_frame(&self) -> bool {
        self.slot_count > 0 || self.frame_ptr.is_some()
    }
}

/// 判断一个 TypeId 是否 GC-managed（与 LIR gc.rs 一致：引用/函数类型）。
pub fn is_gc_traceable_type(ty: TypeId, layouts: &TypeLayoutTable) -> bool {
    match layouts.get(ty) {
        Some(l) => matches!(
            l.kind,
            TypeLayoutKind::Reference { .. } | TypeLayoutKind::Function
        ),
        None => false,
    }
}

impl<'ctx> CodegenContext<'ctx> {
    /// GC 相关全局声明占位（保持 lib.rs 调用兼容）。
    pub fn declare_gc_globals(&self) -> CodegenResult<()> {
        // 提前声明 TLS 全局（确保 thread_local）。
        let _ = self.explicit_root_frame_top_global();
        Ok(())
    }
}
