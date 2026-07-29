//! codegen 顶层上下文：持有 inkwell Context/Module/TargetData + 全局缓存表。
//!
//! `CodegenContext` 在整个 `emit_program` 期间存活，负责：
//! - 管理 LLVM module；
//! - 缓存 TypeId → LLVM 类型（由 `types::TypeLowerer`）；
//! - 缓存 runtime 符号声明（由 `runtime_abi`）；
//! - 缓存全局（type_desc / vtable / itable / string literal）。

use std::cell::RefCell;
use std::collections::HashMap;

use inkwell::AddressSpace;
use inkwell::context::Context;
use inkwell::module::{Linkage, Module};
use inkwell::targets::{InitializationConfig, Target, TargetData, TargetMachine};
use inkwell::types::{AnyTypeEnum, BasicMetadataTypeEnum, BasicTypeEnum, FunctionType};
use inkwell::values::{FunctionValue, PointerValue};

use scoop2_hir::ty::TypeId;
use scoop2_lir::LirProgram;

use crate::error::{CodegenError, CodegenResult};
use crate::target::TargetInfo;

/// 指针地址空间：GC-managed 引用。
pub const GC_ADDRSPACE: u32 = 1;
/// 指针地址空间：native / C-ABI 指针。
pub const NATIVE_ADDRSPACE: u32 = 0;

/// GC 地址空间（addrspace 1）的 inkwell `AddressSpace`。
pub fn gc_address_space() -> AddressSpace {
    AddressSpace::from(GC_ADDRSPACE as u16)
}
/// native 地址空间（addrspace 0）的 inkwell `AddressSpace`。
pub fn native_address_space() -> AddressSpace {
    AddressSpace::from(NATIVE_ADDRSPACE as u16)
}

/// codegen 顶层上下文（持有 inkwell 资源）。
pub struct CodegenContext<'ctx> {
    /// inkwell context（由调用方持有，确保 'ctx 生命周期）。
    pub context: &'ctx Context,
    /// LLVM module。
    pub module: Module<'ctx>,
    /// target machine（用于 object 输出）。
    pub target_machine: TargetMachine,
    /// target data（用于 size/align/offset 查询）。
    pub target_data: TargetData,
    /// 指针字节大小。
    pub pointer_byte_size: u64,
    /// 目标信息。
    pub target_info: TargetInfo,

    // ---- 缓存表（RefCell 以便在 &self 上 mutate）----
    /// TypeId → LLVM 类型缓存。
    type_cache: RefCell<HashMap<TypeId, BasicTypeEnum<'ctx>>>,
    /// named struct FQN → LLVM struct 类型（避免重复创建）。
    named_struct_cache: RefCell<HashMap<String, BasicTypeEnum<'ctx>>>,
    /// runtime 符号 → FunctionValue 声明缓存。
    runtime_fn_cache: RefCell<HashMap<&'static str, FunctionValue<'ctx>>>,
    /// 用户/库函数符号 → FunctionValue 缓存（声明或定义）。
    callable_fn_cache: RefCell<HashMap<String, FunctionValue<'ctx>>>,
    /// 全局符号 → GlobalValue 缓存。
    global_cache: RefCell<HashMap<String, PointerValue<'ctx>>>,
    /// string literal content hash → 全局指针缓存。
    string_literal_cache: RefCell<HashMap<String, PointerValue<'ctx>>>,
    /// type descriptor FQN → 全局指针缓存。
    type_desc_cache: RefCell<HashMap<String, PointerValue<'ctx>>>,
    /// class_itables 数据（从 LirProgram 注入，供 globals 层构建 itable 全局）。
    pub class_itables_data: RefCell<Vec<scoop2_lir::ClassItableLayout>>,
    /// class vtables 数据（从 LirProgram 注入，供 globals 层构建 vtable 全局）。
    pub vtables_data: RefCell<Vec<scoop2_lir::VtableLayout>>,
    /// 类型布局表（从 LirProgram 注入，供 type descriptor 构建 trace_bitmap）。
    pub type_layouts: scoop2_lir::TypeLayoutTable,
    /// 类初始化计划（从 LirProgram 注入，提供 class 字段布局）。
    pub class_inits: Vec<scoop2_lir::ClassInitPlan>,
    /// interface FQN → interface_id 映射（从 LirProgram.itables 注入，
    /// 供 TypeTest/Is-pattern 对接口做 itable 遍历匹配）。
    pub interface_id_map: std::collections::HashMap<String, u64>,
}

impl<'ctx> CodegenContext<'ctx> {
    /// 创建新上下文。`context` 由调用方持有。
    pub fn new(
        context: &'ctx Context,
        program: &LirProgram,
        target_info: TargetInfo,
    ) -> CodegenResult<Self> {
        let type_layouts = program.type_layouts.clone();
        let class_inits = program.class_inits.clone();
        let interface_id_map = program
            .itables
            .iter()
            .map(|il| (il.interface_fqn.clone(), il.interface_id))
            .collect();
        // 初始化 native target（幂等）。
        Target::initialize_native(&InitializationConfig::default()).map_err(|e| {
            CodegenError::TargetOutput {
                message: format!("LLVM native target 初始化失败：{e}"),
            }
        })?;

        let triple = inkwell::targets::TargetMachine::get_default_triple();
        let target = Target::from_triple(&triple).map_err(|e| CodegenError::TargetOutput {
            message: format!("无法从 triple 获取 target：{e}"),
        })?;
        let cpu = inkwell::targets::TargetMachine::get_host_cpu_name().to_string();
        let features = inkwell::targets::TargetMachine::get_host_cpu_features().to_string();
        let target_machine = target
            .create_target_machine(
                &triple,
                &cpu,
                &features,
                inkwell::OptimizationLevel::None,
                inkwell::targets::RelocMode::Default,
                inkwell::targets::CodeModel::Default,
            )
            .ok_or_else(|| CodegenError::TargetOutput {
                message: format!("创建 target machine 失败（triple={}）", triple_str(&triple)),
            })?;
        let target_data = target_machine.get_target_data();
        let pointer_byte_size = target_data.get_pointer_byte_size(None) as u64;

        let module = context.create_module("scoop_program");
        module.set_triple(&triple);
        module.set_data_layout(&target_data.get_data_layout());

        Ok(CodegenContext {
            context,
            module,
            target_machine,
            target_data,
            pointer_byte_size,
            target_info,
            type_cache: RefCell::new(HashMap::new()),
            named_struct_cache: RefCell::new(HashMap::new()),
            runtime_fn_cache: RefCell::new(HashMap::new()),
            callable_fn_cache: RefCell::new(HashMap::new()),
            global_cache: RefCell::new(HashMap::new()),
            string_literal_cache: RefCell::new(HashMap::new()),
            type_desc_cache: RefCell::new(HashMap::new()),
            class_itables_data: RefCell::new(Vec::new()),
            vtables_data: RefCell::new(Vec::new()),
            type_layouts,
            class_inits,
            interface_id_map,
        })
    }

    // ---- 类型缓存 ----
    pub fn cache_type(&self, ty: TypeId, llvm_ty: BasicTypeEnum<'ctx>) {
        self.type_cache.borrow_mut().insert(ty, llvm_ty);
    }
    pub fn lookup_type(&self, ty: TypeId) -> Option<BasicTypeEnum<'ctx>> {
        self.type_cache.borrow().get(&ty).copied()
    }
    pub fn cache_named_struct(&self, fqn: String, llvm_ty: BasicTypeEnum<'ctx>) {
        self.named_struct_cache.borrow_mut().insert(fqn, llvm_ty);
    }
    pub fn lookup_named_struct(&self, fqn: &str) -> Option<BasicTypeEnum<'ctx>> {
        self.named_struct_cache.borrow().get(fqn).copied()
    }

    // ---- 函数缓存 ----
    pub fn cache_runtime_fn(&self, name: &'static str, fv: FunctionValue<'ctx>) {
        self.runtime_fn_cache.borrow_mut().insert(name, fv);
    }
    pub fn lookup_runtime_fn(&self, name: &str) -> Option<FunctionValue<'ctx>> {
        self.runtime_fn_cache.borrow().get(name).copied()
    }
    pub fn cache_callable_fn(&self, symbol: String, fv: FunctionValue<'ctx>) {
        self.callable_fn_cache.borrow_mut().insert(symbol, fv);
    }
    pub fn lookup_callable_fn(&self, symbol: &str) -> Option<FunctionValue<'ctx>> {
        self.callable_fn_cache.borrow().get(symbol).copied()
    }

    // ---- 全局缓存 ----
    pub fn cache_global(&self, name: String, ptr: PointerValue<'ctx>) {
        self.global_cache.borrow_mut().insert(name, ptr);
    }
    pub fn lookup_global(&self, name: &str) -> Option<PointerValue<'ctx>> {
        self.global_cache.borrow().get(name).copied()
    }
    pub fn cache_string_literal(&self, key: String, ptr: PointerValue<'ctx>) {
        self.string_literal_cache.borrow_mut().insert(key, ptr);
    }
    pub fn lookup_string_literal(&self, key: &str) -> Option<PointerValue<'ctx>> {
        self.string_literal_cache.borrow().get(key).copied()
    }
    pub fn cache_type_desc(&self, fqn: String, ptr: PointerValue<'ctx>) {
        self.type_desc_cache.borrow_mut().insert(fqn, ptr);
    }
    pub fn lookup_type_desc(&self, fqn: &str) -> Option<PointerValue<'ctx>> {
        self.type_desc_cache.borrow().get(fqn).copied()
    }

    /// 声明或获取一个外部（External linkage）函数。
    pub fn declare_external_fn(
        &self,
        name: &str,
        fn_ty: FunctionType<'ctx>,
    ) -> FunctionValue<'ctx> {
        if let Some(fv) = self.module.get_function(name) {
            return fv;
        }
        let fv = self
            .module
            .add_function(name, fn_ty, Some(Linkage::External));
        fv
    }
}

fn triple_str(triple: &inkwell::targets::TargetTriple) -> String {
    triple.as_str().to_string_lossy().into_owned()
}

/// 把 AnyTypeEnum 当成 BasicTypeEnum 取出（用于函数返回等场景的辅助）。
#[allow(dead_code)]
pub fn as_basic(ty: AnyTypeEnum<'_>) -> Option<BasicTypeEnum<'_>> {
    match ty {
        AnyTypeEnum::IntType(i) => Some(i.into()),
        AnyTypeEnum::FloatType(f) => Some(f.into()),
        AnyTypeEnum::PointerType(p) => Some(p.into()),
        AnyTypeEnum::StructType(s) => Some(s.into()),
        AnyTypeEnum::ArrayType(a) => Some(a.into()),
        AnyTypeEnum::VectorType(v) => Some(v.into()),
        AnyTypeEnum::ScalableVectorType(_)
        | AnyTypeEnum::FunctionType(_)
        | AnyTypeEnum::VoidType(_) => None,
    }
}

/// 用于 metadata 参数类型的转换辅助。
#[allow(dead_code)]
pub fn to_metadata(b: BasicTypeEnum<'_>) -> BasicMetadataTypeEnum<'_> {
    b.into()
}
