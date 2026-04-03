//! LLVM IR 生成（早期阶段，T0808～T0810）。
//!
//! 当前落点：支持把**入口函数 `fun main`** 与其调用到的顶层函数降低到单个 LLVM module：
//! - 入口保持为 `i32 @main()`（C ABI），其返回值作为进程退出码；
//! - 额外生成（或声明）被调用的顶层函数（先按简单 C ABI）。
//!
//! 表达式/语句子集（当前只覆盖早期最小回归需要）：
//! - 整数/布尔字面量；
//! - 一元运算：`!`、`-`、`~`；
//! - 二元运算：算术/比较/位运算/移位（含 shift count mask）；
//! - 局部绑定：`val`/`var`（映射为 `alloca` + `load/store`）；
//! - 赋值语句：`x = expr`（仅支持 local `var`）；
//! - `return`（以及“block 最后表达式”作为隐式返回）。
//! - `when`（T0813：仅支持 enum tag 判别 + variant binder；不支持 guard/or-pattern）。
//!
//! 非目标（后续任务逐步补齐）：
//! - if/loop 等更复杂控制流（依赖 MIR/CFG codegen 任务）。

use std::collections::{HashMap, HashSet};

use inkwell::AtomicOrdering;
use inkwell::AddressSpace;
use inkwell::IntPredicate;
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Linkage;
use inkwell::module::Module;
use inkwell::targets::TargetData;
use inkwell::types::BasicMetadataTypeEnum;
use inkwell::types::BasicType;
use inkwell::types::BasicTypeEnum;
use inkwell::types::IntType;
use inkwell::types::PointerType;
use inkwell::types::StructType;
use inkwell::values::AggregateValueEnum;
use inkwell::values::BasicValue;
use inkwell::values::BasicValueEnum;
use inkwell::values::FunctionValue;
use inkwell::values::GlobalValue;
use inkwell::values::IntValue;
use inkwell::values::PointerValue;

use crate::ast;
use crate::hir;
use crate::llvm::target::HostTargetInfo;
use crate::source::SourceFile;
use crate::syntax::string_literal::{
    StringLiteralParseError, parse_normal_string_bytes, parse_string_literal_bytes,
};
use crate::ty::layout::{NicheDomain, NicheStorage, TargetLayout, TypeLayout};
use crate::ty::{RefTypeKind, TypeId, TypeKind, TypeStore, ValueTypeKind};

use super::LlvmEmitError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct IntTy {
    bits: u32,
    signed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CgTy {
    Unit,
    Bool,
    Int(IntTy),
    Tuple(TypeId),
    Struct(TypeId),
    Enum(TypeId),
    /// runtime 字符串对象（early stage）
    ///
    /// 说明：
    /// - `scoop.core.String` 运行期表示为 `ScoopString*`：
    ///   - LLVM 侧使用 `addrspace(1)` 指针，表示其为 GC-managed heap 对象；
    ///   - 对象头为 `ScoopGcObjectHeader`（与 `scoop_alloc` 对齐），其后为 `{ len: i64, data: i8* }`；
    /// - 字符串字面量与 f-string 结果当前都会分配一个 `ScoopString` 对象（T1502b3）。
    String,
    /// 通用引用类型（Any / class / interface / function / union ...）。
    ///
    /// 当前阶段的 codegen 约定：
    /// - 一律用 `i8 addrspace(1)*` 表示（LLVM 文本 IR 在 opaque pointers 下通常显示为 `ptr addrspace(1)`）；
    /// - 值类型向引用类型的隐式转换需要装箱（T0817：先只支持 `Int -> Any`）。
    ///
    /// 未来将替换为带对象头（type descriptor/flags/size）的具体布局（PLAN §8.2/§9.1）。
    Ref,
}

/// LLVM GC address space（用于标记 GC-managed 引用指针，后续接入 statepoint/stackmap）。
///
/// 说明：
/// - 约定 `addrspace(1)` 为 GC-managed ref（与运行时 `scoop_alloc` 分配对象一致）；
/// - `addrspace(0)` 保留给“native/unsafe 指针”（例如 malloc buffer、C ABI out pointer、closure env 等）。
const GC_ADDRSPACE: u16 = 1;

// boxing / lint 的启发式阈值（与 typecheck::layout.rs 保持一致）。
const ENUM_BOX_DISPARITY_RATIO: u64 = 4;

/// flag-based unwinding（non-resuming effect）的“捕获边界”记录。
///
/// 说明：
/// - 当前阶段 `Raise.raise` 仍有独立的 `raise_target_stack`（历史原因，T0614）；
/// - T0625 起，为最小自定义 non-resuming effect 增加同样的“最近匹配”捕获边界栈，
///   用于在一个函数内把 `perform` 直接分发到最近的 `handle` catch block。
#[derive(Debug, Clone)]
struct EffectUnwindTarget<'ctx> {
    op_fqn: String,
    target: inkwell::basic_block::BasicBlock<'ctx>,
}
const ENUM_BOX_INLINE_THRESHOLD_WORDS: u64 = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CgEnumRepr {
    TaggedUnion,
    /// niche 优化：无显式 tag，通过 payload 的非法值编码 `None`。
    Niche {
        storage: NicheStorage,
        none_value: u64,
    },
    /// value-only enum：运行期表示就是底层整型标量（spec §2.3.2.1）。
    ValueOnly {
        underlying: IntTy,
    },
}

#[derive(Debug, Clone)]
struct CgEnumVariant {
    name: String,
    tag: u64,
    boxed: bool,
    fields: Vec<CgTy>,
}

#[derive(Debug, Clone)]
struct CgEnumLayout {
    repr: CgEnumRepr,
    variants: Vec<CgEnumVariant>,
}

#[derive(Debug, Clone, Copy)]
struct CgValue<'ctx> {
    ty: CgTy,
    value: Option<BasicValueEnum<'ctx>>,
}

impl<'ctx> CgValue<'ctx> {
    fn unit() -> Self {
        Self {
            ty: CgTy::Unit,
            value: None,
        }
    }

    fn int(value: IntValue<'ctx>, ty: IntTy) -> Self {
        Self {
            ty: CgTy::Int(ty),
            value: Some(value.into()),
        }
    }

    fn bool(value: IntValue<'ctx>) -> Self {
        Self {
            ty: CgTy::Bool,
            value: Some(value.into()),
        }
    }

    fn as_int(self) -> Option<(IntValue<'ctx>, IntTy)> {
        match self.ty {
            CgTy::Int(ty) => match self.value? {
                BasicValueEnum::IntValue(v) => Some((v, ty)),
                _ => None,
            },
            _ => None,
        }
    }

    fn as_bool(self) -> Option<IntValue<'ctx>> {
        match self.ty {
            CgTy::Bool => match self.value? {
                BasicValueEnum::IntValue(v) => Some(v),
                _ => None,
            },
            _ => None,
        }
    }
}

/// 一个局部变量（`val`/`var`）在 LLVM 里的存储形态。
///
/// 当前阶段（T0809）统一用栈分配（`alloca`）承载 locals，并用 `load/store` 实现读写。
#[derive(Debug, Clone, Copy)]
struct CgLocal<'ctx> {
    /// 该局部绑定在 HIR/type 层面的原始 `TypeId`（用于需要“精确类型结构”的场景，例如函数值调用）。
    ///
    /// 说明：
    /// - 早期 codegen 的 `CgTy::Ref` 统一覆盖所有引用类型（Any/class/function/union...），
    ///   但某些操作（例如调用函数值）仍需要区分具体的 `RefTypeKind::Function` 并读取其签名。
    /// - 对于无法在 codegen 阶段轻易恢复 `TypeId` 的合成 locals，可为 `None`。
    hir_ty: Option<TypeId>,
    ty: CgTy,
    ptr: PointerValue<'ctx>,
    mutable: bool,
    /// 若该 local 是“可被 GC 扫描的引用”，则这里记录其对应的 shadow stack roots slot 指针。
    gc_root_slot: Option<PointerValue<'ctx>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AtomicIntLvalueMode {
    ReadOnly,
    ReadWrite,
}

/// 当前函数的 shadow stack（GC roots）插桩状态（TODO T0816）。
struct GcFrameState<'ctx> {
    frame_ptr: PointerValue<'ctx>,
    root_slots: HashMap<hir::SymbolId, PointerValue<'ctx>>,
}

/// `-> resume` lowering（T0616）在 codegen 阶段使用的“立即恢复”上下文。
///
/// 说明：
/// - 当前实现先只覆盖“单个 perform 点”的最小栈上 state machine；
/// - `resume(value)` 会写入 `resume_value_ptr`、更新 `state_ptr`，并跳回 `dispatch_bb`。
#[derive(Debug, Clone, Copy)]
struct ImmediateResumeCtx<'ctx> {
    resume_symbol: hir::SymbolId,
    resume_value_ty: CgTy,
    resume_value_ptr: Option<PointerValue<'ctx>>,
    resume_used_ptr: PointerValue<'ctx>,
    state_ptr: PointerValue<'ctx>,
    next_state: u32,
}

#[derive(Debug, Default)]
struct Env<'ctx> {
    scopes: Vec<HashMap<hir::SymbolId, CgLocal<'ctx>>>,
}

impl<'ctx> Env<'ctx> {
    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        let _ = self.scopes.pop();
    }

    fn insert(&mut self, id: hir::SymbolId, local: CgLocal<'ctx>) {
        if let Some(frame) = self.scopes.last_mut() {
            frame.insert(id, local);
        }
    }

    fn get(&self, id: hir::SymbolId) -> Option<CgLocal<'ctx>> {
        for frame in self.scopes.iter().rev() {
            if let Some(local) = frame.get(&id).copied() {
                return Some(local);
            }
        }
        None
    }
}

pub(crate) struct MainCodegen<'a, 'ctx> {
    context: &'ctx Context,
    module: &'a Module<'ctx>,
    builder: &'a Builder<'ctx>,
    target_data: &'a TargetData,
    host: &'a HostTargetInfo,
    source: &'a SourceFile,
    types: &'a TypeStore,
    struct_layouts: &'a hir::StructLayoutIndex,
    enum_layouts: &'a hir::EnumLayoutIndex,
    top_level_vars: &'a hir::TopLevelVarIndex,
    extern_funs: &'a hir::ExternFunIndex,
    object_inits: &'a hir::ObjectInitIndex,
    class_inits: &'a hir::ClassInitIndex,
    ctor_call_sites: &'a hir::CtorCallSiteIndex,
    fun_index: &'a HashMap<String, &'a hir::FunDecl>,
    env: Env<'ctx>,
    /// `TypeId -> TypeLayout`（仅用于 codegen 侧的 niche 决策；不追求覆盖所有类型语法）。
    type_layout_cache: HashMap<TypeId, TypeLayout>,
    /// `Option<T>` niche 表示的 `None` 编码（用于嵌套 niche）。
    option_niche_cache: HashMap<TypeId, Option<(NicheStorage, u64)>>,
    /// `enum/Option` 的 codegen 表示选择与 boxing 决策缓存。
    enum_cg_layout_cache: HashMap<TypeId, CgEnumLayout>,
    /// `class FQN -> 继承链已展开的字段布局` 缓存。
    ///
    /// 说明：
    /// - 对于 `class Derived : Base()`，`Derived` 的对象 payload 需要以前缀形式包含 `Base` 的字段；
    /// - codegen 侧会把该布局“按继承链展开”，并把字段索引写回到 `field_indices`，以便 field GEP 正确。
    class_init_layout_cache: HashMap<String, hir::ClassInit>,
    gc_frame: Option<GcFrameState<'ctx>>,
    /// 当前正在生成的函数返回类型（用于 effect flag-based unwinding 的“早退返回默认值”）。
    ///
    /// 说明：
    /// - 当 `Raise.raise` 发生且当前不存在 handler boundary 时，需要沿调用链向外传播：
    ///   通过返回默认值结束当前函数，并保持 effect flag/slot 不被消费；
    /// - 若在 handler boundary 内，则会跳转到 catch 分支而不是 return。
    current_fun_return_ty: Option<CgTy>,
    /// Raise/try-catch 的“当前捕获边界”栈（用于最小 flag-based unwinding，TODO T0614）。
    ///
    /// 语义（当前阶段）：
    /// - `Raise.raise(e)`：写 slot + set flag，然后跳到栈顶 catch block；
    /// - 普通函数调用返回后：若 flag 被置位，则跳到栈顶 catch block；
    /// - 若栈为空，则返回默认值继续向外传播。
    raise_target_stack: Vec<inkwell::basic_block::BasicBlock<'ctx>>,
    /// 最小自定义 non-resuming effect 的“当前捕获边界”栈（T0625）。
    ///
    /// 语义：
    /// - `perform` 发生时，根据 op FQN 在该栈中从内到外查找最近匹配的 catch block，并跳转；
    /// - handle body 结束后必须 pop，保证 handler arm body 处于自身 dispatch scope 外（避免 self-capture）。
    effect_unwind_target_stack: Vec<EffectUnwindTarget<'ctx>>,
    /// `-> resume` lowering 的上下文栈（T0616）。
    ///
    /// 说明：handle arm body 内的 `resume(value)` 需要引用该上下文，因此用栈来支持嵌套 handle。
    immediate_resume_ctx_stack: Vec<ImmediateResumeCtx<'ctx>>,
    /// `, k ->`（escape continuation，T0617）在单个函数内生成 step trampoline 时使用的序号。
    escape_continuation_seq: u32,
}

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(crate) fn new(
        context: &'ctx Context,
        module: &'a Module<'ctx>,
        builder: &'a Builder<'ctx>,
        target_data: &'a TargetData,
        host: &'a HostTargetInfo,
        source: &'a SourceFile,
        types: &'a TypeStore,
        struct_layouts: &'a hir::StructLayoutIndex,
        enum_layouts: &'a hir::EnumLayoutIndex,
        top_level_vars: &'a hir::TopLevelVarIndex,
        object_inits: &'a hir::ObjectInitIndex,
        class_inits: &'a hir::ClassInitIndex,
        ctor_call_sites: &'a hir::CtorCallSiteIndex,
        extern_funs: &'a hir::ExternFunIndex,
        fun_index: &'a HashMap<String, &'a hir::FunDecl>,
    ) -> Self {
        Self {
            context,
            module,
            builder,
            target_data,
            host,
            source,
            types,
            struct_layouts,
            enum_layouts,
            top_level_vars,
            extern_funs,
            object_inits,
            class_inits,
            ctor_call_sites,
            fun_index,
            env: Env::default(),
            type_layout_cache: HashMap::new(),
            option_niche_cache: HashMap::new(),
            enum_cg_layout_cache: HashMap::new(),
            class_init_layout_cache: HashMap::new(),
            gc_frame: None,
            current_fun_return_ty: None,
            raise_target_stack: Vec::new(),
            effect_unwind_target_stack: Vec::new(),
            immediate_resume_ctx_stack: Vec::new(),
            escape_continuation_seq: 0,
        }
    }

    fn current_raise_target(&self) -> Option<inkwell::basic_block::BasicBlock<'ctx>> {
        self.raise_target_stack.last().copied()
    }

    fn push_raise_target(&mut self, target: inkwell::basic_block::BasicBlock<'ctx>) {
        self.raise_target_stack.push(target);
    }

    fn pop_raise_target(&mut self) {
        let _ = self.raise_target_stack.pop();
    }

    fn current_effect_unwind_target(
        &self,
        op_fqn: &str,
    ) -> Option<inkwell::basic_block::BasicBlock<'ctx>> {
        self.effect_unwind_target_stack
            .iter()
            .rev()
            .find(|t| t.op_fqn == op_fqn)
            .map(|t| t.target)
    }

    fn push_effect_unwind_target(
        &mut self,
        op_fqn: &str,
        target: inkwell::basic_block::BasicBlock<'ctx>,
    ) {
        self.effect_unwind_target_stack.push(EffectUnwindTarget {
            op_fqn: op_fqn.to_string(),
            target,
        });
    }

    fn pop_effect_unwind_target(&mut self) {
        let _ = self.effect_unwind_target_stack.pop();
    }

    fn current_immediate_resume_ctx(&self) -> Option<ImmediateResumeCtx<'ctx>> {
        self.immediate_resume_ctx_stack.last().copied()
    }

    fn push_immediate_resume_ctx(&mut self, ctx: ImmediateResumeCtx<'ctx>) {
        self.immediate_resume_ctx_stack.push(ctx);
    }

    fn pop_immediate_resume_ctx(&mut self) {
        let _ = self.immediate_resume_ctx_stack.pop();
    }

    fn class_init_layout(
        &mut self,
        at: crate::span::Span,
        class_fqn: &str,
    ) -> Result<hir::ClassInit, LlvmEmitError> {
        let mut visiting: HashSet<String> = HashSet::new();
        self.class_init_layout_inner(at, class_fqn, &mut visiting)
    }

    fn class_init_layout_inner(
        &mut self,
        at: crate::span::Span,
        class_fqn: &str,
        visiting: &mut HashSet<String>,
    ) -> Result<hir::ClassInit, LlvmEmitError> {
        if let Some(cached) = self.class_init_layout_cache.get(class_fqn).cloned() {
            return Ok(cached);
        }

        if !visiting.insert(class_fqn.to_string()) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "class inheritance cycle",
                at: at.into(),
            });
        }

        let base = self
            .class_inits
            .get(class_fqn)
            .cloned()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "class init info",
                at: at.into(),
            })?;

        let mut fields: Vec<hir::ClassField> = Vec::new();
        let mut field_indices: HashMap<String, u32> = HashMap::new();

        if let Some(super_fqn) = base.super_class_fqn.as_deref() {
            let super_layout = self.class_init_layout_inner(at, super_fqn, visiting)?;
            fields.extend(super_layout.fields);
            field_indices.extend(super_layout.field_indices);
        }

        for field in base.fields {
            let idx = fields.len() as u32;
            field_indices.insert(field.fqn.clone(), idx);
            fields.push(field);
        }

        let layouted = hir::ClassInit {
            fqn: base.fqn,
            super_class_fqn: base.super_class_fqn,
            super_ctor_args_span: base.super_ctor_args_span,
            super_ctor_args: base.super_ctor_args,
            this_id: base.this_id,
            fields,
            field_indices,
            steps: base.steps,
            ctors: base.ctors,
        };

        let _ = visiting.remove(class_fqn);
        self.class_init_layout_cache
            .insert(class_fqn.to_string(), layouted.clone());
        Ok(layouted)
    }

    fn class_init_chain(
        &mut self,
        at: crate::span::Span,
        class_fqn: &str,
    ) -> Result<Vec<hir::ClassInit>, LlvmEmitError> {
        let class = self.class_init_layout(at, class_fqn)?;
        match class.super_class_fqn.clone() {
            Some(super_fqn) => {
                let mut chain = self.class_init_chain(at, &super_fqn)?;
                chain.push(class);
                Ok(chain)
            }
            None => Ok(vec![class]),
        }
    }

    pub(crate) fn declare_top_level_fun(
        &mut self,
        fun: &hir::FunDecl,
    ) -> Result<FunctionValue<'ctx>, LlvmEmitError> {
        let llvm_name = self
            .extern_funs
            .get(&fun.fqn)
            .map(|e| e.symbol.as_str())
            .unwrap_or(fun.fqn.as_str());

        if let Some(existing) = self.module.get_function(llvm_name) {
            return Ok(existing);
        }

        let llvm_params = fun
            .params
            .iter()
            .map(|p| self.llvm_param_ty(p.span, p.ty))
            .collect::<Result<Vec<_>, _>>()?;

        let return_cg = self
            .cg_ty_of(fun.return_ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "function return type",
                at: fun.span.into(),
            })?;

        let fn_ty = match return_cg {
            CgTy::Unit => self.context.void_type().fn_type(&llvm_params, false),
            other => self.llvm_basic_type_of(fun.span, other)?.fn_type(&llvm_params, false),
        };

        let linkage = if self.extern_funs.contains_key(&fun.fqn) {
            Some(Linkage::External)
        } else {
            None
        };
        let llvm_fun = self.module.add_function(llvm_name, fn_ty, linkage);
        // `@CallingConvention(...)`：缺省为 C ABI（LLVM callconv 0）。
        llvm_fun.set_call_conventions(self.llvm_call_convention_for_fqn(&fun.fqn));
        Ok(llvm_fun)
    }

    fn llvm_call_convention_for_fqn(&self, fqn: &str) -> u32 {
        let Some(extern_fun) = self.extern_funs.get(fqn) else {
            return 0;
        };
        let Some(name) = extern_fun.calling_convention.as_deref() else {
            return 0;
        };

        match name.trim().to_ascii_lowercase().as_str() {
            "c" | "cdecl" => 0,
            // 其它 calling convention 名称留到后续任务再补齐（spec §15.5.4）。
            _ => 0,
        }
    }

    pub(crate) fn declare_top_level_var_globals(&mut self) -> Result<(), LlvmEmitError> {
        let mut vars: Vec<&hir::TopLevelVar> = self.top_level_vars.values().collect();
        vars.sort_by(|a, b| a.fqn.cmp(&b.fqn));
        for v in vars {
            let _ = self.declare_top_level_var_global(v)?;
        }
        Ok(())
    }

    fn declare_top_level_var_global(
        &mut self,
        v: &hir::TopLevelVar,
    ) -> Result<GlobalValue<'ctx>, LlvmEmitError> {
        let name = top_level_var_global_name(&v.fqn);
        if let Some(existing) = self.module.get_global(&name) {
            return Ok(existing);
        }

        let cg_ty = self
            .cg_ty_of(v.ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "top-level var type",
                at: v.span.into(),
            })?;

        let llvm_ty = self.llvm_basic_type_of(v.span, cg_ty)?;
        let gv = self.module.add_global(llvm_ty, None, &name);
        gv.set_linkage(Linkage::Internal);

        if v.storage == hir::TopLevelVarStorage::ThreadLocal {
            gv.set_thread_local(true);
        }

        let init = self.const_initializer_for_top_level_var(v, cg_ty, llvm_ty)?;
        gv.set_initializer(&init);

        // `@CLayout(aligned = N)`：对显式对齐的值类型，在全局存储上透传 alignment。
        if let CgTy::Struct(struct_ty) = cg_ty {
            if let Some(aligned) = self.struct_clayout(struct_ty).and_then(|c| c.aligned) {
                gv.set_alignment(aligned);
            }
        }
        Ok(gv)
    }

    fn const_initializer_for_top_level_var(
        &mut self,
        v: &hir::TopLevelVar,
        cg_ty: CgTy,
        llvm_ty: BasicTypeEnum<'ctx>,
    ) -> Result<BasicValueEnum<'ctx>, LlvmEmitError> {
        let Some(init) = v.init.as_ref() else {
            return Ok(self.zero_initializer_for_basic_type(llvm_ty));
        };

        Ok(match cg_ty {
            CgTy::Unit => self.context.i8_type().const_int(0, false).into(),
            CgTy::Bool => {
                let value =
                    self.const_eval_bool_expr(init)
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "top-level var initializer (bool const)",
                            at: init.span.into(),
                        })?;
                self.context
                    .bool_type()
                    .const_int(value as u64, false)
                    .into()
            }
            CgTy::Int(int_ty) => {
                let bits = self.const_eval_int_expr_bits(init, int_ty).ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "top-level var initializer (int const)",
                        at: init.span.into(),
                    },
                )?;
                let value = mask_to_bits(bits, int_ty.bits) as u64;
                self.int_type(int_ty).const_int(value, false).into()
            }
            // 早期阶段：仅支持“静态全零初始化”；更复杂的值类型常量构造留给后续任务补齐。
            CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "top-level var initializer (aggregate const)",
                    at: init.span.into(),
                });
            }
            CgTy::String | CgTy::Ref => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "top-level var initializer (non-gc-free)",
                    at: init.span.into(),
                });
            }
        })
    }

    fn const_eval_int_expr_bits(&self, expr: &hir::Expr, int_ty: IntTy) -> Option<u128> {
        match &expr.kind {
            hir::ExprKind::Literal(hir::LiteralKind::Int) => {
                let text = self.source.slice(expr.span);
                let value = parse_int_literal_decimal(text);
                Some(mask_to_bits(value, int_ty.bits))
            }
            hir::ExprKind::Unary {
                op: ast::UnaryOp::Neg,
                expr: inner,
                ..
            } => {
                let v = self.const_eval_int_expr_bits(inner, int_ty)?;
                Some(mask_to_bits(0u128.wrapping_sub(v), int_ty.bits))
            }
            hir::ExprKind::Unary {
                op: ast::UnaryOp::BitNot,
                expr: inner,
                ..
            } => {
                let v = self.const_eval_int_expr_bits(inner, int_ty)?;
                Some(mask_to_bits(!v, int_ty.bits))
            }
            _ => None,
        }
    }

    fn const_eval_bool_expr(&self, expr: &hir::Expr) -> Option<bool> {
        match &expr.kind {
            hir::ExprKind::Literal(hir::LiteralKind::Bool(v)) => Some(*v),
            hir::ExprKind::Unary {
                op: ast::UnaryOp::Not,
                expr: inner,
                ..
            } => Some(!self.const_eval_bool_expr(inner)?),
            _ => None,
        }
    }

    fn zero_initializer_for_basic_type(
        &self,
        llvm_ty: BasicTypeEnum<'ctx>,
    ) -> BasicValueEnum<'ctx> {
        match llvm_ty {
            BasicTypeEnum::IntType(ty) => BasicValueEnum::IntValue(ty.const_int(0, false)),
            BasicTypeEnum::PointerType(ty) => BasicValueEnum::PointerValue(ty.const_null()),
            BasicTypeEnum::StructType(ty) => BasicValueEnum::StructValue(ty.const_zero()),
            BasicTypeEnum::ArrayType(ty) => BasicValueEnum::ArrayValue(ty.const_zero()),
            BasicTypeEnum::FloatType(ty) => BasicValueEnum::FloatValue(ty.const_float(0.0)),
            BasicTypeEnum::VectorType(ty) => BasicValueEnum::VectorValue(ty.const_zero()),
            BasicTypeEnum::ScalableVectorType(ty) => {
                BasicValueEnum::ScalableVectorValue(ty.const_zero())
            }
        }
    }

    pub(crate) fn codegen_top_level_fun(
        mut self,
        fun: &hir::FunDecl,
        llvm_fun: FunctionValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let Some(body) = fun.body.as_ref() else {
            // extern / declaration-only：由调用点按需声明即可，这里不生成 body。
            return Ok(());
        };

        let entry = self.context.append_basic_block(llvm_fun, "entry");
        self.builder.position_at_end(entry);

        self.setup_gc_frame(fun)?;

        self.env.push_scope();
        self.codegen_fun_params(fun, llvm_fun)?;

        let declared_return_cg =
            self.cg_ty_of(fun.return_ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "function return type",
                    at: fun.span.into(),
                })?;
        self.current_fun_return_ty = Some(declared_return_cg);
        let ret_v = self.codegen_block_as_return_value(body, declared_return_cg)?;
        self.emit_return(fun.span, declared_return_cg, ret_v)?;

        self.env.pop_scope();
        Ok(())
    }

    fn setup_gc_frame(&mut self, fun: &hir::FunDecl) -> Result<(), LlvmEmitError> {
        let root_ids = self.collect_gc_root_ids(fun);
        self.setup_gc_frame_for_root_ids(fun.span, &root_ids)
    }

    fn setup_gc_frame_for_root_ids(
        &mut self,
        at: crate::span::Span,
        root_ids: &[hir::SymbolId],
    ) -> Result<(), LlvmEmitError> {
        if root_ids.is_empty() {
            return Ok(());
        }

        let root_count = root_ids.len() as u32;
        let frame_ty = self.llvm_gc_frame_type(root_count);

        // 说明：frame 本身也是栈上的一个 local；放在 entry block 的 alloca 区域，便于后续
        // 统一做 mem2reg / 优化（以及未来可能的 `gc.statepoint` 迁移）。
        let frame_ptr = self.create_entry_alloca_raw(at, "gc_frame", frame_ty.into())?;

        let i32_ty = self.context.i32_type();
        let i8_ptr_ty = self.llvm_i8_ptr_type();
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();

        // 初始化 header：root_count + reserved + prev（prev 将由 push 写入，但这里也写 0 以便调试）。
        let prev_ptr = self
            .builder
            .build_struct_gep(frame_ty, frame_ptr, 0, "gc_prev_gep")?;
        let root_count_ptr =
            self.builder
                .build_struct_gep(frame_ty, frame_ptr, 1, "gc_root_count_gep")?;
        let reserved_ptr =
            self.builder
                .build_struct_gep(frame_ty, frame_ptr, 2, "gc_reserved_gep")?;

        let _ = self.builder.build_store(prev_ptr, i8_ptr_ty.const_null())?;
        let _ = self
            .builder
            .build_store(root_count_ptr, i32_ty.const_int(root_count as u64, false))?;
        let _ = self
            .builder
            .build_store(reserved_ptr, i32_ty.const_zero())?;

        // roots[] 初始化为 NULL，并记录每个 local 对应的 slot 指针。
        let roots_arr_ptr =
            self.builder
                .build_struct_gep(frame_ty, frame_ptr, 3, "gc_roots_arr_gep")?;
        let roots_base = self.builder.build_pointer_cast(
            roots_arr_ptr,
            gc_i8_ptr_ty.ptr_type(AddressSpace::default()),
            "gc_roots_base",
        )?;

        let mut root_slots: HashMap<hir::SymbolId, PointerValue<'ctx>> =
            HashMap::with_capacity(root_ids.len());
        for (idx, id) in root_ids.iter().enumerate() {
            let index = i32_ty.const_int(idx as u64, false);
            let slot_ptr = unsafe {
                self.builder.build_in_bounds_gep(
                    gc_i8_ptr_ty,
                    roots_base,
                    &[index],
                    &format!("gc_root_slot_{idx}"),
                )?
            };
            let _ = self.builder.build_store(slot_ptr, gc_i8_ptr_ty.const_null())?;
            root_slots.insert(*id, slot_ptr);
        }

        // push(frame)
        let push = self.declare_runtime_gc_frame_push();
        let frame_i8 = self
            .builder
            .build_pointer_cast(frame_ptr, i8_ptr_ty, "gc_frame_i8")?;
        let _ = self
            .builder
            .build_call(push, &[frame_i8.into()], "gc_frame_push")?;

        self.gc_frame = Some(GcFrameState {
            frame_ptr,
            root_slots,
        });
        Ok(())
    }

    fn gc_root_slot_for(&self, id: hir::SymbolId) -> Option<PointerValue<'ctx>> {
        self.gc_frame
            .as_ref()
            .and_then(|state| state.root_slots.get(&id).copied())
    }

    fn store_gc_root_slot_value(
        &mut self,
        at: crate::span::Span,
        slot_ptr: PointerValue<'ctx>,
        value: CgValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let Some(raw) = value.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "gc root value",
                at: at.into(),
            });
        };
        let BasicValueEnum::PointerValue(ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "gc root value type",
                at: at.into(),
            });
        };

        let i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let casted = self
            .builder
            .build_pointer_cast(ptr, i8_ptr_ty, "gc_root_i8")?;
        let _ = self.builder.build_store(slot_ptr, casted)?;
        Ok(())
    }

    pub(crate) fn codegen_main_exit_code(
        mut self,
        fun: &hir::FunDecl,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        // 入口 `i32 @main()` 的返回类型固定为 i32；这里记录下来以便最小 Raise 传播时能“早退”。
        self.current_fun_return_ty = Some(CgTy::Int(IntTy {
            bits: 32,
            signed: true,
        }));

        // T0910：GC v0 mark-sweep 需要能够扫描入口函数的 roots（run-pass fixtures 会在 main 中触发 GC）。
        self.setup_gc_frame(fun)?;

        self.env.push_scope();

        let exit = match fun.body.as_ref() {
            Some(body) => self.codegen_block_as_exit_code(body, fun.return_ty)?,
            None => self.context.i32_type().const_int(0, false),
        };

        // main 的返回由调用点在外层插入（见 `llvm/mod.rs`），因此这里需要显式 pop gc frame。
        if let Some(gc) = self.gc_frame.as_ref() {
            let pop = self.declare_runtime_gc_frame_pop();
            let i8_ptr_ty = self.context.i8_type().ptr_type(AddressSpace::default());
            let frame_i8 =
                self.builder
                    .build_pointer_cast(gc.frame_ptr, i8_ptr_ty, "gc_frame_i8")?;
            let _ = self
                .builder
                .build_call(pop, &[frame_i8.into()], "gc_frame_pop")?;
        }

        self.env.pop_scope();
        Ok(exit)
    }

    fn collect_gc_root_ids(&self, fun: &hir::FunDecl) -> Vec<hir::SymbolId> {
        let mut out: Vec<hir::SymbolId> = Vec::new();
        let mut seen: HashSet<hir::SymbolId> = HashSet::new();

        for param in &fun.params {
            if matches!(self.cg_ty_of(param.ty), Some(CgTy::Ref) | Some(CgTy::String))
                && seen.insert(param.id)
            {
                out.push(param.id);
            }
        }

        if let Some(body) = fun.body.as_ref() {
            self.collect_gc_root_ids_in_block(body, &mut out, &mut seen);
        }

        out
    }

    fn collect_gc_root_ids_in_block(
        &self,
        block: &hir::Block,
        out: &mut Vec<hir::SymbolId>,
        seen: &mut HashSet<hir::SymbolId>,
    ) {
        for stmt in &block.stmts {
            match &stmt.kind {
                hir::StmtKind::Empty => {}
                hir::StmtKind::Val(decl) => {
                    if matches!(self.cg_ty_of(decl.ty), Some(CgTy::Ref) | Some(CgTy::String)) {
                        if let Some(id) = decl.id {
                            if seen.insert(id) {
                                out.push(id);
                            }
                        }
                    }
                    if let Some(init) = decl.init.as_ref() {
                        self.collect_gc_root_ids_in_expr(init, out, seen);
                    }
                }
                hir::StmtKind::Expr(expr) => self.collect_gc_root_ids_in_expr(expr, out, seen),
                hir::StmtKind::Assign { lhs, rhs, .. } => {
                    self.collect_gc_root_ids_in_expr(lhs, out, seen);
                    self.collect_gc_root_ids_in_expr(rhs, out, seen);
                }
                hir::StmtKind::While { cond, body } => {
                    self.collect_gc_root_ids_in_expr(cond, out, seen);
                    self.collect_gc_root_ids_in_block(body, out, seen);
                }
                hir::StmtKind::Return { value } => {
                    if let Some(expr) = value.as_ref() {
                        self.collect_gc_root_ids_in_expr(expr, out, seen);
                    }
                }
                hir::StmtKind::Break { .. }
                | hir::StmtKind::Continue { .. }
                | hir::StmtKind::Todo(_) => {}
            }
        }
    }

    fn collect_gc_root_ids_in_expr(
        &self,
        expr: &hir::Expr,
        out: &mut Vec<hir::SymbolId>,
        seen: &mut HashSet<hir::SymbolId>,
    ) {
        match &expr.kind {
            hir::ExprKind::Missing | hir::ExprKind::Todo(_) => {}
            hir::ExprKind::Literal(_)
            | hir::ExprKind::VarRef(_)
            | hir::ExprKind::UnresolvedIdent { .. } => {}
            hir::ExprKind::StructLit { fields, .. } => {
                for field in fields {
                    self.collect_gc_root_ids_in_expr(&field.value, out, seen);
                }
            }
            hir::ExprKind::TupleLit { elements } => {
                for elem in elements {
                    self.collect_gc_root_ids_in_expr(elem, out, seen);
                }
            }
            hir::ExprKind::InterpolatedString { parts, .. } => {
                for part in parts {
                    if let hir::InterpolatedStringPart::Expr { expr } = part {
                        self.collect_gc_root_ids_in_expr(expr, out, seen);
                    }
                }
            }
            hir::ExprKind::Unary { expr, .. } => self.collect_gc_root_ids_in_expr(expr, out, seen),
            hir::ExprKind::Binary { lhs, rhs, .. } => {
                self.collect_gc_root_ids_in_expr(lhs, out, seen);
                self.collect_gc_root_ids_in_expr(rhs, out, seen);
            }
            hir::ExprKind::Block(block) => self.collect_gc_root_ids_in_block(block, out, seen),
            hir::ExprKind::Closure(_) => {
                // closure body 将在其自身的函数体里插桩；此处不把 closure 内部 locals 计入外层函数。
            }
            hir::ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                self.collect_gc_root_ids_in_expr(cond, out, seen);
                self.collect_gc_root_ids_in_expr(then_branch, out, seen);
                if let Some(else_branch) = else_branch.as_ref() {
                    self.collect_gc_root_ids_in_expr(else_branch, out, seen);
                }
            }
            hir::ExprKind::When { subject, arms } => {
                self.collect_gc_root_ids_in_expr(subject, out, seen);
                for arm in arms {
                    if let Some(guard) = arm.guard.as_ref() {
                        self.collect_gc_root_ids_in_expr(guard, out, seen);
                    }
                    self.collect_gc_root_ids_in_expr(&arm.body, out, seen);
                }
            }
            hir::ExprKind::Call { callee, args } => {
                self.collect_gc_root_ids_in_expr(callee, out, seen);
                for arg in args {
                    match arg {
                        hir::CallArg::Positional(expr) => {
                            self.collect_gc_root_ids_in_expr(expr, out, seen);
                        }
                        hir::CallArg::Named { value, .. } => {
                            self.collect_gc_root_ids_in_expr(value, out, seen);
                        }
                    }
                }
            }
            hir::ExprKind::MemberAccess { receiver, .. } => {
                self.collect_gc_root_ids_in_expr(receiver, out, seen);
            }
            hir::ExprKind::Perform { args, .. } => {
                for arg in args {
                    match arg {
                        hir::CallArg::Positional(expr) => {
                            self.collect_gc_root_ids_in_expr(expr, out, seen);
                        }
                        hir::CallArg::Named { value, .. } => {
                            self.collect_gc_root_ids_in_expr(value, out, seen);
                        }
                    }
                }
            }
            hir::ExprKind::Handle(handle) => {
                self.collect_gc_root_ids_in_block(&handle.body, out, seen);
                for arm in &handle.arms {
                    // handler arm binder 也可能是引用类型（例如 `catch (e: Any)`）：
                    // - lowering 后会变成一个局部 slot；
                    // - 若它是 ref，则必须为其分配 GC root slot，避免 error 值被错误回收。
                    for binder in &arm.op.binders {
                        if matches!(self.cg_ty_of(binder.ty), Some(CgTy::Ref))
                            && seen.insert(binder.id)
                        {
                            out.push(binder.id);
                        }
                    }
                    // `, k ->`：continuation binder 本身也是引用类型（Continuation 是 class）。
                    if let hir::HandleArmKind::EscapeContinuation { continuation } = arm.kind {
                        if seen.insert(continuation) {
                            out.push(continuation);
                        }
                    }
                    self.collect_gc_root_ids_in_expr(&arm.body, out, seen);
                }
                if let Some(finally) = handle.finally.as_ref() {
                    self.collect_gc_root_ids_in_block(finally, out, seen);
                }
            }
        }
    }

    fn codegen_block_as_exit_code(
        &mut self,
        block: &hir::Block,
        declared_return_ty: TypeId,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        // block 是表达式：若末尾是表达式语句，则它的值作为 block value。
        let mut tail_value: Option<CgValue<'ctx>> = None;
        let declared_return_cg = self.cg_ty_of(declared_return_ty).unwrap_or(CgTy::Unit);
        // main 的隐式返回只关心 `Int/Bool`（用于 exit code）；其它返回类型一律忽略，且不应把
        // “期望返回类型”强行向下传播到最后一个表达式（避免触发不必要的 coercion 失败）。
        let expected_tail_cg = match declared_return_cg {
            CgTy::Int(_) | CgTy::Bool => declared_return_cg,
            _ => CgTy::Unit,
        };

        self.env.push_scope();

        for (idx, stmt) in block.stmts.iter().enumerate() {
            let is_last = idx + 1 == block.stmts.len();
            match &stmt.kind {
                hir::StmtKind::Empty => {}
                hir::StmtKind::Val(decl) => {
                    self.codegen_val_decl(decl)?;
                    tail_value = None;
                }
                hir::StmtKind::Assign { lhs, eq_span, rhs } => {
                    self.codegen_assign_stmt(*eq_span, lhs, rhs)?;
                    tail_value = None;
                }
                hir::StmtKind::Expr(expr) => {
                    let expected = if is_last {
                        Some(expected_tail_cg)
                    } else {
                        Some(CgTy::Unit)
                    };
                    let v = self.codegen_expr_in_expected_context(expr, expected)?;
                    if is_last {
                        tail_value = Some(v);
                    } else {
                        tail_value = None;
                    }
                }
                hir::StmtKind::Return { value } => {
                    let exit = match value {
                        Some(expr) => {
                            let v = self.codegen_expr_in_expected_context(expr, Some(declared_return_cg))?;
                            self.coerce_exit_code(expr.span, v)?
                        }
                        None => self.context.i32_type().const_int(0, false),
                    };

                    self.env.pop_scope();
                    return Ok(exit);
                }
                // 控制流留待后续任务（需要 function-level CFG/MIR codegen）。
                hir::StmtKind::While { .. }
                | hir::StmtKind::Break { .. }
                | hir::StmtKind::Continue { .. }
                | hir::StmtKind::Todo(_) => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "statement",
                        at: stmt.span.into(),
                    });
                }
            }
        }

        // 隐式返回：当函数声明了整数/Bool 返回类型时，允许用 block tail value 作为返回值。
        let exit = if let Some(v) = tail_value {
            match self.cg_ty_of(declared_return_ty) {
                Some(CgTy::Int(_) | CgTy::Bool) => self.coerce_exit_code(block.span, v)?,
                _ => self.context.i32_type().const_int(0, false),
            }
        } else {
            self.context.i32_type().const_int(0, false)
        };

        self.env.pop_scope();
        Ok(exit)
    }

    fn codegen_val_decl(&mut self, decl: &hir::ValDecl) -> Result<(), LlvmEmitError> {
        let Some(id) = decl.id else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "anonymous val binding",
                at: decl.span.into(),
            });
        };

        let target_ty = self
            .cg_ty_of(decl.ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "val type",
                at: decl.span.into(),
            })?;

        let init = match decl.init.as_ref() {
            Some(expr) => match &expr.kind {
                hir::ExprKind::Closure(closure) => {
                    self.codegen_closure_expr(expr.span, closure, decl.ty)?
                }
                _ => self.codegen_expr_in_expected_context(expr, Some(target_ty))?,
            },
            None => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "val without initializer",
                    at: decl.span.into(),
                });
            }
        };

        // T0809：局部变量统一降为 alloca + store/load；`val/var` 仅在“是否允许赋值”上有差异。
        let name = decl.name.as_deref().unwrap_or("local");
        let ptr = self.create_entry_alloca(decl.span, name, target_ty)?;
        let stored = self.store_local_value(decl.span, ptr, target_ty, init)?;

        let gc_root_slot = self.gc_root_slot_for(id);
        if let Some(slot_ptr) = gc_root_slot {
            self.store_gc_root_slot_value(decl.span, slot_ptr, stored)?;
        }

        self.env.insert(
            id,
            CgLocal {
                hir_ty: Some(decl.ty),
                ty: target_ty,
                ptr,
                mutable: decl.mutable,
                gc_root_slot,
            },
        );
        Ok(())
    }

    fn codegen_expr_in_expected_context(
        &mut self,
        expr: &hir::Expr,
        expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match &expr.kind {
            hir::ExprKind::UnresolvedIdent { name } => {
                self.codegen_unresolved_ident(expr.span, name, expected)
            }
            hir::ExprKind::Call { callee, args } => {
                self.codegen_call(expr.span, callee, args, expected)
            }
            hir::ExprKind::Perform { op, args } => {
                self.codegen_perform_expr(expr.span, op, args, expected)
            }
            hir::ExprKind::Handle(handle) => self.codegen_handle_expr(expr.span, handle, expected),
            hir::ExprKind::Block(block) => {
                self.codegen_block_value_in_expected_context(block, expected)
            }
            hir::ExprKind::When { subject, arms } => {
                self.codegen_when_expr(expr.span, subject, arms, expected)
            }
            hir::ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => self.codegen_if_expr(
                expr.span,
                expr.ty,
                cond,
                then_branch,
                else_branch.as_deref(),
                expected,
            ),
            _ => self.codegen_expr(expr),
        }
    }

    fn codegen_assign_stmt(
        &mut self,
        eq_span: crate::span::Span,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
    ) -> Result<(), LlvmEmitError> {
        match &lhs.kind {
            hir::ExprKind::VarRef(vref) => match vref {
                hir::ValueRef::Local { id, .. } => {
                    let local = self
                        .env
                        .get(*id)
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "unknown local value",
                            at: lhs.span.into(),
                        })?;

                    if !local.mutable {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "assignment to immutable local",
                            at: eq_span.into(),
                        });
                    }

                    let rhs_v = self.codegen_expr_in_expected_context(rhs, Some(local.ty))?;
                    let stored = self.store_local_value(eq_span, local.ptr, local.ty, rhs_v)?;
                    if let Some(slot_ptr) = local.gc_root_slot {
                        self.store_gc_root_slot_value(eq_span, slot_ptr, stored)?;
                    }
                    Ok(())
                }
                hir::ValueRef::TopLevel { fqn, .. } => {
                    let Some(var) = self.top_level_vars.get(fqn) else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "assignment to non-local",
                            at: lhs.span.into(),
                        });
                    };

                    let cg_ty =
                        self.cg_ty_of(var.ty)
                            .ok_or(LlvmEmitError::UnsupportedMainBody {
                                kind: "top-level var type",
                                at: var.span.into(),
                            })?;

                    let gv = self.declare_top_level_var_global(var)?;
                    let rhs_v = self.codegen_expr_in_expected_context(rhs, Some(cg_ty))?;
                    let _stored =
                        self.store_local_value(eq_span, gv.as_pointer_value(), cg_ty, rhs_v)?;
                    Ok(())
                }
            },
            hir::ExprKind::MemberAccess { receiver, member } => {
                let Some(hir::MemberRef::Value { fqn, .. }) = member.resolved.as_ref() else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "assignment lhs member target",
                        at: lhs.span.into(),
                    });
                };

                let Some((class, field_idx, field_cg)) =
                    self.lookup_class_field_by_fqn(fqn, member.span)?
                else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "assignment lhs",
                        at: lhs.span.into(),
                    });
                };

                let field = class.fields.get(field_idx as usize).ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "assignment class field index",
                        at: lhs.span.into(),
                    },
                )?;
                if !field.mutable {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "assignment to immutable class field",
                        at: eq_span.into(),
                    });
                }

                let recv = self.codegen_expr_in_expected_context(receiver, Some(CgTy::Ref))?;
                let recv = self.coerce_value(receiver.span, recv, CgTy::Ref)?;
                let Some(raw) = recv.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "assignment class receiver value",
                        at: receiver.span.into(),
                    });
                };
                let BasicValueEnum::PointerValue(obj_ptr) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "assignment class receiver type",
                        at: receiver.span.into(),
                    });
                };

                let rhs_v = self.codegen_expr_in_expected_context(rhs, Some(field_cg))?;
                let field_ptr =
                    self.codegen_class_field_ptr(eq_span, &class, obj_ptr, field_idx)?;
                let _stored = self.store_local_value(eq_span, field_ptr, field_cg, rhs_v)?;
                Ok(())
            }
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "assignment lhs",
                at: lhs.span.into(),
            }),
        }
    }

    fn codegen_block_stmt(&mut self, block: &hir::Block) -> Result<(), LlvmEmitError> {
        self.env.push_scope();

        for stmt in &block.stmts {
            match &stmt.kind {
                hir::StmtKind::Empty => {}
                hir::StmtKind::Val(decl) => self.codegen_val_decl(decl)?,
                hir::StmtKind::Assign { lhs, eq_span, rhs } => {
                    self.codegen_assign_stmt(*eq_span, lhs, rhs)?;
                }
                hir::StmtKind::Expr(expr) => {
                    let _ = self.codegen_expr(expr)?;
                }
                hir::StmtKind::While { cond, body } => {
                    self.codegen_while_stmt(stmt.span, cond, body)?;
                }
                // 当前阶段（expr-based codegen）不支持在 block/loop 内部使用 `return/break/continue`：
                // 这需要 function-level CFG/MIR codegen（见 PLAN §8）。
                hir::StmtKind::Return { .. }
                | hir::StmtKind::Break { .. }
                | hir::StmtKind::Continue { .. }
                | hir::StmtKind::Todo(_) => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "statement",
                        at: stmt.span.into(),
                    });
                }
            }
        }

        self.env.pop_scope();
        Ok(())
    }

    fn codegen_while_stmt(
        &mut self,
        at: crate::span::Span,
        cond: &hir::Expr,
        body: &hir::Block,
    ) -> Result<(), LlvmEmitError> {
        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: at.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: at.into(),
            })?;

        let cond_bb = self.context.append_basic_block(func, "while_cond");
        let body_bb = self.context.append_basic_block(func, "while_body");
        let after_bb = self.context.append_basic_block(func, "while_after");

        self.builder.build_unconditional_branch(cond_bb)?;

        self.builder.position_at_end(cond_bb);
        let cv = self.codegen_expr_in_expected_context(cond, Some(CgTy::Bool))?;
        let cb = cv.as_bool().ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "while cond value",
            at: cond.span.into(),
        })?;
        self.builder
            .build_conditional_branch(cb, body_bb, after_bb)?;

        self.builder.position_at_end(body_bb);
        self.codegen_block_stmt(body)?;

        let body_end = self.builder.get_insert_block().ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "builder has no insert block",
            at: at.into(),
        })?;
        if body_end.get_terminator().is_none() {
            self.builder.build_unconditional_branch(cond_bb)?;
        }

        self.builder.position_at_end(after_bb);
        Ok(())
    }

    fn codegen_expr(&mut self, expr: &hir::Expr) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match &expr.kind {
            hir::ExprKind::Missing | hir::ExprKind::Todo(_) => {
                Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "expression",
                    at: expr.span.into(),
                })
            }
            hir::ExprKind::UnresolvedIdent { .. } => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "unresolved ident (missing expected type context)",
                at: expr.span.into(),
            }),
            hir::ExprKind::Literal(lit) => self.codegen_literal(expr.span, expr.ty, lit),
            hir::ExprKind::VarRef(v) => self.codegen_var_ref(expr.span, v),
            hir::ExprKind::StructLit { ty, fields } => {
                self.codegen_struct_lit(expr.span, *ty, fields)
            }
            hir::ExprKind::TupleLit { elements } => {
                self.codegen_tuple_lit(expr.span, expr.ty, elements)
            }
            hir::ExprKind::InterpolatedString { raw, parts } => {
                self.codegen_interpolated_string(expr.span, *raw, parts)
            }
            hir::ExprKind::Unary {
                op, expr: inner, ..
            } => self.codegen_unary(expr.span, *op, inner),
            hir::ExprKind::Binary { lhs, op, rhs, .. } => {
                self.codegen_binary(expr.span, *op, lhs, rhs)
            }
            hir::ExprKind::Block(block) => self.codegen_block_value(block),
            hir::ExprKind::Call { callee, args } => {
                self.codegen_call(expr.span, callee, args, None)
            }
            hir::ExprKind::MemberAccess { receiver, member } => {
                self.codegen_member_access(expr.span, receiver, member)
            }
            hir::ExprKind::When { subject, arms } => {
                self.codegen_when_expr(expr.span, subject, arms, None)
            }
            hir::ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => self.codegen_if_expr(
                expr.span,
                expr.ty,
                cond,
                then_branch,
                else_branch.as_deref(),
                None,
            ),

            // 后续任务接入 MIR/CFG codegen
            hir::ExprKind::Closure(_) => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "expression kind",
                at: expr.span.into(),
            }),
            hir::ExprKind::Perform { op, args } => {
                self.codegen_perform_expr(expr.span, op, args, None)
            }
            hir::ExprKind::Handle(handle) => self.codegen_handle_expr(expr.span, handle, None),
        }
    }

    /// 读取运行时 TLS effect flag，并返回 `i1`（是否 active）。
    ///
    /// 说明：这里直接调用 runtime C ABI（`scoop_effect_is_active`），避免把该读取当作“普通函数调用”
    /// 从而触发递归插桩（call site 检查 flag → 再调用 is_active → 再检查...）。
    fn emit_effect_is_active_i1(
        &mut self,
        at: crate::span::Span,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let rt = self.declare_runtime_effect_is_active();
        let call = self.builder.build_call(rt, &[], "effect_is_active")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "effect is_active return value",
                at: at.into(),
            })?;
        let BasicValueEnum::IntValue(active_i32) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "effect is_active return type",
                at: at.into(),
            });
        };
        Ok(self.builder.build_int_compare(
            IntPredicate::NE,
            active_i32,
            self.context.i32_type().const_zero(),
            "effect_active",
        )?)
    }

    /// 在“最近 handler boundary”存在时跳转到 catch；否则返回默认值向外传播。
    ///
    /// 用途：
    /// - 普通函数调用返回后：callee 可能执行 `Raise.raise`，因此返回后需要检查 flag 并决定是否 unwind。
    fn emit_effect_unwind_if_active(&mut self, at: crate::span::Span) -> Result<(), LlvmEmitError> {
        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: at.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: at.into(),
            })?;

        let cont_bb = self.context.append_basic_block(func, "effect_unwind_cont");
        let is_active = self.emit_effect_is_active_i1(at)?;

        if let Some(target) = self.current_raise_target() {
            self.builder
                .build_conditional_branch(is_active, target, cont_bb)?;
        } else {
            let ret_bb = self
                .context
                .append_basic_block(func, "effect_unwind_return");
            self.builder
                .build_conditional_branch(is_active, ret_bb, cont_bb)?;

            self.builder.position_at_end(ret_bb);
            let ret_ty = self
                .current_fun_return_ty
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "effect unwind needs function return type",
                    at: at.into(),
                })?;
            let v = self.default_value(ret_ty);
            self.emit_return(at, ret_ty, v)?;
        }

        self.builder.position_at_end(cont_bb);
        Ok(())
    }

    fn effect_trace_line_col(
        &self,
        at: crate::span::Span,
    ) -> Result<(u32, u32), LlvmEmitError> {
        // 注意：当前阶段 HIR span 仍是“无 file-id 的 byte offsets”，当 codegen 生成跨文件函数体
        //（例如 stdlib/helper 被内联为可 codegen 的顶层函数）时，span 可能不属于入口 `source`。
        //
        // 为避免把“诊断辅助信息”升级成 hard error，这里选择在无法映射时降级为 (0, 0)：
        // - 不影响 non-resuming effect 的语义（仍由 flag+slot 决定）；
        // - fixtures 可选择性断言：对入口文件的 raise/perform，line/col 仍可稳定；
        // - 未来当 span 携带 file-id 后，再把这里升级为精确映射。
        let Ok((line, col)) = self.source.offset_to_line_col(at.start) else {
            return Ok((0, 0));
        };
        let line_u32 = line.min(u32::MAX as usize) as u32;
        let col_u32 = col.min(u32::MAX as usize) as u32;
        Ok((line_u32, col_u32))
    }

    /// 将 `Raise.raise(error)` 的 `error` 值编码为 runtime perform slot 的 payload words。
    ///
    /// 当前阶段（T0818）的目标是先把 `Raise<RuntimeError>` 跑通，以支持：
    /// - `x!!` / `x as T` 等“运行期失败 → Raise<RuntimeError>”的语义落点；
    /// - `try/catch` 能读回并匹配 `RuntimeError` 的 unit variants。
    ///
    /// ABI（TODO T0630）：
    /// - payload 使用 2 个 word：`(kind, value)`
    ///   - `kind`：判别信息（union 风格），便于在 handler 边界做断言/调试
    ///   - `value`：实际载荷（按 u64 编码）
    fn codegen_raise_error_payload_words(
        &mut self,
        err_expr: &hir::Expr,
    ) -> Result<(IntValue<'ctx>, IntValue<'ctx>), LlvmEmitError> {
        // slot 的 word 固定为 u64（runtime ABI，T0630）。
        let u64_ty = self.context.i64_type();
        let from_u64 = IntTy {
            bits: 64,
            signed: false,
        };

        // payload.kind（用于 union 风格判别；0 表示未初始化）。
        const KIND_INT: u64 = 1;
        const KIND_RUNTIME_ERROR: u64 = 2;

        // 注意：HIR 在早期阶段并不总是为每个表达式标注精确类型（例如 member access 常为 `Any`），
        // 因此这里以 codegen 后的 `CgValue.ty` 为准（避免过度依赖 `hir::Expr.ty`）。
        let err_v = self.codegen_expr(err_expr)?;

        match err_v.ty {
            CgTy::Int(from_ty) => {
                // 整数族：把值编码进 slot 的 u64。
                let (err_raw, _) = err_v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "Raise.raise arg value",
                    at: err_expr.span.into(),
                })?;
                let kind = u64_ty.const_int(KIND_INT, false);
                let value = self.cast_int(err_raw, from_ty, from_u64)?;
                Ok((kind, value))
            }
            CgTy::Enum(enum_ty) if self.is_sysroot_runtime_error_enum(enum_ty) => {
                // `RuntimeError`：写入 tag（u32）到 slot（u64）。
                //
                // 注意：当前 `RuntimeError` 的 enum 表示是 tagged union `{ tag: i32, payload: word }`，
                // 其中 payload 为空（unit variants），因此只需要写回 tag 即可。
                let repr = self.cg_enum_layout(err_expr.span, enum_ty)?.repr;
                if !matches!(repr, CgEnumRepr::TaggedUnion) {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "Raise<RuntimeError> niche repr (not supported)",
                        at: err_expr.span.into(),
                    });
                }

                let raw = err_v.value.ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "Raise.raise arg value",
                    at: err_expr.span.into(),
                })?;
                let enum_v = raw.into_struct_value();
                let extracted =
                    self.builder
                        .build_extract_value(enum_v, 0, "raise_runtime_error_tag")?;
                let BasicValueEnum::IntValue(tag_i32) = extracted else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "Raise<RuntimeError> tag value",
                        at: err_expr.span.into(),
                    });
                };
                let kind = u64_ty.const_int(KIND_RUNTIME_ERROR, false);
                let value = self.builder.build_int_z_extend(
                    tag_i32,
                    u64_ty,
                    "raise_runtime_error_tag_u64",
                )?;
                Ok((kind, value))
            }
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "Raise.raise arg type (payload encoding)",
                at: err_expr.span.into(),
            }),
        }
    }

    /// 判断一个 value nominal type 是否是 sysroot 内建的 `scoop.core.RuntimeError`。
    ///
    /// 说明：T0818 只要求打通 `Raise<RuntimeError>`；其它 `Raise<E>` 的复杂 payload ABI 留给 T0630。
    fn is_sysroot_runtime_error_enum(&self, ty: TypeId) -> bool {
        matches!(
            self.types.kind(ty),
            TypeKind::Value(ValueTypeKind::Nominal(nominal))
                if nominal.fqn == "scoop.core.RuntimeError"
        )
    }

    /// codegen 一个 `Raise.raise(e)`（HIR `Perform` 的最小子集）。
    ///
    /// 当前阶段（T0614）约束：
    /// - 只支持 `scoop.core.Raise.raise`；
    /// - `e` 只支持：
    ///   - word-sized `Int`（沿用 T0614 的最小约定）；
    ///   - `RuntimeError`（T0818：写入 enum tag）；
    /// - 不支持 finally / 自定义 effect / `-> resume`。
    fn codegen_perform_expr(
        &mut self,
        span: crate::span::Span,
        op: &hir::EffectOpRef,
        args: &[hir::CallArg],
        expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if op.fqn != "scoop.core.Raise.raise" {
            return self.codegen_perform_expr_nonresuming_custom_int(span, op, args, expected);
        }

        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "Raise.raise arity mismatch",
                at: span.into(),
            });
        }
        let hir::CallArg::Positional(err_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "Raise.raise named arg",
                at: span.into(),
            });
        };

        // 1) 计算 `error` 值，并编码为 slot 的复合 payload（2 words）。
        let (payload_kind_u64, payload_value_u64) =
            self.codegen_raise_error_payload_words(err_expr)?;

        // 2) 写 slot + set flag。
        // 说明：当前阶段只需要“可观测的最小表示”；op_tag 未来会与更通用的 payload ABI 对齐（T0630）。
        const OP_TAG_RAISE: u64 = 1;
        let tag_i32 = self.context.i32_type().const_int(OP_TAG_RAISE, false);
        let rt_write = self.declare_runtime_effect_perform_slot_write_u64_2();
        let _ = self.builder.build_call(
            rt_write,
            &[
                tag_i32.into(),
                payload_kind_u64.into(),
                payload_value_u64.into(),
            ],
            "raise_write_slot",
        )?;

        let (src_line, src_col) = self.effect_trace_line_col(span)?;
        let rt_set = self.declare_runtime_effect_set_active_with_trace();
        let i32_ty = self.context.i32_type();
        let src_line_i32 = i32_ty.const_int(src_line as u64, false);
        let src_col_i32 = i32_ty.const_int(src_col as u64, false);
        let _ = self.builder.build_call(
            rt_set,
            &[src_line_i32.into(), src_col_i32.into()],
            "raise_set_active",
        )?;

        // 3) “早退”：在 handler boundary 内跳到 catch，否则返回默认值向外传播。
        if let Some(target) = self.current_raise_target() {
            self.builder.build_unconditional_branch(target)?;
        } else {
            let ret_ty = self
                .current_fun_return_ty
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "Raise.raise needs function return type",
                    at: span.into(),
                })?;
            let v = self.default_value(ret_ty);
            self.emit_return(span, ret_ty, v)?;
        }

        // 4) 继续生成后续 IR：把 builder 移到一个“不可达 continuation block”，避免后续插入失败。
        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: span.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: span.into(),
            })?;
        let dead = self.context.append_basic_block(func, "after_raise_dead");
        self.builder.position_at_end(dead);

        // Raise 的返回类型在类型系统里是 `Nothing`，可用于任意期望类型；
        // 这里返回一个“期望类型的默认值”以保持后续 codegen 可继续推进。
        Ok(match expected {
            Some(ty) => self.default_value(ty),
            None => CgValue::unit(),
        })
    }

    /// codegen 一个最小自定义 non-resuming effect `perform`（T0625）。
    ///
    /// 当前阶段约束：
    /// - 仅支持 `op(arg)` 形式，且 `arg` 必须是 word-sized `Int`；
    /// - 仅支持在同一函数内存在匹配的 `handle ... with { Effect.op(x) -> ... }` 捕获边界：
    ///   若不存在，则直接报错（避免与现有 `Raise` 的“返回默认值向外传播”机制混淆）。
    ///
    /// 语义：
    /// - 写入 runtime perform slot（1 word payload）并 set flag；
    /// - 直接跳转到最近的匹配 catch block（最近匹配：从栈顶向外找）。
    fn codegen_perform_expr_nonresuming_custom_int(
        &mut self,
        span: crate::span::Span,
        op: &hir::EffectOpRef,
        args: &[hir::CallArg],
        expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "effect op arity mismatch (custom non-resuming)",
                at: span.into(),
            });
        }
        let hir::CallArg::Positional(payload_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "effect op named arg (custom non-resuming)",
                at: span.into(),
            });
        };

        let payload_v = self.codegen_expr(payload_expr)?;
        let CgTy::Int(from_ty) = payload_v.ty else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "effect payload type (custom non-resuming, only Int supported)",
                at: payload_expr.span.into(),
            });
        };
        let (payload_raw, _) = payload_v
            .as_int()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "effect payload value (custom non-resuming)",
                at: payload_expr.span.into(),
            })?;

        // v0：自定义 effect 的 op_tag 暂不分配稳定编号（runtime 仍会记录到 slot 里便于调试）。
        let op_tag_i32 = self.context.i32_type().const_zero();
        let from_u64 = IntTy {
            bits: 64,
            signed: false,
        };
        let payload_u64 = self.cast_int(payload_raw, from_ty, from_u64)?;

        let rt_write = self.declare_runtime_effect_perform_slot_write_u64();
        let _ = self.builder.build_call(
            rt_write,
            &[op_tag_i32.into(), payload_u64.into()],
            "effect_write_slot",
        )?;
        let (src_line, src_col) = self.effect_trace_line_col(span)?;
        let rt_set = self.declare_runtime_effect_set_active_with_trace();
        let i32_ty = self.context.i32_type();
        let src_line_i32 = i32_ty.const_int(src_line as u64, false);
        let src_col_i32 = i32_ty.const_int(src_col as u64, false);
        let _ = self.builder.build_call(
            rt_set,
            &[src_line_i32.into(), src_col_i32.into()],
            "effect_set_active",
        )?;

        let Some(target) = self.current_effect_unwind_target(&op.fqn) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "effect op without handle boundary (custom non-resuming)",
                at: span.into(),
            });
        };
        self.builder.build_unconditional_branch(target)?;

        // 继续生成后续 IR：把 builder 移到一个“不可达 continuation block”，避免后续插入失败。
        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: span.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: span.into(),
            })?;
        let dead = self.context.append_basic_block(func, "after_effect_dead");
        self.builder.position_at_end(dead);

        Ok(match expected {
            Some(ty) => self.default_value(ty),
            None => CgValue::unit(),
        })
    }

    /// codegen 一个 `handle { ... } with { Raise.raise(e) -> ... }`（`try/catch` 的 lowering 产物）。
    ///
    /// 当前阶段（T0614）约束：
    /// - 只支持捕获 `scoop.core.Raise.raise`；
    /// - 只支持单个 arm（最小示例）；finally 语义由 T0615 补齐；
    /// - arm body 在“handler scope”之外生成，避免 self-capture（PLAN §6.2）。
    fn codegen_handle_expr(
        &mut self,
        span: crate::span::Span,
        handle: &hir::HandleExpr,
        expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let out_ty = expected.ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "handle needs expected type context",
            at: span.into(),
        })?;

        if handle.arms.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle arm count (only 1 supported)",
                at: span.into(),
            });
        }
        let arm = &handle.arms[0];
        if let hir::HandleArmKind::ImmediateResume { resume } = arm.kind {
            return self.codegen_handle_expr_immediate_resume(span, handle, arm, resume, out_ty);
        }
        if let hir::HandleArmKind::EscapeContinuation { continuation } = arm.kind {
            let seq = self.escape_continuation_seq;
            self.escape_continuation_seq = self.escape_continuation_seq.saturating_add(1);
            return self.codegen_handle_expr_escape_continuation(
                span,
                handle,
                arm,
                continuation,
                seq,
                out_ty,
            );
        }
        if arm.op.op.fqn != "scoop.core.Raise.raise" {
            return self
                .codegen_handle_expr_nonresuming_custom_int_payload(span, handle, arm, out_ty);
        }
        if arm.op.binders.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle binder count (only 1 supported)",
                at: arm.op.span.into(),
            });
        }
        let binder = &arm.op.binders[0];

        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: span.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: span.into(),
            })?;

        // TODO T0913：在动态层维护 handler stack（Appendix A）。
        // 当前阶段只需要：
        // - 进入 handle body 前 push；
        // - 正常结束或进入 arm/catch 前 pop（arm body 在 dispatch scope 外执行，Appendix A.4）。
        const OP_TAG_RAISE: u64 = 1;
        let handler_frame_ty = self.llvm_effect_handler_frame_type();
        let handler_frame_ptr =
            self.create_entry_alloca_raw(span, "handle_effect_frame", handler_frame_ty.into())?;

        let outer_raise_target = self.current_raise_target();

        let body_bb = self.context.append_basic_block(func, "handle_body");
        let catch_bb = self.context.append_basic_block(func, "handle_catch");

        // `finally` 语义：保证在“正常路径 / catch 返回 / catch 继续 raise 向外传播”三种情况下都执行一次。
        // - 正常路径与 catch 返回：汇合到 finally_bb 再进入 merge；
        // - catch 内发生 raise：先进入 finally_unwind_bb 执行 finally，然后向外传播 raise（不清 flag/slot）。
        let finally_bb = self.context.append_basic_block(func, "handle_finally");
        let finally_unwind_bb = self
            .context
            .append_basic_block(func, "handle_finally_unwind");
        let merge_bb = self.context.append_basic_block(func, "handle_merge");

        let result_ptr = if out_ty == CgTy::Unit {
            None
        } else {
            Some(self.create_entry_alloca(span, "handle_result", out_ty)?)
        };

        // 进入 handle body：push handler frame（动态上下文）。
        let rt_push = self.declare_runtime_effect_handler_stack_push();
        let i8_ptr_ty = self.context.i8_type().ptr_type(AddressSpace::default());
        let frame_i8 =
            self.builder
                .build_bit_cast(handler_frame_ptr, i8_ptr_ty, "handle_effect_frame_i8")?;
        let op_tag_i32 = self.context.i32_type().const_int(OP_TAG_RAISE, false);
        let _ = self.builder.build_call(
            rt_push,
            &[frame_i8.into(), op_tag_i32.into()],
            "handle_effect_push",
        )?;

        // 进入 handle：先执行 body；若发生 Raise，则通过 flag/slot unwind 到 catch_bb。
        self.builder.build_unconditional_branch(body_bb)?;

        // --- body ---
        self.builder.position_at_end(body_bb);
        self.push_raise_target(catch_bb);
        let body_v = self.codegen_block_value(&handle.body)?;
        let body_v = if out_ty == CgTy::Unit {
            CgValue::unit()
        } else {
            self.coerce_value(handle.body.span, body_v, out_ty)?
        };
        self.pop_raise_target();

        // body 正常结束：进入 finally（并保存结果值）。
        if let Some(bb) = self.builder.get_insert_block() {
            if bb.get_terminator().is_none() {
                if let Some(ptr) = result_ptr {
                    let _ = self.store_local_value(handle.body.span, ptr, out_ty, body_v)?;
                }

                // body 正常结束：pop handler frame，使 finally 处于 handler scope 之外（与现有 lowering 一致）。
                let rt_pop = self.declare_runtime_effect_handler_stack_pop();
                let i8_ptr_ty = self.context.i8_type().ptr_type(AddressSpace::default());
                let frame_i8 = self.builder.build_bit_cast(
                    handler_frame_ptr,
                    i8_ptr_ty,
                    "handle_effect_frame_i8",
                )?;
                let _ = self
                    .builder
                    .build_call(rt_pop, &[frame_i8.into()], "handle_effect_pop")?;

                self.builder.build_unconditional_branch(finally_bb)?;
            }
        }

        // --- catch ---
        self.builder.position_at_end(catch_bb);

        // 进入 handler arm：pop handler frame（Appendix A.4：arm body 在自身 handler scope 外执行）。
        let rt_pop = self.declare_runtime_effect_handler_stack_pop();
        let i8_ptr_ty = self.context.i8_type().ptr_type(AddressSpace::default());
        let frame_i8 =
            self.builder
                .build_bit_cast(handler_frame_ptr, i8_ptr_ty, "handle_effect_frame_i8")?;
        let _ = self
            .builder
            .build_call(rt_pop, &[frame_i8.into()], "handle_effect_pop")?;

        // 读取 slot（payload words）并清除 flag/slot。
        //
        // TODO T0630：目前 `Raise.raise` 统一写入 2 个 word（kind + value），这里做运行期断言，
        // 以便快速发现 lowering/codegen/runtime ABI 不一致的问题。
        let rt_len = self.declare_runtime_effect_perform_slot_read_len_words();
        let call = self
            .builder
            .build_call(rt_len, &[], "raise_read_slot_len_words")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "effect slot_read_len_words return value",
                at: span.into(),
            })?;
        let BasicValueEnum::IntValue(len_words_i32) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "effect slot_read_len_words return type",
                at: span.into(),
            });
        };

        let expected_len = self.context.i32_type().const_int(2, false);
        let len_ok = self.builder.build_int_compare(
            IntPredicate::EQ,
            len_words_i32,
            expected_len,
            "raise_slot_len_ok",
        )?;
        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: span.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: span.into(),
            })?;
        let len_ok_bb = self
            .context
            .append_basic_block(func, "raise_slot_len_ok_bb");
        let len_bad_bb = self
            .context
            .append_basic_block(func, "raise_slot_len_bad_bb");
        self.builder
            .build_conditional_branch(len_ok, len_ok_bb, len_bad_bb)?;

        self.builder.position_at_end(len_bad_bb);
        let exit = self.declare_libc_exit();
        let code = self.context.i32_type().const_int(3, false);
        let _ = self
            .builder
            .build_call(exit, &[code.into()], "raise_slot_len_exit")?;
        self.builder.build_unreachable()?;

        self.builder.position_at_end(len_ok_bb);

        let rt_read_at = self.declare_runtime_effect_perform_slot_read_u64_at();
        let idx0 = self.context.i32_type().const_int(0, false);
        let idx1 = self.context.i32_type().const_int(1, false);

        let kind_call =
            self.builder
                .build_call(rt_read_at, &[idx0.into()], "raise_read_slot_word0")?;
        let kind_raw =
            kind_call
                .try_as_basic_value()
                .basic()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "effect slot_read_word0 return value",
                    at: span.into(),
                })?;
        let BasicValueEnum::IntValue(kind_u64) = kind_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "effect slot_read_word0 return type",
                at: span.into(),
            });
        };

        let value_call =
            self.builder
                .build_call(rt_read_at, &[idx1.into()], "raise_read_slot_word1")?;
        let value_raw =
            value_call
                .try_as_basic_value()
                .basic()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "effect slot_read_word1 return value",
                    at: span.into(),
                })?;
        let BasicValueEnum::IntValue(value_u64) = value_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "effect slot_read_word1 return type",
                at: span.into(),
            });
        };

        let rt_clear = self.declare_runtime_effect_clear();
        let _ = self.builder.build_call(rt_clear, &[], "raise_clear")?;

        // binder scope：arm body 在 handler scope 之外执行（因此不 push raise_target）。
        self.env.push_scope();

        let binder_cg_ty = self
            .cg_ty_of(binder.ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "handle binder type",
                at: binder.span.into(),
            })?;
        let binder_value = match binder_cg_ty {
            CgTy::Int(int_ty) => {
                // kind 断言：避免把 `RuntimeError` 等误解码为整数。
                let expected = self.context.i64_type().const_int(1, false);
                let ok = self.builder.build_int_compare(
                    IntPredicate::EQ,
                    kind_u64,
                    expected,
                    "raise_kind_is_int",
                )?;
                let ok_bb = self.context.append_basic_block(func, "raise_kind_int_ok");
                let bad_bb = self.context.append_basic_block(func, "raise_kind_int_bad");
                self.builder.build_conditional_branch(ok, ok_bb, bad_bb)?;

                self.builder.position_at_end(bad_bb);
                let exit = self.declare_libc_exit();
                let code = self.context.i32_type().const_int(3, false);
                let _ = self
                    .builder
                    .build_call(exit, &[code.into()], "raise_kind_int_exit")?;
                self.builder.build_unreachable()?;

                self.builder.position_at_end(ok_bb);

                // 传统路径：`Raise<Int>` —— 直接把 slot 的 u64 解码回整数。
                let from_u64 = IntTy {
                    bits: 64,
                    signed: false,
                };
                let decoded = self.cast_int(value_u64, from_u64, int_ty)?;
                CgValue::int(decoded, int_ty)
            }
            CgTy::Enum(enum_ty) if self.is_sysroot_runtime_error_enum(enum_ty) => {
                // kind 断言：避免把整数误解码为 RuntimeError。
                let expected = self.context.i64_type().const_int(2, false);
                let ok = self.builder.build_int_compare(
                    IntPredicate::EQ,
                    kind_u64,
                    expected,
                    "raise_kind_is_runtime_error",
                )?;
                let ok_bb = self
                    .context
                    .append_basic_block(func, "raise_kind_runtime_error_ok");
                let bad_bb = self
                    .context
                    .append_basic_block(func, "raise_kind_runtime_error_bad");
                self.builder.build_conditional_branch(ok, ok_bb, bad_bb)?;

                self.builder.position_at_end(bad_bb);
                let exit = self.declare_libc_exit();
                let code = self.context.i32_type().const_int(3, false);
                let _ = self.builder.build_call(
                    exit,
                    &[code.into()],
                    "raise_kind_runtime_error_exit",
                )?;
                self.builder.build_unreachable()?;

                self.builder.position_at_end(ok_bb);

                // `Raise<RuntimeError>`：slot 里承载的是 enum tag（u64），这里把它恢复为 enum 值。
                let repr = self.cg_enum_layout(span, enum_ty)?.repr;
                if !matches!(repr, CgEnumRepr::TaggedUnion) {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "Raise<RuntimeError> niche repr (not supported)",
                        at: span.into(),
                    });
                }

                let tag_i32 = self.builder.build_int_truncate(
                    value_u64,
                    self.context.i32_type(),
                    "raise_runtime_error_tag_i32",
                )?;
                let payload_zero = self.int_type(self.enum_payload_ty()).const_int(0, false);

                let llvm_enum_ty = self.llvm_enum_value_type(span, enum_ty)?;
                let llvm_enum_ty = llvm_enum_ty.into_struct_type();
                let mut agg: AggregateValueEnum<'ctx> = llvm_enum_ty.get_undef().into();
                agg =
                    self.builder
                        .build_insert_value(agg, tag_i32, 0, "raise_runtime_error_tag")?;
                agg = self.builder.build_insert_value(
                    agg,
                    payload_zero,
                    1,
                    "raise_runtime_error_payload",
                )?;
                CgValue {
                    ty: CgTy::Enum(enum_ty),
                    value: Some(agg.as_basic_value_enum()),
                }
            }
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle binder type (Raise payload decode)",
                    at: binder.span.into(),
                });
            }
        };
        let binder_ptr = self.create_entry_alloca(binder.span, &binder.name, binder_cg_ty)?;
        let stored = self.store_local_value(binder.span, binder_ptr, binder_cg_ty, binder_value)?;
        let gc_root_slot = self.gc_root_slot_for(binder.id);
        if let Some(slot_ptr) = gc_root_slot {
            self.store_gc_root_slot_value(binder.span, slot_ptr, stored)?;
        }
        self.env.insert(
            binder.id,
            CgLocal {
                hir_ty: Some(binder.ty),
                ty: binder_cg_ty,
                ptr: binder_ptr,
                mutable: false,
                gc_root_slot,
            },
        );

        // catch body 若再次发生 Raise：先执行 finally，再向外传播（不在本 handler 内消费 slot）。
        self.push_raise_target(finally_unwind_bb);
        let arm_v = self.codegen_expr_in_expected_context(&arm.body, Some(out_ty))?;
        self.pop_raise_target();
        let arm_v = if out_ty == CgTy::Unit {
            CgValue::unit()
        } else {
            self.coerce_value(arm.body.span, arm_v, out_ty)?
        };

        let catch_end =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: span.into(),
                })?;
        let catch_reaches_merge = catch_end.get_terminator().is_none();
        if catch_reaches_merge {
            if let Some(ptr) = result_ptr {
                let _ = self.store_local_value(arm.body.span, ptr, out_ty, arm_v)?;
            }
            self.builder.build_unconditional_branch(finally_bb)?;
        }
        self.env.pop_scope();

        // --- finally_unwind ---
        self.builder.position_at_end(finally_unwind_bb);
        if let Some(finally) = handle.finally.as_ref() {
            let _ = self.codegen_block_value(finally)?;
        }
        if let Some(bb) = self.builder.get_insert_block() {
            if bb.get_terminator().is_none() {
                if let Some(target) = outer_raise_target {
                    self.builder.build_unconditional_branch(target)?;
                } else {
                    let ret_ty =
                        self.current_fun_return_ty
                            .ok_or(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle finally unwind needs function return type",
                                at: span.into(),
                            })?;
                    let v = self.default_value(ret_ty);
                    self.emit_return(span, ret_ty, v)?;
                }
            }
        }

        // --- finally ---
        self.builder.position_at_end(finally_bb);
        if let Some(finally) = handle.finally.as_ref() {
            let _ = self.codegen_block_value(finally)?;
        }
        if let Some(bb) = self.builder.get_insert_block() {
            if bb.get_terminator().is_none() {
                self.builder.build_unconditional_branch(merge_bb)?;
            }
        }

        // --- merge ---
        self.builder.position_at_end(merge_bb);

        match out_ty {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Bool | CgTy::Int(_) | CgTy::String | CgTy::Ref => {
                let Some(ptr) = result_ptr else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle result slot",
                        at: span.into(),
                    });
                };

                let llvm_ty = self.llvm_basic_type_of(span, out_ty)?;
                let loaded = self.builder.build_load(llvm_ty, ptr, "handle_result")?;
                Ok(CgValue {
                    ty: out_ty,
                    value: Some(loaded),
                })
            }
            CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
                Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle result type",
                    at: span.into(),
                })
            }
        }
    }

    /// codegen 一个最小自定义 non-resuming effect 的 `handle`（T0625）。
    ///
    /// 当前阶段约束：
    /// - 仅支持单 arm；
    /// - binder 仅支持 1 个且类型为 `Int`；
    /// - payload ABI：`perform` 往 slot 写 1 个 word（u64），catch 读取并清 flag/slot。
    ///
    /// 关键语义（Appendix A.4）：
    /// - handler arm body 在自身 dispatch scope 外执行：因此 arm codegen 期间不在
    ///   `effect_unwind_target_stack` 中保留 `catch_bb` 入口；
    /// - 但为了确保 `finally` 语义（若有）仍然成立，arm body 内若再次 perform 同一 op，
    ///   会先跳到 `finally_unwind_bb` 执行 finally，再向外层 handler 传播。
    fn codegen_handle_expr_nonresuming_custom_int_payload(
        &mut self,
        span: crate::span::Span,
        handle: &hir::HandleExpr,
        arm: &hir::HandleArm,
        out_ty: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if arm.op.binders.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle binder count (custom non-resuming, only 1 supported)",
                at: arm.op.span.into(),
            });
        }
        let binder = &arm.op.binders[0];

        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: span.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: span.into(),
            })?;

        // v0：自定义 effect 的 op_tag 暂用 0（与现有 resume/escape 代码保持一致）。
        let op_tag_i32 = self.context.i32_type().const_zero();

        let outer_target = self.current_effect_unwind_target(&arm.op.op.fqn);

        let body_bb = self.context.append_basic_block(func, "handle_custom_body");
        let catch_bb = self.context.append_basic_block(func, "handle_custom_catch");

        // `finally` 语义：保证在“正常路径 / catch 返回 / catch 继续 perform 向外传播”三种情况下都执行一次。
        let finally_bb = self
            .context
            .append_basic_block(func, "handle_custom_finally");
        let finally_unwind_bb = self
            .context
            .append_basic_block(func, "handle_custom_finally_unwind");
        let merge_bb = self.context.append_basic_block(func, "handle_custom_merge");

        let result_ptr = if out_ty == CgTy::Unit {
            None
        } else {
            Some(self.create_entry_alloca(span, "handle_custom_result", out_ty)?)
        };

        // handler frame（动态上下文）。
        let handler_frame_ty = self.llvm_effect_handler_frame_type();
        let handler_frame_ptr = self.create_entry_alloca_raw(
            span,
            "handle_custom_effect_frame",
            handler_frame_ty.into(),
        )?;

        // 进入 handle body：push handler frame（动态上下文）。
        let rt_push = self.declare_runtime_effect_handler_stack_push();
        let i8_ptr_ty = self.context.i8_type().ptr_type(AddressSpace::default());
        let frame_i8 =
            self.builder
                .build_bit_cast(handler_frame_ptr, i8_ptr_ty, "handle_custom_frame_i8")?;
        let _ = self.builder.build_call(
            rt_push,
            &[frame_i8.into(), op_tag_i32.into()],
            "handle_custom_effect_push",
        )?;

        // 进入 handle：先执行 body；若发生 perform，则跳到 catch_bb。
        self.builder.build_unconditional_branch(body_bb)?;

        // --- body ---
        self.builder.position_at_end(body_bb);
        self.push_effect_unwind_target(&arm.op.op.fqn, catch_bb);
        let body_v = self.codegen_block_value_in_expected_context(&handle.body, Some(out_ty))?;
        self.pop_effect_unwind_target();

        let body_v = if out_ty == CgTy::Unit {
            CgValue::unit()
        } else {
            self.coerce_value(handle.body.span, body_v, out_ty)?
        };

        // body 正常结束：进入 finally（并保存结果值）。
        if let Some(bb) = self.builder.get_insert_block() {
            if bb.get_terminator().is_none() {
                if let Some(ptr) = result_ptr {
                    let _ = self.store_local_value(handle.body.span, ptr, out_ty, body_v)?;
                }

                // body 正常结束：pop handler frame，使 finally 处于 handler scope 之外（Appendix A.4）。
                let rt_pop = self.declare_runtime_effect_handler_stack_pop();
                let i8_ptr_ty = self.context.i8_type().ptr_type(AddressSpace::default());
                let frame_i8 = self.builder.build_bit_cast(
                    handler_frame_ptr,
                    i8_ptr_ty,
                    "handle_custom_frame_i8",
                )?;
                let _ = self.builder.build_call(
                    rt_pop,
                    &[frame_i8.into()],
                    "handle_custom_effect_pop",
                )?;

                self.builder.build_unconditional_branch(finally_bb)?;
            }
        }

        // --- catch ---
        self.builder.position_at_end(catch_bb);

        // 进入 handler arm：pop handler frame（Appendix A.4：arm body 在自身 handler scope 外执行）。
        let rt_pop = self.declare_runtime_effect_handler_stack_pop();
        let i8_ptr_ty = self.context.i8_type().ptr_type(AddressSpace::default());
        let frame_i8 =
            self.builder
                .build_bit_cast(handler_frame_ptr, i8_ptr_ty, "handle_custom_frame_i8")?;
        let _ = self
            .builder
            .build_call(rt_pop, &[frame_i8.into()], "handle_custom_effect_pop")?;

        // 读取 slot（1 word payload）并清除 flag/slot。
        let rt_len = self.declare_runtime_effect_perform_slot_read_len_words();
        let call = self
            .builder
            .build_call(rt_len, &[], "custom_read_slot_len_words")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "effect slot_read_len_words return value",
                at: span.into(),
            })?;
        let BasicValueEnum::IntValue(len_words_i32) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "effect slot_read_len_words return type",
                at: span.into(),
            });
        };

        let expected_len = self.context.i32_type().const_int(1, false);
        let len_ok = self.builder.build_int_compare(
            IntPredicate::EQ,
            len_words_i32,
            expected_len,
            "custom_slot_len_ok",
        )?;
        let len_ok_bb = self
            .context
            .append_basic_block(func, "custom_slot_len_ok_bb");
        let len_bad_bb = self
            .context
            .append_basic_block(func, "custom_slot_len_bad_bb");
        self.builder
            .build_conditional_branch(len_ok, len_ok_bb, len_bad_bb)?;

        self.builder.position_at_end(len_bad_bb);
        self.emit_exit_with_code(span, 3)?;

        self.builder.position_at_end(len_ok_bb);

        let rt_read = self.declare_runtime_effect_perform_slot_read_u64();
        let value_call = self
            .builder
            .build_call(rt_read, &[], "custom_read_slot_word0")?;
        let value_raw =
            value_call
                .try_as_basic_value()
                .basic()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "effect slot_read_word0 return value",
                    at: span.into(),
                })?;
        let BasicValueEnum::IntValue(value_u64) = value_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "effect slot_read_word0 return type",
                at: span.into(),
            });
        };

        let rt_clear = self.declare_runtime_effect_clear();
        let _ = self.builder.build_call(rt_clear, &[], "custom_clear")?;

        // binder scope：arm body 在 handler scope 之外执行（因此不 push effect_unwind_target_stack 的 catch_bb）。
        self.env.push_scope();

        let binder_cg_ty = self
            .cg_ty_of(binder.ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "handle binder type (custom non-resuming)",
                at: binder.span.into(),
            })?;
        let CgTy::Int(int_ty) = binder_cg_ty else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle binder type (custom non-resuming, only Int supported)",
                at: binder.span.into(),
            });
        };

        let from_u64 = IntTy {
            bits: 64,
            signed: false,
        };
        let decoded = self.cast_int(value_u64, from_u64, int_ty)?;
        let binder_value = CgValue::int(decoded, int_ty);

        let binder_ptr = self.create_entry_alloca(binder.span, &binder.name, binder_cg_ty)?;
        let stored = self.store_local_value(binder.span, binder_ptr, binder_cg_ty, binder_value)?;
        let gc_root_slot = self.gc_root_slot_for(binder.id);
        if let Some(slot_ptr) = gc_root_slot {
            self.store_gc_root_slot_value(binder.span, slot_ptr, stored)?;
        }
        self.env.insert(
            binder.id,
            CgLocal {
                hir_ty: Some(binder.ty),
                ty: binder_cg_ty,
                ptr: binder_ptr,
                mutable: false,
                gc_root_slot,
            },
        );

        // catch body 若再次发生 perform：先执行 finally，再向外传播（不在本 handler 内消费 slot）。
        self.push_effect_unwind_target(&arm.op.op.fqn, finally_unwind_bb);
        let arm_v = self.codegen_expr_in_expected_context(&arm.body, Some(out_ty))?;
        self.pop_effect_unwind_target();
        let arm_v = if out_ty == CgTy::Unit {
            CgValue::unit()
        } else {
            self.coerce_value(arm.body.span, arm_v, out_ty)?
        };

        let catch_end =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: span.into(),
                })?;
        let catch_reaches_merge = catch_end.get_terminator().is_none();
        if catch_reaches_merge {
            if let Some(ptr) = result_ptr {
                let _ = self.store_local_value(arm.body.span, ptr, out_ty, arm_v)?;
            }
            self.builder.build_unconditional_branch(finally_bb)?;
        }
        self.env.pop_scope();

        // --- finally_unwind ---
        self.builder.position_at_end(finally_unwind_bb);
        if let Some(finally) = handle.finally.as_ref() {
            let _ = self.codegen_block_value(finally)?;
        }
        if let Some(bb) = self.builder.get_insert_block() {
            if bb.get_terminator().is_none() {
                if let Some(target) = outer_target {
                    self.builder.build_unconditional_branch(target)?;
                } else {
                    // 当前阶段：自定义 effect 在程序边界的处理策略尚未固定；先按运行期错误处理。
                    self.emit_exit_with_code(span, 3)?;
                }
            }
        }

        // --- finally ---
        self.builder.position_at_end(finally_bb);
        if let Some(finally) = handle.finally.as_ref() {
            let _ = self.codegen_block_value(finally)?;
        }
        if let Some(bb) = self.builder.get_insert_block() {
            if bb.get_terminator().is_none() {
                self.builder.build_unconditional_branch(merge_bb)?;
            }
        }

        // --- merge ---
        self.builder.position_at_end(merge_bb);

        match out_ty {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Bool | CgTy::Int(_) | CgTy::String | CgTy::Ref => {
                let Some(ptr) = result_ptr else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle result slot",
                        at: span.into(),
                    });
                };

                let llvm_ty = self.llvm_basic_type_of(span, out_ty)?;
                let loaded = self
                    .builder
                    .build_load(llvm_ty, ptr, "handle_custom_result")?;
                Ok(CgValue {
                    ty: out_ty,
                    value: Some(loaded),
                })
            }
            CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
                Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle result type",
                    at: span.into(),
                })
            }
        }
    }

    fn codegen_handle_expr_immediate_resume(
        &mut self,
        span: crate::span::Span,
        handle: &hir::HandleExpr,
        arm: &hir::HandleArm,
        resume_symbol: hir::SymbolId,
        out_ty: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        // T0616：先实现最小“栈 state machine”版本的 `-> resume`：
        // - 只支持单个 perform 点（位于一个 `val x: T = Effect.op(...)` 的 init 中）
        // - `resume(value)` 必须恰好一次：重复/缺失先按运行期错误处理（exit(3)）
        if handle.finally.is_some() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle finally (immediate-resume)",
                at: span.into(),
            });
        }

        // 1) 在 handle body 中找到唯一的 perform 点（当前阶段只支持 `val x: T = perform` 这种形式）。
        let mut perform_site: Option<(usize, &hir::ValDecl, &hir::EffectOpRef, &[hir::CallArg])> =
            None;
        for (idx, stmt) in handle.body.stmts.iter().enumerate() {
            match &stmt.kind {
                hir::StmtKind::Val(decl) => {
                    let Some(init) = decl.init.as_ref() else {
                        continue;
                    };
                    if let hir::ExprKind::Perform { op, args } = &init.kind {
                        if perform_site.is_some() {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle resume body (multiple perform points)",
                                at: init.span.into(),
                            });
                        }
                        perform_site = Some((idx, decl, op, args.as_slice()));
                    }
                }
                hir::StmtKind::Expr(expr) => {
                    if matches!(expr.kind, hir::ExprKind::Perform { .. }) {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle resume body (perform must be bound to val)",
                            at: expr.span.into(),
                        });
                    }
                }
                _ => {}
            }
        }

        let Some((perform_idx, perform_decl, perform_op, perform_args)) = perform_site else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle resume body (missing perform)",
                at: span.into(),
            });
        };

        if perform_op.fqn != arm.op.op.fqn {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle resume op mismatch",
                at: perform_op.span.into(),
            });
        }

        let Some(perform_id) = perform_decl.id else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle resume perform binding id",
                at: perform_decl.span.into(),
            });
        };

        let resume_value_ty =
            self.cg_ty_of(perform_decl.ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle resume perform value type",
                    at: perform_decl.span.into(),
                })?;

        if arm.op.binders.len() != perform_args.len() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle resume binder arity mismatch",
                at: arm.op.span.into(),
            });
        }

        // 2) 创建 state machine 所需的基本块与栈上存储。
        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: span.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: span.into(),
            })?;

        // TODO T0913：在动态层维护 handler stack（Appendix A）。
        //
        // - handle 进入后 push handler frame；
        // - arm body 执行期间将其标记为 inactive（Appendix A.4），避免 self-capture；
        // - 进入 resumed computation（dispatch/state1...）前再恢复为 active；
        // - handle 结束时 pop。
        let handler_frame_ty = self.llvm_effect_handler_frame_type();
        let handler_frame_ptr = self.create_entry_alloca_raw(
            span,
            "handle_resume_effect_frame",
            handler_frame_ty.into(),
        )?;

        let dispatch_bb = self
            .context
            .append_basic_block(func, "handle_resume_dispatch");
        let state0_bb = self
            .context
            .append_basic_block(func, "handle_resume_state0");
        let state1_bb = self
            .context
            .append_basic_block(func, "handle_resume_state1");
        let arm_bb = self.context.append_basic_block(func, "handle_resume_arm");
        let done_bb = self.context.append_basic_block(func, "handle_resume_done");
        let bad_state_bb = self
            .context
            .append_basic_block(func, "handle_resume_bad_state");

        let i32_ty = self.context.i32_type();
        let state_ptr = self.create_entry_alloca_raw(span, "handle_state", i32_ty.into())?;
        let resume_used_ptr = self.create_entry_alloca_raw(
            span,
            "handle_resume_used",
            self.context.bool_type().into(),
        )?;
        let resume_value_ptr = if resume_value_ty == CgTy::Unit {
            None
        } else {
            Some(self.create_entry_alloca(span, "handle_resume_value", resume_value_ty)?)
        };

        let result_ptr = if out_ty == CgTy::Unit {
            None
        } else {
            Some(self.create_entry_alloca(span, "handle_result", out_ty)?)
        };

        // binder locals：提前在 entry block 分配 slot；在 perform 点写入，在 arm body 内读取。
        struct BinderSlot<'ctx> {
            id: hir::SymbolId,
            hir_ty: TypeId,
            ty: CgTy,
            ptr: PointerValue<'ctx>,
            gc_root_slot: Option<PointerValue<'ctx>>,
        }
        let mut binder_slots: Vec<BinderSlot<'ctx>> = Vec::new();
        for binder in &arm.op.binders {
            let binder_ty = self
                .cg_ty_of(binder.ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle resume binder type",
                    at: binder.span.into(),
                })?;
            let ptr = self.create_entry_alloca(binder.span, &binder.name, binder_ty)?;
            binder_slots.push(BinderSlot {
                id: binder.id,
                hir_ty: binder.ty,
                ty: binder_ty,
                ptr,
                gc_root_slot: self.gc_root_slot_for(binder.id),
            });
        }

        // 3) 初始化并进入 dispatch。
        let _ = self.builder.build_store(state_ptr, i32_ty.const_zero())?;
        let _ = self.builder.build_store(
            resume_used_ptr,
            self.context.bool_type().const_int(0, false),
        )?;

        // push handler frame（动态上下文）。
        //
        // 说明：op_tag 目前仅对 `Raise.raise` 固化为 1；其它 op 先写 0（未来由统一的 op_tag 分配规则补齐）。
        let rt_push = self.declare_runtime_effect_handler_stack_push();
        let i8_ptr_ty = self.context.i8_type().ptr_type(AddressSpace::default());
        let frame_i8 = self.builder.build_bit_cast(
            handler_frame_ptr,
            i8_ptr_ty,
            "handle_resume_effect_frame_i8",
        )?;
        let op_tag_i32 = if arm.op.op.fqn == "scoop.core.Raise.raise" {
            self.context.i32_type().const_int(1, false)
        } else {
            self.context.i32_type().const_zero()
        };
        let _ = self.builder.build_call(
            rt_push,
            &[frame_i8.into(), op_tag_i32.into()],
            "handle_resume_effect_push",
        )?;

        self.builder.build_unconditional_branch(dispatch_bb)?;

        // --- dispatch ---
        self.builder.position_at_end(dispatch_bb);
        let state = self
            .builder
            .build_load(i32_ty, state_ptr, "handle_state")?
            .into_int_value();
        let cases = [
            (i32_ty.const_int(0, false), state0_bb),
            (i32_ty.const_int(1, false), state1_bb),
        ];
        self.builder.build_switch(state, bad_state_bb, &cases)?;

        // --- bad_state ---
        self.builder.position_at_end(bad_state_bb);
        self.emit_exit_with_code(span, 3)?;

        // `handle` body 的 locals 在整个 state machine 生命周期内有效（因此这里不使用 `codegen_block_value`）。
        self.env.push_scope();

        // --- state0：执行 perform 之前的片段，遇到 perform 则进入 arm ---
        self.builder.position_at_end(state0_bb);
        for (idx, stmt) in handle.body.stmts.iter().enumerate() {
            if idx == perform_idx {
                break;
            }
            match &stmt.kind {
                hir::StmtKind::Empty => {}
                hir::StmtKind::Val(decl) => self.codegen_val_decl(decl)?,
                hir::StmtKind::Assign { lhs, eq_span, rhs } => {
                    self.codegen_assign_stmt(*eq_span, lhs, rhs)?;
                }
                hir::StmtKind::Expr(expr) => {
                    let _ = self.codegen_expr(expr)?;
                }
                hir::StmtKind::Return { .. } => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "`return` inside handle resume body",
                        at: stmt.span.into(),
                    });
                }
                hir::StmtKind::While { .. }
                | hir::StmtKind::Break { .. }
                | hir::StmtKind::Continue { .. }
                | hir::StmtKind::Todo(_) => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "statement inside handle resume body",
                        at: stmt.span.into(),
                    });
                }
            }
        }

        // perform 语句本身：当前阶段仅支持 `val x: T = Effect.op(args...)`。
        let target_ptr = {
            let name = perform_decl.name.as_deref().unwrap_or("perform_value");
            let ptr = self.create_entry_alloca(perform_decl.span, name, resume_value_ty)?;

            let gc_root_slot = self.gc_root_slot_for(perform_id);
            self.env.insert(
                perform_id,
                CgLocal {
                    hir_ty: Some(perform_decl.ty),
                    ty: resume_value_ty,
                    ptr,
                    mutable: perform_decl.mutable,
                    gc_root_slot,
                },
            );
            ptr
        };

        // 写入 binder values（供 arm body 使用）。
        for (idx, arg) in perform_args.iter().enumerate() {
            let hir::CallArg::Positional(expr) = arg else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle resume perform args (named arg not supported)",
                    at: span.into(),
                });
            };
            let slot = &binder_slots[idx];
            if slot.ty == CgTy::Unit {
                continue;
            }

            let v = self.codegen_expr_in_expected_context(expr, Some(slot.ty))?;
            let v = self.coerce_value(expr.span, v, slot.ty)?;
            let stored = self.store_local_value(expr.span, slot.ptr, slot.ty, v)?;
            if let Some(gc_root_slot) = slot.gc_root_slot {
                self.store_gc_root_slot_value(expr.span, gc_root_slot, stored)?;
            }
        }

        // 重置一次性标记，并进入 handler arm。
        let _ = self.builder.build_store(
            resume_used_ptr,
            self.context.bool_type().const_int(0, false),
        )?;
        self.builder.build_unconditional_branch(arm_bb)?;

        // --- arm：执行 handler 片段，必须调用 `resume(value)` 跳回 dispatch ---
        self.builder.position_at_end(arm_bb);

        // Appendix A.4：arm body 在自身 handler 的 dispatch scope 外执行（避免 self-capture）。
        let rt_set_active = self.declare_runtime_effect_handler_stack_set_active();
        let i8_ptr_ty = self.context.i8_type().ptr_type(AddressSpace::default());
        let frame_i8 = self.builder.build_bit_cast(
            handler_frame_ptr,
            i8_ptr_ty,
            "handle_resume_effect_frame_i8",
        )?;
        let inactive = self.context.i32_type().const_zero();
        let _ = self.builder.build_call(
            rt_set_active,
            &[frame_i8.into(), inactive.into()],
            "handle_resume_effect_inactive",
        )?;

        self.env.push_scope();
        for slot in &binder_slots {
            self.env.insert(
                slot.id,
                CgLocal {
                    hir_ty: Some(slot.hir_ty),
                    ty: slot.ty,
                    ptr: slot.ptr,
                    mutable: false,
                    gc_root_slot: slot.gc_root_slot,
                },
            );
        }

        let resume_ctx = ImmediateResumeCtx {
            resume_symbol,
            resume_value_ty,
            resume_value_ptr,
            resume_used_ptr,
            state_ptr,
            next_state: 1,
        };
        self.push_immediate_resume_ctx(resume_ctx);
        let _ = self.codegen_expr_in_expected_context(&arm.body, Some(CgTy::Unit))?;
        self.pop_immediate_resume_ctx();

        // `resume(value)` 必须恰好一次：
        // - 未调用：arm 结束时检测到 `resume_used == false`，运行期退出；
        // - 多次调用：在 `resume(value)` intrinsic 内部检测到 `resume_used == true`，运行期退出。
        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: span.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: span.into(),
            })?;
        let resume_ok_bb = self
            .context
            .append_basic_block(func, "handle_resume_arm_ok");
        let resume_missing_bb = self
            .context
            .append_basic_block(func, "handle_resume_arm_missing");

        let used = self
            .builder
            .build_load(self.context.bool_type(), resume_used_ptr, "resume_used")?
            .into_int_value();
        self.builder
            .build_conditional_branch(used, resume_ok_bb, resume_missing_bb)?;

        self.builder.position_at_end(resume_missing_bb);
        self.emit_exit_with_code(span, 3)?;

        self.builder.position_at_end(resume_ok_bb);

        // 恢复 handler 为 active：后续 resumed computation（dispatch/state1）应处于该 handler 的动态 scope 下。
        let rt_set_active = self.declare_runtime_effect_handler_stack_set_active();
        let i8_ptr_ty = self.context.i8_type().ptr_type(AddressSpace::default());
        let frame_i8 = self.builder.build_bit_cast(
            handler_frame_ptr,
            i8_ptr_ty,
            "handle_resume_effect_frame_i8",
        )?;
        let active = self.context.i32_type().const_int(1, false);
        let _ = self.builder.build_call(
            rt_set_active,
            &[frame_i8.into(), active.into()],
            "handle_resume_effect_active",
        )?;

        self.builder.build_unconditional_branch(dispatch_bb)?;

        self.env.pop_scope();

        // --- state1：恢复 perform 的返回值，并继续执行剩余片段，计算 handle 的结果 ---
        self.builder.position_at_end(state1_bb);

        if let Some(ptr) = resume_value_ptr {
            let llvm_ty = self.llvm_basic_type_of(span, resume_value_ty)?;
            let loaded = self.builder.build_load(llvm_ty, ptr, "resume_value")?;
            let v = CgValue {
                ty: resume_value_ty,
                value: Some(loaded),
            };
            let stored = self.store_local_value(span, target_ptr, resume_value_ty, v)?;

            // 若该 binding 是 GC root，则在写回后同步 shadow stack slot。
            if let Some(local) = self.env.get(perform_id) {
                if let Some(slot_ptr) = local.gc_root_slot {
                    self.store_gc_root_slot_value(span, slot_ptr, stored)?;
                }
            }
        }

        let mut value: CgValue<'ctx> = CgValue::unit();
        for (idx, stmt) in handle.body.stmts.iter().enumerate() {
            if idx <= perform_idx {
                continue;
            }
            let is_last = idx + 1 == handle.body.stmts.len();
            match &stmt.kind {
                hir::StmtKind::Empty => {}
                hir::StmtKind::Val(decl) => {
                    self.codegen_val_decl(decl)?;
                    value = CgValue::unit();
                }
                hir::StmtKind::Assign { lhs, eq_span, rhs } => {
                    self.codegen_assign_stmt(*eq_span, lhs, rhs)?;
                    value = CgValue::unit();
                }
                hir::StmtKind::Expr(expr) => {
                    let v = self.codegen_expr(expr)?;
                    value = if is_last { v } else { CgValue::unit() };
                }
                hir::StmtKind::Return { .. } => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "`return` inside handle resume body",
                        at: stmt.span.into(),
                    });
                }
                hir::StmtKind::While { .. }
                | hir::StmtKind::Break { .. }
                | hir::StmtKind::Continue { .. }
                | hir::StmtKind::Todo(_) => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "statement inside handle resume body",
                        at: stmt.span.into(),
                    });
                }
            }
        }

        let value = if out_ty == CgTy::Unit {
            CgValue::unit()
        } else {
            self.coerce_value(handle.body.span, value, out_ty)?
        };
        if let Some(ptr) = result_ptr {
            let _ = self.store_local_value(handle.body.span, ptr, out_ty, value)?;
        }
        self.builder.build_unconditional_branch(done_bb)?;

        // --- done：读取并返回结果 ---
        self.builder.position_at_end(done_bb);

        // handle 结束：pop handler frame（动态上下文）。
        let rt_pop = self.declare_runtime_effect_handler_stack_pop();
        let i8_ptr_ty = self.context.i8_type().ptr_type(AddressSpace::default());
        let frame_i8 = self.builder.build_bit_cast(
            handler_frame_ptr,
            i8_ptr_ty,
            "handle_resume_effect_frame_i8",
        )?;
        let _ = self
            .builder
            .build_call(rt_pop, &[frame_i8.into()], "handle_resume_effect_pop")?;

        self.env.pop_scope();

        Ok(match out_ty {
            CgTy::Unit => CgValue::unit(),
            CgTy::Bool | CgTy::Int(_) | CgTy::String | CgTy::Ref => {
                let Some(ptr) = result_ptr else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle result slot",
                        at: span.into(),
                    });
                };
                let llvm_ty = self.llvm_basic_type_of(span, out_ty)?;
                let loaded = self.builder.build_load(llvm_ty, ptr, "handle_result")?;
                CgValue {
                    ty: out_ty,
                    value: Some(loaded),
                }
            }
            CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle result type",
                    at: span.into(),
                });
            }
        })
    }

    fn codegen_handle_expr_escape_continuation(
        &mut self,
        span: crate::span::Span,
        handle: &hir::HandleExpr,
        arm: &hir::HandleArm,
        continuation_symbol: hir::SymbolId,
        seq: u32,
        out_ty: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        // T0617：`Effect.op(...), k -> { ... }`
        //
        // 当前阶段（最小可回归落点）：
        // - 仅支持单个 arm（在外层已校验）；
        // - handle body 仅支持“单个 perform 点”，且要求为 block 的第一个语句；
        // - heap state machine 先只承载 handler frame，并用 step trampoline 执行 perform 之后的剩余语句；
        // - continuation one-shot 与 handler stack 捕获由 runtime（T0914/T0915a）保证。
        if handle.finally.is_some() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle finally (escape continuation)",
                at: span.into(),
            });
        }

        // 1) 在 handle body 中找到唯一的 perform 点（当前阶段只支持 `val x: T = perform` 且位于 block 首语句）。
        let mut perform_site: Option<(usize, &hir::ValDecl, &hir::EffectOpRef, &[hir::CallArg])> =
            None;
        for (idx, stmt) in handle.body.stmts.iter().enumerate() {
            match &stmt.kind {
                hir::StmtKind::Val(decl) => {
                    let Some(init) = decl.init.as_ref() else {
                        continue;
                    };
                    if let hir::ExprKind::Perform { op, args } = &init.kind {
                        if perform_site.is_some() {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "handle escape body (multiple perform points)",
                                at: init.span.into(),
                            });
                        }
                        perform_site = Some((idx, decl, op, args.as_slice()));
                    }
                }
                hir::StmtKind::Expr(expr) => {
                    if matches!(expr.kind, hir::ExprKind::Perform { .. }) {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "handle escape body (perform must be bound to val)",
                            at: expr.span.into(),
                        });
                    }
                }
                _ => {}
            }
        }

        let Some((perform_idx, perform_decl, perform_op, perform_args)) = perform_site else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle escape body (missing perform)",
                at: span.into(),
            });
        };
        if perform_idx != 0 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle escape body (perform must be first statement)",
                at: handle.body.span.into(),
            });
        }
        if perform_op.fqn != arm.op.op.fqn {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle escape op mismatch",
                at: perform_op.span.into(),
            });
        }
        if arm.op.binders.len() != perform_args.len() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle escape binder arity mismatch",
                at: arm.op.span.into(),
            });
        }

        let Some(perform_id) = perform_decl.id else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "handle escape perform binding id",
                at: perform_decl.span.into(),
            });
        };

        let resume_value_ty =
            self.cg_ty_of(perform_decl.ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle escape perform value type",
                    at: perform_decl.span.into(),
                })?;

        // 2) 生成 step trampoline：`void step(void* state, uint64_t resume_value)`
        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: span.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: span.into(),
            })?;

        let func_name = func.get_name().to_str().unwrap_or("anonymous").to_string();
        let func_name = sanitize_llvm_ident(&func_name);

        let i8_ptr_ty = self.llvm_i8_ptr_type();
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let i64_ty = self.context.i64_type();

        // escape continuation：把当前作用域内的引用类型 locals 捕获到 heap state 中，
        // 以便在 step trampoline（异步 resume）里继续访问它们。
        //
        // 注意：
        // - 当前 v0 实现捕获 `Ref/String/Bool/Int`：
        //   - `Ref/String`：用于保活 closure/env 等引用类型；
        //   - `Bool/Int`：用于保活 word-sized handle（例如 sysroot 的 `Task<T>`/`Executor` 早期落点）。
        // - 这里按“当前可见的绑定”去重（内层 scope shadow 外层），并按 SymbolId 排序保证 determinism。
        struct CapturedLocal<'ctx> {
            id: hir::SymbolId,
            local: CgLocal<'ctx>,
        }
        let mut captures: Vec<CapturedLocal<'ctx>> = Vec::new();
        let mut seen: HashSet<hir::SymbolId> = HashSet::new();
        for scope in self.env.scopes.iter().rev() {
            for (&id, &local) in scope.iter() {
                if !seen.insert(id) {
                    continue;
                }
                if matches!(
                    local.ty,
                    CgTy::Ref | CgTy::String | CgTy::Bool | CgTy::Int(_)
                ) {
                    captures.push(CapturedLocal { id, local });
                }
            }
        }
        captures.sort_by_key(|c| c.id.as_u32());

        let state_ty_name = format!("scoop.runtime.ContState__{func_name}_{seq}");
        let state_ty = if let Some(existing) = self.context.get_struct_type(&state_ty_name) {
            existing
        } else {
            let ty = self.context.opaque_struct_type(&state_ty_name);
            let header_ty = self.llvm_gc_object_header_type();
            let frame_ty = self.llvm_effect_handler_frame_type();
            let mut fields: Vec<BasicTypeEnum<'ctx>> = Vec::new();
            fields.push(header_ty.into());
            fields.push(frame_ty.into());
            for cap in &captures {
                fields.push(match cap.local.ty {
                    CgTy::Ref => gc_i8_ptr_ty.into(),
                    CgTy::String => gc_i8_ptr_ty.into(),
                    CgTy::Bool | CgTy::Int(_) => i64_ty.into(),
                    _ => unreachable!("captures filtered by type"),
                });
            }
            ty.set_body(&fields, false);
            ty
        };

        let step_name = format!("__scoop_cont_step__{func_name}_{seq}");
        let step_fn_ty = self
            .context
            .void_type()
            .fn_type(&[gc_i8_ptr_ty.into(), i64_ty.into()], false);
        let step_fn = self.module.add_function(&step_name, step_fn_ty, None);
        step_fn.set_linkage(Linkage::Internal);

        // 保存外层插入点：step 生成会重定位 builder。
        let saved_block = insert_block;

        // 生成 step 函数体：执行 perform 之后的剩余语句（state 参数当前阶段仅用于 keep-alive handler frame）。
        {
            let mut cg = MainCodegen::new(
                self.context,
                self.module,
                self.builder,
                self.target_data,
                self.host,
                self.source,
                self.types,
                self.struct_layouts,
                self.enum_layouts,
                self.top_level_vars,
                self.object_inits,
                self.class_inits,
                self.ctor_call_sites,
                self.extern_funs,
                self.fun_index,
            );

            let entry = self.context.append_basic_block(step_fn, "entry");
            cg.builder.position_at_end(entry);

            // step 为内部 trampoline：返回类型固定为 Unit。
            cg.current_fun_return_ty = Some(CgTy::Unit);

            cg.env.push_scope();

            // 恢复 captures：step 函数运行在“原函数栈已不存在”的异步时刻，
            // 因此需要从 heap state 里把所需 locals 读回到本函数的 env。
            if !captures.is_empty() {
                let state_raw = step_fn
                    .get_nth_param(0)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "continuation step state param",
                        at: span.into(),
                    })?
                    .into_pointer_value();
                let state_ptr_ty = state_ty.ptr_type(cg.gc_address_space());
                let state_ptr =
                    cg.builder
                        .build_pointer_cast(state_raw, state_ptr_ty, "cont_step_state_ptr")?;

                for (idx, cap) in captures.iter().enumerate() {
                    let field_idx = 2u32.saturating_add(idx as u32);
                    let field_ptr = cg.builder.build_struct_gep(
                        state_ty,
                        state_ptr,
                        field_idx,
                        "cont_step_capture_gep",
                    )?;
                    let name = format!("capture_{}", cap.id.as_u32());
                    match cap.local.ty {
                        CgTy::Ref => {
                            let loaded = cg
                                .builder
                                .build_load(
                                    gc_i8_ptr_ty,
                                    field_ptr,
                                    "cont_step_capture_load_ref",
                                )?
                                .into_pointer_value();
                            let ptr = cg.create_entry_alloca(span, &name, CgTy::Ref)?;
                            let _ = cg.builder.build_store(ptr, loaded)?;
                            cg.env.insert(
                                cap.id,
                                CgLocal {
                                    hir_ty: cap.local.hir_ty,
                                    ty: CgTy::Ref,
                                    ptr,
                                    mutable: cap.local.mutable,
                                    gc_root_slot: None,
                                },
                            );
                        }
                        CgTy::String => {
                            let loaded = cg
                                .builder
                                .build_load(
                                    gc_i8_ptr_ty,
                                    field_ptr,
                                    "cont_step_capture_load_str",
                                )?
                                .into_pointer_value();
                            let str_ptr_ty = cg.llvm_scoop_string_ptr_type();
                            let casted = cg.builder.build_pointer_cast(
                                loaded,
                                str_ptr_ty,
                                "cont_step_capture_str",
                            )?;
                            let ptr = cg.create_entry_alloca(span, &name, CgTy::String)?;
                            let _ = cg.builder.build_store(ptr, casted)?;
                            cg.env.insert(
                                cap.id,
                                CgLocal {
                                    hir_ty: cap.local.hir_ty,
                                    ty: CgTy::String,
                                    ptr,
                                    mutable: cap.local.mutable,
                                    gc_root_slot: None,
                                },
                            );
                        }
                        CgTy::Bool => {
                            let loaded = cg
                                .builder
                                .build_load(i64_ty, field_ptr, "cont_step_capture_load_bool")?
                                .into_int_value();
                            let zero = i64_ty.const_zero();
                            let b = cg.builder.build_int_compare(
                                IntPredicate::NE,
                                loaded,
                                zero,
                                "cont_step_capture_bool",
                            )?;
                            let ptr = cg.create_entry_alloca(span, &name, CgTy::Bool)?;
                            let _ = cg.builder.build_store(ptr, b)?;
                            cg.env.insert(
                                cap.id,
                                CgLocal {
                                    hir_ty: cap.local.hir_ty,
                                    ty: CgTy::Bool,
                                    ptr,
                                    mutable: cap.local.mutable,
                                    gc_root_slot: None,
                                },
                            );
                        }
                        CgTy::Int(int_ty) => {
                            if int_ty.bits > 64 {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "cont state capture int width > 64",
                                    at: span.into(),
                                });
                            }

                            let loaded = cg
                                .builder
                                .build_load(i64_ty, field_ptr, "cont_step_capture_load_int")?
                                .into_int_value();
                            let to = cg.int_type(int_ty);
                            let v = if int_ty.bits == 64 {
                                loaded
                            } else {
                                cg.builder
                                    .build_int_truncate(loaded, to, "cont_step_capture_trunc")?
                            };

                            let slot_ty = CgTy::Int(int_ty);
                            let ptr = cg.create_entry_alloca(span, &name, slot_ty)?;
                            let _ = cg.builder.build_store(ptr, v)?;
                            cg.env.insert(
                                cap.id,
                                CgLocal {
                                    hir_ty: cap.local.hir_ty,
                                    ty: slot_ty,
                                    ptr,
                                    mutable: cap.local.mutable,
                                    gc_root_slot: None,
                                },
                            );
                        }
                        _ => unreachable!("captures filtered by type"),
                    }
                }
            }

            // v0：只支持把 resume_value 当作一个 word-sized payload 写回到 perform binding。
            let resume_word = step_fn
                .get_nth_param(1)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "continuation step resume param",
                    at: span.into(),
                })?
                .into_int_value();

            let local_name = perform_decl.name.as_deref().unwrap_or("resume_value");
            let target_ptr = cg.create_entry_alloca(span, local_name, resume_value_ty)?;

            let resume_value = match resume_value_ty {
                CgTy::Unit => CgValue::unit(),
                CgTy::Bool => {
                    let zero = i64_ty.const_int(0, false);
                    let b = cg.builder.build_int_compare(
                        IntPredicate::NE,
                        resume_word,
                        zero,
                        "resume_bool",
                    )?;
                    CgValue::bool(b)
                }
                CgTy::Int(int_ty) => {
                    let to = cg.int_type(int_ty);
                    let v = if int_ty.bits == 64 {
                        resume_word
                    } else {
                        cg.builder
                            .build_int_truncate(resume_word, to, "resume_int")?
                    };
                    CgValue::int(v, int_ty)
                }
                CgTy::String => {
                    let ptr_ty = cg.llvm_scoop_string_ptr_type();
                    let ptr = cg
                        .builder
                        .build_int_to_ptr(resume_word, ptr_ty, "resume_str")?;
                    CgValue {
                        ty: CgTy::String,
                        value: Some(ptr.into()),
                    }
                }
                CgTy::Ref => {
                    let ptr_ty = cg.llvm_gc_i8_ptr_type();
                    let ptr = cg
                        .builder
                        .build_int_to_ptr(resume_word, ptr_ty, "resume_ref")?;
                    CgValue {
                        ty: CgTy::Ref,
                        value: Some(ptr.into()),
                    }
                }
                CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "continuation resume payload type",
                        at: perform_decl.span.into(),
                    });
                }
            };

            let stored = cg.store_local_value(span, target_ptr, resume_value_ty, resume_value)?;
            let gc_root_slot = cg.gc_root_slot_for(perform_id);
            if let Some(slot_ptr) = gc_root_slot {
                cg.store_gc_root_slot_value(span, slot_ptr, stored)?;
            }
            cg.env.insert(
                perform_id,
                CgLocal {
                    hir_ty: Some(perform_decl.ty),
                    ty: resume_value_ty,
                    ptr: target_ptr,
                    mutable: false,
                    gc_root_slot,
                },
            );

            // 执行 perform 之后的剩余语句。
            let mut _value: CgValue<'ctx> = CgValue::unit();
            for (idx, stmt) in handle.body.stmts.iter().enumerate() {
                if idx <= perform_idx {
                    continue;
                }
                match &stmt.kind {
                    hir::StmtKind::Empty => {}
                    hir::StmtKind::Val(decl) => {
                        cg.codegen_val_decl(decl)?;
                        _value = CgValue::unit();
                    }
                    hir::StmtKind::Assign { lhs, eq_span, rhs } => {
                        cg.codegen_assign_stmt(*eq_span, lhs, rhs)?;
                        _value = CgValue::unit();
                    }
                    hir::StmtKind::Expr(expr) => {
                        let _ = cg.codegen_expr(expr)?;
                        _value = CgValue::unit();
                    }
                    hir::StmtKind::Return { .. } => {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "`return` inside continuation step",
                            at: stmt.span.into(),
                        });
                    }
                    hir::StmtKind::While { .. }
                    | hir::StmtKind::Break { .. }
                    | hir::StmtKind::Continue { .. }
                    | hir::StmtKind::Todo(_) => {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "statement inside continuation step",
                            at: stmt.span.into(),
                        });
                    }
                }
            }

            cg.env.pop_scope();
            cg.builder.build_return(None)?;
        }

        // 恢复外层插入点。
        self.builder.position_at_end(saved_block);

        // 3) 生成 handle 的初始执行：push handler frame → 在 perform 点创建 continuation → 执行 arm → 返回。
        let body_bb = self.context.append_basic_block(func, "handle_escape_body");
        let arm_bb = self.context.append_basic_block(func, "handle_escape_arm");
        let done_bb = self.context.append_basic_block(func, "handle_escape_done");

        let result_ptr = if out_ty == CgTy::Unit {
            None
        } else {
            Some(self.create_entry_alloca(span, "handle_escape_result", out_ty)?)
        };

        // binder slots：在 perform 点写入，在 arm body 中读取。
        struct BinderSlot<'ctx> {
            id: hir::SymbolId,
            hir_ty: TypeId,
            ty: CgTy,
            ptr: PointerValue<'ctx>,
            gc_root_slot: Option<PointerValue<'ctx>>,
        }
        let mut binder_slots: Vec<BinderSlot<'ctx>> = Vec::new();
        for binder in &arm.op.binders {
            let binder_ty = self
                .cg_ty_of(binder.ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle escape binder type",
                    at: binder.span.into(),
                })?;
            let ptr = self.create_entry_alloca(binder.span, &binder.name, binder_ty)?;
            binder_slots.push(BinderSlot {
                id: binder.id,
                hir_ty: binder.ty,
                ty: binder_ty,
                ptr,
                gc_root_slot: self.gc_root_slot_for(binder.id),
            });
        }

        // continuation binder local：在 perform 点写入，在 arm body 中读取。
        let cont_ptr =
            self.create_entry_alloca(span, &format!("handle_escape_k_{seq}"), CgTy::Ref)?;
        let cont_root_slot = self.gc_root_slot_for(continuation_symbol);

        self.builder.build_unconditional_branch(body_bb)?;

        // --- body ---
        self.builder.position_at_end(body_bb);
        self.env.push_scope();

        // heap state：`{ header, handler_frame, captured_refs... }`
        let total_size = self.target_data.get_store_size(&state_ty);

        let rt_alloc = self.declare_runtime_alloc();
        let size_v = i64_ty.const_int(total_size, false);
        let call = self
            .builder
            .build_call(rt_alloc, &[size_v.into()], "rt_alloc_cont_state")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(state_raw) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc return type",
                at: span.into(),
            });
        };

        let state_ptr_ty = state_ty.ptr_type(self.gc_address_space());
        let state_ptr =
            self.builder
                .build_pointer_cast(state_raw, state_ptr_ty, "cont_state_ptr")?;
        let frame_ptr =
            self.builder
                .build_struct_gep(state_ty, state_ptr, 1, "cont_state_frame_gep")?;

        // 把当前作用域内的 locals 写入 heap state：用于 step trampoline 在异步 resume 时恢复 env。
        for (idx, cap) in captures.iter().enumerate() {
            let field_idx = 2u32.saturating_add(idx as u32);
            let field_ptr = self.builder.build_struct_gep(
                state_ty,
                state_ptr,
                field_idx,
                "cont_state_capture_gep",
            )?;

            match cap.local.ty {
                CgTy::Ref => {
                    let llvm_ty = self.llvm_basic_type_of(span, CgTy::Ref)?;
                    let loaded = self
                        .builder
                        .build_load(llvm_ty, cap.local.ptr, "cont_state_capture_load_ref")?;
                    let BasicValueEnum::PointerValue(ptr) = loaded else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "cont state capture value type (ref ptr)",
                            at: span.into(),
                        });
                    };
                    let casted = self.builder.build_pointer_cast(
                        ptr,
                        gc_i8_ptr_ty,
                        "cont_state_capture_ref_i8",
                    )?;
                    let _ = self.builder.build_store(field_ptr, casted)?;
                }
                CgTy::String => {
                    let llvm_ty = self.llvm_basic_type_of(span, CgTy::String)?;
                    let loaded = self
                        .builder
                        .build_load(llvm_ty, cap.local.ptr, "cont_state_capture_load_str")?;
                    let BasicValueEnum::PointerValue(ptr) = loaded else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "cont state capture value type (str ptr)",
                            at: span.into(),
                        });
                    };
                    let casted = self.builder.build_pointer_cast(
                        ptr,
                        gc_i8_ptr_ty,
                        "cont_state_capture_str_i8",
                    )?;
                    let _ = self.builder.build_store(field_ptr, casted)?;
                }
                CgTy::Bool => {
                    let loaded = self
                        .builder
                        .build_load(
                            self.llvm_basic_type_of(span, CgTy::Bool)?,
                            cap.local.ptr,
                            "cont_state_capture_load_bool",
                        )?
                        .into_int_value();
                    let extended = self
                        .builder
                        .build_int_z_extend(loaded, i64_ty, "cont_state_capture_zext_bool")?;
                    let _ = self.builder.build_store(field_ptr, extended)?;
                }
                CgTy::Int(int_ty) => {
                    if int_ty.bits > 64 {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "cont state capture int width > 64",
                            at: span.into(),
                        });
                    }

                    let llvm_ty = self.llvm_basic_type_of(span, cap.local.ty)?;
                    let loaded = self
                        .builder
                        .build_load(llvm_ty, cap.local.ptr, "cont_state_capture_load_int")?
                        .into_int_value();
                    let extended = if int_ty.bits == 64 {
                        loaded
                    } else if int_ty.signed {
                        self.builder
                            .build_int_s_extend(loaded, i64_ty, "cont_state_capture_sext_int")?
                    } else {
                        self.builder
                            .build_int_z_extend(loaded, i64_ty, "cont_state_capture_zext_int")?
                    };
                    let _ = self.builder.build_store(field_ptr, extended)?;
                }
                _ => unreachable!("captures filtered by type"),
            }
        }

        // push handler frame（动态上下文）。
        let rt_push = self.declare_runtime_effect_handler_stack_push();
        let frame_i8 =
            self.builder
                .build_address_space_cast(frame_ptr, i8_ptr_ty, "handle_escape_frame_i8")?;
        let op_tag_i32 = if arm.op.op.fqn == "scoop.core.Raise.raise" {
            self.context.i32_type().const_int(1, false)
        } else {
            self.context.i32_type().const_zero()
        };
        let _ = self.builder.build_call(
            rt_push,
            &[frame_i8.into(), op_tag_i32.into()],
            "handle_escape_effect_push",
        )?;

        // --- perform site：计算 args → 写 binder slots → 创建 continuation ---
        for (slot, arg) in binder_slots.iter().zip(perform_args.iter()) {
            let hir::CallArg::Positional(expr) = arg else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle escape named perform arg",
                    at: span.into(),
                });
            };
            let v = self.codegen_expr_in_expected_context(expr, Some(slot.ty))?;
            let stored = self.store_local_value(expr.span, slot.ptr, slot.ty, v)?;
            if let Some(slot_ptr) = slot.gc_root_slot {
                self.store_gc_root_slot_value(expr.span, slot_ptr, stored)?;
            }
        }

        let rt_cont_alloc = self.declare_runtime_continuation_alloc();
        let step_ptr = step_fn.as_global_value().as_pointer_value();
        let call = self.builder.build_call(
            rt_cont_alloc,
            &[state_raw.into(), step_ptr.into()],
            "cont_alloc",
        )?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "continuation alloc return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(k_raw) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "continuation alloc return type",
                at: span.into(),
            });
        };

        let stored = self.store_local_value(
            span,
            cont_ptr,
            CgTy::Ref,
            CgValue {
                ty: CgTy::Ref,
                value: Some(k_raw.into()),
            },
        )?;
        if let Some(slot_ptr) = cont_root_slot {
            self.store_gc_root_slot_value(span, slot_ptr, stored)?;
        }

        // 将 handler frame 从当前线程的 handler stack 顶部“摘除”（不清理 frame 字段），以便：
        // - handler arm body 在 dispatch scope 外执行（Appendix A.4）
        // - continuation 捕获的 handler stack（frame->prev 链）保持完整（spec §5.5）
        let handler_frame_ty = self.llvm_effect_handler_frame_type();
        let prev_ptr = self.builder.build_struct_gep(
            handler_frame_ty,
            frame_ptr,
            0,
            "handle_escape_prev_gep",
        )?;
        let prev_raw = self
            .builder
            .build_load(i8_ptr_ty, prev_ptr, "handle_escape_prev")?;
        let rt_swap = self.declare_runtime_effect_handler_stack_swap_top();
        let _ = self
            .builder
            .build_call(rt_swap, &[prev_raw.into()], "handle_escape_detach")?;

        // body locals 不应在 arm scope 可见：提前 pop。
        self.env.pop_scope();

        self.builder.build_unconditional_branch(arm_bb)?;

        // --- arm ---
        self.builder.position_at_end(arm_bb);
        self.env.push_scope();
        for slot in &binder_slots {
            self.env.insert(
                slot.id,
                CgLocal {
                    hir_ty: Some(slot.hir_ty),
                    ty: slot.ty,
                    ptr: slot.ptr,
                    mutable: false,
                    gc_root_slot: slot.gc_root_slot,
                },
            );
        }
        self.env.insert(
            continuation_symbol,
            CgLocal {
                hir_ty: None,
                ty: CgTy::Ref,
                ptr: cont_ptr,
                mutable: false,
                gc_root_slot: cont_root_slot,
            },
        );

        let arm_v = self.codegen_expr_in_expected_context(&arm.body, Some(out_ty))?;
        let arm_v = if out_ty == CgTy::Unit {
            CgValue::unit()
        } else {
            self.coerce_value(arm.body.span, arm_v, out_ty)?
        };
        if let Some(ptr) = result_ptr {
            let _ = self.store_local_value(arm.body.span, ptr, out_ty, arm_v)?;
        }

        self.env.pop_scope();
        self.builder.build_unconditional_branch(done_bb)?;

        // --- done ---
        self.builder.position_at_end(done_bb);

        Ok(match out_ty {
            CgTy::Unit => CgValue::unit(),
            CgTy::Bool | CgTy::Int(_) | CgTy::String | CgTy::Ref => {
                let Some(ptr) = result_ptr else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "handle escape result slot",
                        at: span.into(),
                    });
                };
                let llvm_ty = self.llvm_basic_type_of(span, out_ty)?;
                let loaded = self
                    .builder
                    .build_load(llvm_ty, ptr, "handle_escape_result")?;
                CgValue {
                    ty: out_ty,
                    value: Some(loaded),
                }
            }
            CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
                Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "handle escape result type",
                    at: span.into(),
                })?
            }
        })
    }

    fn codegen_call(
        &mut self,
        span: crate::span::Span,
        callee: &hir::Expr,
        args: &[hir::CallArg],
        expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        // T0616：`-> resume` arm body 内的 `resume(value)`（隐式注入的局部符号）。
        if let Some(ctx) = self.current_immediate_resume_ctx() {
            if let hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) = &callee.kind {
                if *id == ctx.resume_symbol {
                    return self.codegen_immediate_resume_call(span, args, expected, ctx);
                }
            }
        }

        // 0.5) 调用局部函数值（闭包/函数类型参数）：`f(args...)`。
        //
        // 说明：
        // - HIR lowering（用于 early codegen）不会在 `Expr.ty` 上提供精确类型，因此这里依赖：
        //   - codegen 阶段保存的局部绑定 `hir_ty: Option<TypeId>`；
        //   - 当它是 `RefTypeKind::Function` 时，读取其签名并生成 indirect call。
        if let hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) = &callee.kind {
            let local = self
                .env
                .get(*id)
                .ok_or_else(|| {
                    LlvmEmitError::UnsupportedMainBody {
                    kind: "unknown local value",
                    at: callee.span.into(),
                    }
                })?;

            if let Some(hir_ty) = local.hir_ty {
                if let TypeKind::Ref(RefTypeKind::Function(fun_ty)) = self.types.kind(hir_ty) {
                    return self.codegen_function_value_call(
                        span,
                        callee.span,
                        &local,
                        fun_ty,
                        args,
                    );
                }

                // T1026：`FunPtr<F>` 的直接调用：`fp(args...)`（unsafe）。
                if let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = self.types.kind(hir_ty) {
                    if nominal.fqn == "scoop.unsafe.FunPtr" {
                        let sig_ty = nominal.args.first().copied().ok_or(
                            LlvmEmitError::UnsupportedMainBody {
                                kind: "funptr signature type",
                                at: callee.span.into(),
                            },
                        )?;
                        let TypeKind::Ref(RefTypeKind::Function(fun_ty)) =
                            self.types.kind(sig_ty)
                        else {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "funptr signature kind",
                                at: callee.span.into(),
                            });
                        };

                        let CgTy::Int(int_ty) = local.ty else {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "funptr local cg type",
                                at: callee.span.into(),
                            });
                        };
                        let loaded = self
                            .builder
                            .build_load(
                                self.llvm_basic_type_of(callee.span, local.ty)?,
                                local.ptr,
                                "load_funptr",
                            )?
                            .into_int_value();

                        return self.codegen_funptr_value_call(
                            span,
                            callee.span,
                            loaded,
                            int_ty,
                            fun_ty,
                            args,
                        );
                    }
                }
            }
        }

        // 1) 普通顶层函数调用：`foo(args...)`
        if let hir::ExprKind::VarRef(hir::ValueRef::TopLevel { fqn, .. }) = &callee.kind {
            // T1026：sysroot 函数指针 intrinsics。
            //
            // 说明：
            // - sysroot 的 `scoop.unsafe.*` 函数不会出现在当前 compilation unit 的 `fun_index` 中，
            //   因此需要在 codegen 阶段以 intrinsic 形式内建 lowering；
            // - v0 阶段只支持 `invoke` 与 `UIntPtr`/`FunPtr` 间的整数转换。
            if fqn == "scoop.unsafe.invoke" {
                return self.codegen_sysroot_funptr_invoke(span, callee.span, args);
            }
            if fqn == "scoop.unsafe.funPtrToUIntPtr" {
                return self.codegen_sysroot_funptr_to_uintptr(span, callee.span, args);
            }
            if fqn == "scoop.unsafe.uintPtrToFunPtr" {
                return self.codegen_sysroot_uintptr_to_funptr(span, callee.span, args);
            }

            // TODO T0816：shadow stack 插桩回归用的 debug helper。
            if fqn == "scoop.core.__scoop_gc_debug_count_roots_current_thread" {
                if !args.is_empty() {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "gc debug count roots arity mismatch",
                        at: span.into(),
                    });
                }

                let rt = self.declare_runtime_gc_debug_count_roots_current_thread();
                let call = self.builder.build_call(rt, &[], "gc_debug_count_roots")?;
                let raw = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "gc debug count roots return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(raw_int) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "gc debug count roots return type",
                        at: span.into(),
                    });
                };

                let from = IntTy {
                    bits: 64,
                    signed: false,
                };
                let to = IntTy {
                    bits: self.host.word_bit_width(),
                    signed: true,
                };
                let casted = self.cast_int(raw_int, from, to)?;
                return Ok(CgValue::int(casted, to));
            }

            // TODO T0910：GC v0（mark-sweep，测试辅助）。
            if fqn == "scoop.core.__scoop_gc_collect" {
                if !args.is_empty() {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "gc collect arity mismatch",
                        at: span.into(),
                    });
                }

                let rt = self.declare_runtime_gc_collect();
                let _ = self.builder.build_call(rt, &[], "gc_collect")?;
                return Ok(CgValue::unit());
            }

            if fqn == "scoop.core.__scoop_gc_debug_heap_object_count" {
                if !args.is_empty() {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "gc heap object count arity mismatch",
                        at: span.into(),
                    });
                }

                let rt = self.declare_runtime_gc_debug_heap_object_count();
                let call = self
                    .builder
                    .build_call(rt, &[], "gc_debug_heap_object_count")?;
                let raw = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "gc heap object count return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(raw_int) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "gc heap object count return type",
                        at: span.into(),
                    });
                };

                let from = IntTy {
                    bits: 64,
                    signed: false,
                };
                let to = IntTy {
                    bits: self.host.word_bit_width(),
                    signed: true,
                };
                let casted = self.cast_int(raw_int, from, to)?;
                return Ok(CgValue::int(casted, to));
            }

            if fqn == "scoop.core.__scoop_gc_debug_alloc_garbage" {
                if args.len() != 1 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "gc debug alloc garbage arity mismatch",
                        at: span.into(),
                    });
                }

                let hir::CallArg::Positional(count_expr) = &args[0] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "gc debug alloc garbage named arg",
                        at: span.into(),
                    });
                };

                let value_word = IntTy {
                    bits: self.host.word_bit_width(),
                    signed: true,
                };

                let count_v =
                    self.codegen_expr_in_expected_context(count_expr, Some(CgTy::Int(value_word)))?;
                let count_v = self.coerce_value(count_expr.span, count_v, CgTy::Int(value_word))?;
                let (count_raw, count_from) =
                    count_v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "gc debug alloc garbage count value",
                        at: count_expr.span.into(),
                    })?;
                let count_to = IntTy {
                    bits: 64,
                    signed: true,
                };
                let count_i64 = self.cast_int(count_raw, count_from, count_to)?;

                let rt = self.declare_runtime_gc_debug_alloc_garbage();
                let _ =
                    self.builder
                        .build_call(rt, &[count_i64.into()], "gc_debug_alloc_garbage")?;
                return Ok(CgValue::unit());
            }

            // TODO T0613：effect runtime ABI（flag + perform slot）回归用的 sysroot debug helpers。
            if fqn.starts_with("scoop.core.__scoop_effect_") {
                return self.codegen_sysroot_effect_intrinsics(span, callee.span, fqn, args);
            }

            // T1007：`@Intrinsic fun sizeOf<T>(value: T): Int`（early stage）。
            if fqn == "scoop.core.sizeOf" {
                return self.codegen_sysroot_size_of(span, callee.span, args);
            }

            // T0822：最小 I/O（sysroot `print/println(String)`）直接映射到 runtime 符号。
            if fqn == "scoop.core.print" || fqn == "scoop.core.println" {
                return self.codegen_sysroot_print_like(span, callee.span, fqn, args);
            }
            // T1318e：std v2 io（stdin/stdout/stderr）最小平台接口：由 sysroot 表面直接映射到 runtime C 符号。
            if fqn == "scoop.io.stdoutWriteString" {
                return self.codegen_sysroot_io_write_string(
                    span,
                    callee.span,
                    "scoop_io_stdout_write_string",
                    args,
                );
            }
            if fqn == "scoop.io.stdoutWriteLine" {
                return self.codegen_sysroot_io_write_string(
                    span,
                    callee.span,
                    "scoop_io_stdout_write_line",
                    args,
                );
            }
            if fqn == "scoop.io.stderrWriteString" {
                return self.codegen_sysroot_io_write_string(
                    span,
                    callee.span,
                    "scoop_io_stderr_write_string",
                    args,
                );
            }
            if fqn == "scoop.io.stderrWriteLine" {
                return self.codegen_sysroot_io_write_string(
                    span,
                    callee.span,
                    "scoop_io_stderr_write_line",
                    args,
                );
            }
            if fqn == "scoop.io.stdinReadLine" {
                return self.codegen_sysroot_io_stdin_read_line_utf8(
                    span,
                    callee.span,
                    args,
                    expected,
                );
            }
            // T1318a：std v2 env/time（host）平台接口：由 sysroot 表面直接映射到 runtime C 符号。
            if fqn == "scoop.env.getOrNull" {
                return self.codegen_sysroot_env_get(span, args, expected);
            }
            if fqn == "scoop.time.nowUnixMillis" {
                return self.codegen_sysroot_time_now_unix_millis(span, callee.span, args);
            }
            // T1318b：std v2 fs（host）平台接口：由 sysroot 表面直接映射到 runtime C 符号。
            if fqn == "scoop.fs.readAllText" {
                return self.codegen_sysroot_fs_read_all_text_utf8(span, args, expected);
            }
            if fqn == "scoop.fs.writeAllText" {
                return self.codegen_sysroot_fs_write_all_text_utf8(span, callee.span, args);
            }
            // T1318c：std v2 process（host）平台接口：由 sysroot 表面直接映射到 runtime C 符号。
            if fqn == "scoop.process.exit" {
                return self.codegen_sysroot_process_exit(span, callee.span, args);
            }
            if fqn == "scoop.process.args" {
                return self.codegen_sysroot_process_args(span, callee.span, args);
            }
            // T1318d：std v2 path（host）平台接口：由 sysroot 表面直接映射到 runtime C 符号。
            if fqn == "scoop.path.normalize" {
                return self.codegen_sysroot_path_normalize(span, callee.span, args);
            }
            if fqn == "scoop.path.join" {
                return self.codegen_sysroot_path_join(span, callee.span, args);
            }
            if fqn == "scoop.path.basename" {
                return self.codegen_sysroot_path_basename(span, callee.span, args);
            }
            if fqn == "scoop.path.dirname" {
                return self.codegen_sysroot_path_dirname(span, callee.span, args);
            }
            // T1319b：std v3 sync（Mutex/CondVar/Once）最小平台接口：由 sysroot 表面直接映射到 runtime C 符号。
            if fqn == "scoop.sync.mutexCreate" {
                return self.codegen_sysroot_sync_mutex_create(span, callee.span, args);
            }
            if fqn == "scoop.sync.lock" {
                return self.codegen_sysroot_sync_mutex_lock(span, callee.span, args);
            }
            if fqn == "scoop.sync.unlock" {
                return self.codegen_sysroot_sync_mutex_unlock(span, callee.span, args);
            }
            if fqn == "scoop.sync.condVarCreate" {
                return self.codegen_sysroot_sync_condvar_create(span, callee.span, args);
            }
            if fqn == "scoop.sync.wait" {
                return self.codegen_sysroot_sync_condvar_wait(span, callee.span, args);
            }
            if fqn == "scoop.sync.notifyOne" {
                return self.codegen_sysroot_sync_condvar_notify_one(span, callee.span, args);
            }
            if fqn == "scoop.sync.notifyAll" {
                return self.codegen_sysroot_sync_condvar_notify_all(span, callee.span, args);
            }
            if fqn == "scoop.sync.onceCreate" {
                return self.codegen_sysroot_sync_once_create(span, callee.span, args);
            }
            if fqn == "scoop.sync.isDone" {
                return self.codegen_sysroot_sync_once_is_done(span, callee.span, args);
            }
            if fqn == "scoop.sync.run" {
                return self.codegen_sysroot_sync_once_run(span, callee.span, args);
            }
            // 注意：`destroy` 在 sysroot 侧为 overload set（Mutex/CondVar 均有 destroy），需在 codegen 侧按实参类型分派。
            if fqn == "scoop.sync.destroy" {
                return self.codegen_sysroot_sync_destroy(span, callee.span, args);
            }
            // T1319c：std v3 thread（spawn/join/sleep/yield/currentId）最小平台接口：由 sysroot 表面直接映射到 runtime C 符号。
            if fqn == "scoop.thread.threadSpawn" {
                return self.codegen_sysroot_thread_spawn(span, callee.span, args);
            }
            if fqn == "scoop.thread.join" {
                return self.codegen_sysroot_thread_join(span, callee.span, args);
            }
            if fqn == "scoop.thread.sleepMillis" {
                return self.codegen_sysroot_thread_sleep_millis(span, callee.span, args);
            }
            if fqn == "scoop.thread.yield" {
                return self.codegen_sysroot_thread_yield(span, callee.span, args);
            }
            if fqn == "scoop.thread.currentId" {
                return self.codegen_sysroot_thread_current_id(span, callee.span, args);
            }
            // T1319d：std v3 channels（unbounded mpsc）最小平台接口：由 sysroot 表面直接映射到 runtime C 符号。
            if fqn == "scoop.channels.channelCreate" {
                return self.codegen_sysroot_channels_channel_create(span, callee.span, args);
            }
            if fqn == "scoop.channels.send" {
                return self.codegen_sysroot_channels_send(span, callee.span, args);
            }
            if fqn == "scoop.channels.recv" {
                return self.codegen_sysroot_channels_recv(span, callee.span, args, expected);
            }
            if fqn == "scoop.channels.close" {
                return self.codegen_sysroot_channels_close(span, callee.span, args);
            }
            // T1319e：std v3 task/executor 最小平台接口：由 sysroot 表面直接映射到 runtime C 符号。
            if fqn == "scoop.task.executorCreate" {
                return self.codegen_sysroot_task_executor_create(span, callee.span, args);
            }
            if fqn == "scoop.task.destroy" {
                return self.codegen_sysroot_task_executor_destroy(span, callee.span, args);
            }
            if fqn == "scoop.task.debugPendingCount" {
                return self.codegen_sysroot_task_executor_debug_pending_count(span, callee.span, args);
            }
            if fqn == "scoop.task.runNext" {
                return self.codegen_sysroot_task_executor_run_next(span, callee.span, args);
            }
            if fqn == "scoop.task.runUntilIdle" {
                return self.codegen_sysroot_task_executor_run_until_idle(span, callee.span, args);
            }
            if fqn == "scoop.task.taskCreate" {
                return self.codegen_sysroot_task_create(span, callee.span, args);
            }
            if fqn == "scoop.task.taskCreateManual" {
                return self.codegen_sysroot_task_create_manual(span, callee.span, args);
            }
            if fqn == "scoop.task.state" {
                return self.codegen_sysroot_task_state(span, callee.span, args);
            }
            if fqn == "scoop.task.result" {
                return self.codegen_sysroot_task_result(span, callee.span, args);
            }
            if fqn == "scoop.task.tryStart" {
                return self.codegen_sysroot_task_try_start(span, callee.span, args);
            }
            if fqn == "scoop.task.complete" {
                return self.codegen_sysroot_task_complete(span, callee.span, args);
            }
            if fqn == "scoop.task.onComplete" {
                return self.codegen_sysroot_task_on_complete(span, callee.span, args);
            }
            // T1027：internal atomics（FFI/runtime oriented）
            //
            // 说明：
            // - sysroot 在 `scoop.unsafe` 暴露 `__atomicInt*` 一组内建函数；
            // - 第一个参数要求是可寻址变量槽（lvalue），codegen 直接对该槽生成 LLVM atomic 指令。
            if fqn.starts_with("scoop.unsafe.__atomicInt") {
                return self.codegen_sysroot_atomic_int_intrinsics(span, callee.span, fqn, args);
            }
            // T1317d：`Array`/`MutableArray` 最小运行期 primitive（len/get/set）与 array literal builder。
            //
            // 说明：
            // - `size/get/set` 作为 sysroot 声明的 extension fun 暴露（`Array<T>.size()` 等）；
            // - 数组字面量 `[...]` 在 HIR lowering 中会先被降为 `__scoop_array_builder_*` 调用序列；
            // - 这里把它们直接映射到 runtime 的 C ABI（`runtime/c/scoop_array.c`）。
            if fqn == "scoop.core.size" || fqn == "scoop.core.get" || fqn == "scoop.core.set" {
                return self.codegen_sysroot_array_intrinsics(span, callee.span, fqn, args, expected);
            }
            if fqn.starts_with("scoop.core.__scoop_array_builder_") {
                return self.codegen_sysroot_array_builder_intrinsics(span, callee.span, fqn, args);
            }
            // T0620：spawn/join（结构化并发最小模型）使用 runtime helper。
            if fqn == "scoop.core.__scoop_task_spawn_int"
                || fqn == "scoop.core.__scoop_task_join_int"
            {
                return self.codegen_sysroot_task_intrinsics(span, callee.span, fqn, args);
            }
            // T0618：跨线程 resume（创建线程并 join，避免引入调度器）。
            if fqn == "scoop.core.__scoop_thread_spawn_join_resume_u64" {
                return self.codegen_sysroot_thread_intrinsics(span, callee.span, fqn, args);
            }
            return self.codegen_top_level_fun_call(span, callee.span, fqn, args);
        }

        // 1.5) 内建 String API（early stage）：`receiver.trimIndent()`
        //
        // 说明：
        // - `trimIndent` 在语言层面是 `String` 的 `const fun`（spec §8.4）；
        // - 编译期折叠由 TODO T1216 负责；此处只负责运行期 fallback：调用 runtime 实现。
        if let hir::ExprKind::MemberAccess { receiver, member } = &callee.kind {
            // T1008：GC pin/unpin（spec §15.10）。
            if let Some(hir::MemberRef::Fun { fqn, .. }) = member.resolved.as_ref() {
                if fqn == "scoop.core.GC.pin" {
                    return self.codegen_sysroot_gc_pin(span, member.span, args, expected);
                }
                if fqn == "scoop.core.GC.unpin" {
                    return self.codegen_sysroot_gc_unpin(span, member.span, args);
                }
            }

            // spec §5.5：`k.resume(value)`（escape continuation）。
            if member.name == "resume" {
                return self.codegen_continuation_resume_call(span, receiver, args);
            }
            if member.name == "trimIndent" {
                return self.codegen_string_trim_indent(span, receiver, args);
            }
        }

        // 2) enum variant ctor：`Some(x)` 这类调用在 resolver 阶段不会 resolve，
        //    需要依赖“期望类型语境”才能决定属于哪个 enum。
        if let hir::ExprKind::UnresolvedIdent { name } = &callee.kind {
            // T1312：class ctor call —— resolver 在 call-site 写回 ctor candidates，
            // HIR v0 仍把 callee 降为 `UnresolvedIdent`，因此这里需要通过 side table 判断并执行 ctor。
            if let Some(candidates) = self.ctor_call_sites.get(&callee.span) {
                return self.codegen_class_ctor_call(span, callee.span, name, args, candidates);
            }

            let Some(CgTy::Enum(enum_ty)) = expected else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "enum variant ctor call without expected enum type",
                    at: callee.span.into(),
                });
            };
            return self.codegen_enum_variant_ctor_call(span, enum_ty, name, args);
        }

        Err(LlvmEmitError::UnsupportedMainBody {
            kind: "call callee",
            at: callee.span.into(),
        })
    }

    fn codegen_sysroot_funptr_invoke(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        // sysroot 里 `invoke` 是一个 extension fun：
        // - `FunPtr<...>.invoke(...)`
        // - HIR lowering 会把它降为：`scoop.unsafe.invoke(receiver, ...args)`
        //
        // 约束（v0）：
        // - receiver 必须是局部变量引用（需要借助 env.local.hir_ty 取回 `FunPtr<F>` 的精确签名）。
        let Some((receiver_arg, call_args)) = args.split_first() else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "funptr invoke arity mismatch",
                at: span.into(),
            });
        };

        let hir::CallArg::Positional(receiver_expr) = receiver_arg else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "funptr invoke receiver (named arg)",
                at: span.into(),
            });
        };

        let hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) = &receiver_expr.kind else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "funptr invoke receiver (non-local)",
                at: receiver_expr.span.into(),
            });
        };

        let local = self
            .env
            .get(*id)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "unknown local funptr receiver",
                at: receiver_expr.span.into(),
            })?;

        let Some(hir_ty) = local.hir_ty else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "funptr invoke receiver type",
                at: receiver_expr.span.into(),
            });
        };

        let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = self.types.kind(hir_ty) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "funptr invoke receiver kind",
                at: receiver_expr.span.into(),
            });
        };
        if nominal.fqn != "scoop.unsafe.FunPtr" {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "funptr invoke receiver kind",
                at: receiver_expr.span.into(),
            });
        }

        let sig_ty = nominal.args.first().copied().ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "funptr invoke signature type",
            at: receiver_expr.span.into(),
        })?;
        let TypeKind::Ref(RefTypeKind::Function(fun_ty)) = self.types.kind(sig_ty) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "funptr invoke signature kind",
                at: receiver_expr.span.into(),
            });
        };

        let CgTy::Int(int_ty) = local.ty else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "funptr invoke receiver cg type",
                at: receiver_expr.span.into(),
            });
        };
        let loaded = self
            .builder
            .build_load(
                self.llvm_basic_type_of(receiver_expr.span, local.ty)?,
                local.ptr,
                "load_funptr",
            )?
            .into_int_value();

        self.codegen_funptr_value_call(
            span,
            callee_span,
            loaded,
            int_ty,
            fun_ty,
            call_args,
        )
    }

    fn codegen_sysroot_funptr_to_uintptr(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "funptrToUIntPtr arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "funptrToUIntPtr named arg",
                at: span.into(),
            });
        };

        let v = self.codegen_expr(expr)?;
        let (raw, from_ty) = v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "funptrToUIntPtr arg type",
            at: expr.span.into(),
        })?;

        let to_ty = IntTy {
            bits: self.host.word_bit_width(),
            signed: false,
        };
        let casted = self.cast_int(raw, from_ty, to_ty)?;
        Ok(CgValue::int(casted, to_ty))
    }

    fn codegen_sysroot_uintptr_to_funptr(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "uintPtrToFunPtr arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "uintPtrToFunPtr named arg",
                at: span.into(),
            });
        };

        let v = self.codegen_expr(expr)?;
        let (raw, from_ty) = v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "uintPtrToFunPtr arg type",
            at: expr.span.into(),
        })?;

        let to_ty = IntTy {
            bits: self.host.word_bit_width(),
            signed: false,
        };
        let casted = self.cast_int(raw, from_ty, to_ty)?;
        Ok(CgValue::int(casted, to_ty))
    }

    /// 生成 class 构造调用（Appendix B.2.2，Kotlin-like 初始化顺序）。
    ///
    /// 当前阶段的约束（为保持 run-pass 可落地且实现量可控）：
    /// - 调用点仅支持位置参数（positional args），不支持 named args / default args；
    /// - ctor 选择规则：按“参数个数”在已收集 ctor 集合中匹配；若不唯一则报错；
    /// - class 单继承初始化链：会从最基类到派生类逐层执行 init steps；
    /// - super ctor args 与 secondary ctor delegation args 同样只支持位置参数，并按源码顺序求值。
    fn codegen_class_ctor_call(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        _name: &str,
        args: &[hir::CallArg],
        candidates: &[String],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        // 1) 选择唯一可用的 class candidate（HIR side table 里必须存在该 class 的 init 信息）。
        let mut class_candidates: Vec<&String> = candidates
            .iter()
            .filter(|fqn| self.class_inits.contains_key(*fqn))
            .collect();
        class_candidates.sort();
        class_candidates.dedup();

        let class_fqn = match class_candidates.as_slice() {
            [one] => (*one).clone(),
            [] => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "class ctor call candidate class",
                    at: callee_span.into(),
                });
            }
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "class ctor call candidate class (ambiguous)",
                    at: callee_span.into(),
                });
            }
        };
        let class = self.class_init_layout(callee_span, &class_fqn)?;

        // 2) 仅支持 positional args，并按源码顺序求值。
        let mut positional_args: Vec<&hir::Expr> = Vec::with_capacity(args.len());
        for arg in args {
            match arg {
                hir::CallArg::Positional(expr) => positional_args.push(expr),
                hir::CallArg::Named { .. } => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "class ctor call named arg",
                        at: span.into(),
                    });
                }
            }
        }

        // 3) 在 ctor 集合中选择匹配项（按参数个数）；若 class 未显式声明任何 ctor，则视为隐式 0-参 primary ctor。
        let matching: Vec<Option<&hir::ClassCtor>> = if class.ctors.is_empty() {
            if positional_args.is_empty() {
                vec![None]
            } else {
                Vec::new()
            }
        } else {
            class
                .ctors
                .iter()
                .filter(|ctor| ctor.params.len() == positional_args.len())
                .map(Some)
                .collect()
        };

        if matching.is_empty() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "class ctor call overload mismatch",
                at: callee_span.into(),
            });
        }
        if matching.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "class ctor call overload ambiguous",
                at: callee_span.into(),
            });
        }

        let selected_ctor = matching[0];
        let ctor_params: &[hir::ClassCtorParam] = match selected_ctor {
            Some(ctor) => ctor.params.as_slice(),
            None => &[][..],
        };

        // 4) 分配对象（header 由 runtime 初始化）；payload 先清零，避免读取未初始化字段导致的非确定性。
        let obj_ty = self.llvm_class_object_type(span, &class)?;
        let obj_size_bytes = self.target_data.get_store_size(&obj_ty);

        let rt_alloc = self.declare_runtime_alloc();
        let size_v = self.context.i64_type().const_int(obj_size_bytes, false);
        let call = self
            .builder
            .build_call(rt_alloc, &[size_v.into()], "rt_alloc_class")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(obj_ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc return type",
                at: span.into(),
            });
        };

        let obj_ptr_ty = obj_ty.ptr_type(self.gc_address_space());
        let typed_obj = self
            .builder
            .build_pointer_cast(obj_ptr, obj_ptr_ty, "class_obj_ptr")?;
        let payload_ptr =
            self.builder
                .build_struct_gep(obj_ty, typed_obj, 1, "class_payload_gep")?;
        let payload_ty = self.llvm_class_payload_type(span, &class)?;
        let payload_size_bytes = self.target_data.get_store_size(&payload_ty);
        if payload_size_bytes > 0 {
            let i8_ptr_ty = self.llvm_gc_i8_ptr_type();
            let payload_i8 = self
                .builder
                .build_bit_cast(payload_ptr, i8_ptr_ty, "class_payload_i8")?
                .into_pointer_value();
            let size_ty = self
                .target_data
                .ptr_sized_int_type_in_context(self.context, None);
            let size_v = size_ty.const_int(payload_size_bytes, false);
            let zero = self.context.i8_type().const_int(0, false);
            let _ = self.builder.build_memset(payload_i8, 1, zero, size_v)?;
        }

        // 5) 在执行 init steps / ctor body 期间，把 `this` 临时放进一个 1-slot GC frame，避免显式 GC 导致对象被回收。
        let tmp_gc_frame_ty = self.llvm_gc_frame_type(1);
        let tmp_gc_frame_ptr =
            self.create_entry_alloca_raw(span, "class_gc_frame", tmp_gc_frame_ty.into())?;

        let i8_ptr_ty = self.llvm_i8_ptr_type();
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let i32_ty = self.context.i32_type();
        let prev_ptr = self.builder.build_struct_gep(
            tmp_gc_frame_ty,
            tmp_gc_frame_ptr,
            0,
            "class_gc_prev_gep",
        )?;
        let root_count_ptr = self.builder.build_struct_gep(
            tmp_gc_frame_ty,
            tmp_gc_frame_ptr,
            1,
            "class_gc_root_count_gep",
        )?;
        let reserved_ptr = self.builder.build_struct_gep(
            tmp_gc_frame_ty,
            tmp_gc_frame_ptr,
            2,
            "class_gc_reserved_gep",
        )?;
        let _ = self.builder.build_store(prev_ptr, i8_ptr_ty.const_null())?;
        let _ = self
            .builder
            .build_store(root_count_ptr, i32_ty.const_int(1, false))?;
        let _ = self
            .builder
            .build_store(reserved_ptr, i32_ty.const_zero())?;

        let roots_arr_ptr = self.builder.build_struct_gep(
            tmp_gc_frame_ty,
            tmp_gc_frame_ptr,
            3,
            "class_gc_roots_arr_gep",
        )?;
        let roots_base = self.builder.build_pointer_cast(
            roots_arr_ptr,
            gc_i8_ptr_ty.ptr_type(AddressSpace::default()),
            "class_gc_roots_base",
        )?;
        let slot_ptr = unsafe {
            self.builder.build_in_bounds_gep(
                gc_i8_ptr_ty,
                roots_base,
                &[i32_ty.const_zero()],
                "class_gc_root_slot_0",
            )?
        };
        let _ = self.builder.build_store(slot_ptr, obj_ptr)?;

        let push = self.declare_runtime_gc_frame_push();
        let frame_i8 =
            self.builder
                .build_pointer_cast(tmp_gc_frame_ptr, i8_ptr_ty, "class_gc_frame_i8")?;
        let _ = self
            .builder
            .build_call(push, &[frame_i8.into()], "class_gc_frame_push")?;

        // T1327b：初始化期间若发生 `Raise.raise` / custom non-resuming effect 的 unwinding，
        // 必须先清理（pop）该临时 GC frame，再跳转到外层 catch/return；
        // 否则会破坏 shadow stack 的 push/pop 平衡，导致 roots 泄漏甚至后续 GC 行为不确定。
        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: span.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: span.into(),
            })?;

        let ctor_cont_bb = insert_block;
        let outer_raise_target = self.current_raise_target();

        // custom effect：为当前函数已有的 handle 边界注入 cleanup 包装，确保 perform unwind 先 pop 再跳转。
        let mut pushed_effect_wrappers: usize = 0;
        if !self.effect_unwind_target_stack.is_empty() {
            let mut seen_ops: HashSet<String> = HashSet::new();
            let mut outer_effect_targets: Vec<(String, inkwell::basic_block::BasicBlock<'ctx>)> =
                Vec::new();
            for t in self.effect_unwind_target_stack.iter().rev() {
                if seen_ops.insert(t.op_fqn.clone()) {
                    outer_effect_targets.push((t.op_fqn.clone(), t.target));
                }
            }
            // 稳定性：保持从外到内的遍历顺序，便于阅读 IR；不影响语义（按 op_fqn 精确匹配）。
            outer_effect_targets.reverse();

            for (idx, (op_fqn, outer_target)) in outer_effect_targets.iter().enumerate() {
                let cleanup_bb = self.context.append_basic_block(
                    func,
                    &format!("class_ctor_effect_cleanup_{idx}"),
                );
                self.builder.position_at_end(cleanup_bb);

                let pop = self.declare_runtime_gc_frame_pop();
                let frame_i8 = self.builder.build_pointer_cast(
                    tmp_gc_frame_ptr,
                    i8_ptr_ty,
                    "class_gc_frame_i8",
                )?;
                let _ = self.builder.build_call(
                    pop,
                    &[frame_i8.into()],
                    "class_gc_frame_pop_effect_cleanup",
                )?;
                self.builder.build_unconditional_branch(*outer_target)?;

                self.builder.position_at_end(ctor_cont_bb);
                self.push_effect_unwind_target(op_fqn.as_str(), cleanup_bb);
                pushed_effect_wrappers = pushed_effect_wrappers.saturating_add(1);
            }
        }

        // Raise.raise：同样注入 cleanup 包装（即使当前没有 outer catch，也需要 pop 后再 return 默认值向外传播）。
        let raise_cleanup_bb = self
            .context
            .append_basic_block(func, "class_ctor_raise_cleanup");
        self.builder.position_at_end(raise_cleanup_bb);
        let pop = self.declare_runtime_gc_frame_pop();
        let frame_i8 = self.builder.build_pointer_cast(
            tmp_gc_frame_ptr,
            i8_ptr_ty,
            "class_gc_frame_i8",
        )?;
        let _ = self.builder.build_call(
            pop,
            &[frame_i8.into()],
            "class_gc_frame_pop_raise_cleanup",
        )?;
        if let Some(target) = outer_raise_target {
            self.builder.build_unconditional_branch(target)?;
        } else {
            let ret_ty = self
                .current_fun_return_ty
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "Raise.raise needs function return type",
                    at: span.into(),
                })?;
            let v = self.default_value(ret_ty);
            self.emit_return(span, ret_ty, v)?;
        }

        self.builder.position_at_end(ctor_cont_bb);
        self.push_raise_target(raise_cleanup_bb);

        // 6) 执行构造调用：支持 super ctor args + secondary ctor delegation（T1327c）。
        //
        // 语义（Kotlin-like，Appendix B.2.2）：
        // - 调用点先按源码顺序求值 ctor 实参；
        // - 进入 ctor 后：
        //   - 若是 `: this(...)`，先执行被委托 ctor，再执行当前 ctor body；
        //   - 否则先执行 super ctor call，再执行本类的参数属性赋值、property initializer、init blocks，
        //     最后执行 secondary ctor body（若有）。

        // 调用点实参求值（按源码顺序），供“被调用的 ctor”注入 params locals。
        let mut evaluated_args: Vec<CgValue<'ctx>> = Vec::with_capacity(positional_args.len());
        for (param, arg_expr) in ctor_params.iter().zip(positional_args.iter()) {
            let param_cg = self
                .cg_ty_of(param.ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "class ctor param type",
                    at: callee_span.into(),
                })?;
            let v = self.codegen_expr_in_expected_context(arg_expr, Some(param_cg))?;
            let v = self.coerce_value(arg_expr.span, v, param_cg)?;
            evaluated_args.push(v);
        }

        self.codegen_class_ctor_invoke(
            span,
            callee_span,
            &class,
            selected_ctor,
            evaluated_args.as_slice(),
            obj_ptr,
        )?;

        self.pop_raise_target();
        for _ in 0..pushed_effect_wrappers {
            self.pop_effect_unwind_target();
        }

        // pop 临时 GC frame
        let pop = self.declare_runtime_gc_frame_pop();
        let frame_i8 =
            self.builder
                .build_pointer_cast(tmp_gc_frame_ptr, i8_ptr_ty, "class_gc_frame_i8")?;
        let _ = self
            .builder
            .build_call(pop, &[frame_i8.into()], "class_gc_frame_pop")?;

        Ok(CgValue {
            ty: CgTy::Ref,
            value: Some(obj_ptr.into()),
        })
    }

    fn pick_class_ctor_by_arity<'b>(
        &self,
        at: crate::span::Span,
        class: &'b hir::ClassInit,
        arg_count: usize,
        exclude_ctor_span: Option<crate::span::Span>,
        kind: &'static str,
    ) -> Result<Option<&'b hir::ClassCtor>, LlvmEmitError> {
        // 若 class 未显式声明任何 ctor，则视为隐式 0-参 primary ctor。
        if class.ctors.is_empty() {
            return if arg_count == 0 {
                Ok(None)
            } else {
                Err(LlvmEmitError::UnsupportedMainBody { kind, at: at.into() })
            };
        }

        let mut matching: Vec<&hir::ClassCtor> = class
            .ctors
            .iter()
            .filter(|ctor| ctor.params.len() == arg_count)
            .collect();
        if let Some(exclude) = exclude_ctor_span {
            matching.retain(|c| c.span != exclude);
        }

        if matching.is_empty() {
            return Err(LlvmEmitError::UnsupportedMainBody { kind, at: at.into() });
        }
        if matching.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody { kind, at: at.into() });
        }

        Ok(Some(matching[0]))
    }

    fn codegen_class_ctor_eval_args(
        &mut self,
        at: crate::span::Span,
        callee_span: crate::span::Span,
        arg_exprs: &[hir::Expr],
        ctor_params: &[hir::ClassCtorParam],
        kind: &'static str,
    ) -> Result<Vec<CgValue<'ctx>>, LlvmEmitError> {
        if arg_exprs.len() != ctor_params.len() {
            return Err(LlvmEmitError::UnsupportedMainBody { kind, at: at.into() });
        }

        let mut out: Vec<CgValue<'ctx>> = Vec::with_capacity(arg_exprs.len());
        for (param, arg_expr) in ctor_params.iter().zip(arg_exprs.iter()) {
            let param_cg = self
                .cg_ty_of(param.ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "class ctor param type",
                    at: callee_span.into(),
                })?;
            let v = self.codegen_expr_in_expected_context(arg_expr, Some(param_cg))?;
            let v = self.coerce_value(arg_expr.span, v, param_cg)?;
            out.push(v);
        }
        Ok(out)
    }

    fn codegen_class_ctor_call_super(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        class: &hir::ClassInit,
        super_arg_exprs: &[hir::Expr],
        obj_ptr: PointerValue<'ctx>,
        stack: &mut HashSet<(String, crate::span::Span)>,
        kind: &'static str,
    ) -> Result<(), LlvmEmitError> {
        let Some(super_fqn) = class.super_class_fqn.as_deref() else {
            return Ok(());
        };

        let super_class = self.class_init_layout(callee_span, super_fqn)?;
        let super_ctor = self.pick_class_ctor_by_arity(
            callee_span,
            &super_class,
            super_arg_exprs.len(),
            None,
            kind,
        )?;

        let super_ctor_params: &[hir::ClassCtorParam] = match super_ctor {
            Some(ctor) => ctor.params.as_slice(),
            None => &[][..],
        };
        let super_values = self.codegen_class_ctor_eval_args(
            callee_span,
            callee_span,
            super_arg_exprs,
            super_ctor_params,
            kind,
        )?;

        self.codegen_class_ctor_invoke_inner(
            span,
            callee_span,
            &super_class,
            super_ctor,
            super_values.as_slice(),
            obj_ptr,
            stack,
        )?;

        Ok(())
    }

    fn codegen_class_ctor_run_init_steps(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        class: &hir::ClassInit,
        ctor_params: &[hir::ClassCtorParam],
        stored_args: &[CgValue<'ctx>],
        obj_ptr: PointerValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        // primary ctor 参数属性赋值（在 super ctor 之后执行，Kotlin-like）。
        for (param, arg_v) in ctor_params.iter().zip(stored_args.iter()) {
            if !param.is_property {
                continue;
            }
            let param_cg = self
                .cg_ty_of(param.ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "class ctor param type",
                    at: callee_span.into(),
                })?;

            let Some(field_fqn) = param.property_field_fqn.as_deref() else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "class ctor param property fqn",
                    at: callee_span.into(),
                });
            };
            let Some(field_idx) = class.field_indices.get(field_fqn).copied() else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "class ctor param property field index",
                    at: callee_span.into(),
                });
            };
            let field_ptr = self.codegen_class_field_ptr(span, class, obj_ptr, field_idx)?;
            let _ = self.store_local_value(span, field_ptr, param_cg, *arg_v)?;
        }

        // property initializer / init blocks（按源码顺序）
        for step in &class.steps {
            match step {
                hir::ClassInitStep::PropertyInit { field_fqn, init } => {
                    let Some(field_idx) = class.field_indices.get(field_fqn).copied() else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "class property init field index",
                            at: init.span.into(),
                        });
                    };
                    let field = class.fields.get(field_idx as usize).ok_or(
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "class property init field",
                            at: init.span.into(),
                        },
                    )?;
                    let field_cg =
                        self.cg_ty_of(field.ty)
                            .ok_or(LlvmEmitError::UnsupportedMainBody {
                                kind: "class property init field type",
                                at: init.span.into(),
                            })?;

                    let v = self.codegen_expr_in_expected_context(init, Some(field_cg))?;
                    let field_ptr = self.codegen_class_field_ptr(init.span, class, obj_ptr, field_idx)?;
                    let _ = self.store_local_value(init.span, field_ptr, field_cg, v)?;
                }
                hir::ClassInitStep::InitBlock { block } => {
                    let _ = self.codegen_block_value(block)?;
                }
            }
        }

        Ok(())
    }

    fn codegen_class_ctor_invoke(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        class: &hir::ClassInit,
        ctor: Option<&hir::ClassCtor>,
        args: &[CgValue<'ctx>],
        obj_ptr: PointerValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let mut stack: HashSet<(String, crate::span::Span)> = HashSet::new();
        self.codegen_class_ctor_invoke_inner(span, callee_span, class, ctor, args, obj_ptr, &mut stack)
    }

    fn codegen_class_ctor_invoke_inner(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        class: &hir::ClassInit,
        ctor: Option<&hir::ClassCtor>,
        args: &[CgValue<'ctx>],
        obj_ptr: PointerValue<'ctx>,
        stack: &mut HashSet<(String, crate::span::Span)>,
    ) -> Result<(), LlvmEmitError> {
        let (ctor_kind, ctor_span, ctor_params, ctor_body, delegation) = match ctor {
            Some(ctor) => (
                ctor.kind,
                ctor.span,
                ctor.params.as_slice(),
                ctor.body.as_ref(),
                ctor.delegation.as_ref(),
            ),
            None => (hir::ClassCtorKind::Primary, callee_span, &[][..], None, None),
        };

        let key = (class.fqn.clone(), ctor_span);
        if !stack.insert(key.clone()) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "class ctor delegation cycle",
                at: callee_span.into(),
            });
        }

        let result = (|| {
        self.env.push_scope();

        // this local（注意：每一层都有独立的 this SymbolId）。
        let this_ptr = self.create_entry_alloca(span, "this", CgTy::Ref)?;
        let _ = self.builder.build_store(this_ptr, obj_ptr)?;
        self.env.insert(
            class.this_id,
            CgLocal {
                hir_ty: None,
                ty: CgTy::Ref,
                ptr: this_ptr,
                mutable: false,
                gc_root_slot: None,
            },
        );

        // ctor params locals（先写 locals；参数属性赋值延后到 super ctor call 之后）。
        if args.len() != ctor_params.len() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "class ctor call arg/param len mismatch",
                at: callee_span.into(),
            });
        }

        let mut stored_args: Vec<CgValue<'ctx>> = Vec::with_capacity(args.len());
        for (param, arg_v) in ctor_params.iter().zip(args.iter()) {
            let param_cg = self
                .cg_ty_of(param.ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "class ctor param type",
                    at: callee_span.into(),
                })?;
            let param_ptr = self.create_entry_alloca(param.decl_span, &param.name, param_cg)?;
            let stored = self.store_local_value(param.decl_span, param_ptr, param_cg, *arg_v)?;
            stored_args.push(stored);
            self.env.insert(
                param.id,
                CgLocal {
                    hir_ty: Some(param.ty),
                    ty: param_cg,
                    ptr: param_ptr,
                    mutable: false,
                    gc_root_slot: None,
                },
            );
        }

        // secondary ctor delegation（T1327c）
        if ctor_kind == hir::ClassCtorKind::Secondary {
            if let Some(deleg) = delegation {
                match deleg.kind {
                    ast::CtorDelegationKind::This => {
                        let target = self.pick_class_ctor_by_arity(
                            callee_span,
                            class,
                            deleg.args.len(),
                            Some(ctor_span),
                            "class this delegation overload mismatch/ambiguous",
                        )?;

                        let target_params: &[hir::ClassCtorParam] = match target {
                            Some(c) => c.params.as_slice(),
                            None => &[][..],
                        };
                        let target_values = self.codegen_class_ctor_eval_args(
                            callee_span,
                            callee_span,
                            deleg.args.as_slice(),
                            target_params,
                            "class this delegation arg eval",
                        )?;

                        self.codegen_class_ctor_invoke_inner(
                            span,
                            callee_span,
                            class,
                            target,
                            target_values.as_slice(),
                            obj_ptr,
                            stack,
                        )?;

                        if let Some(body) = ctor_body {
                            let _ = self.codegen_block_value(body)?;
                        }

                        self.env.pop_scope();
                        return Ok(());
                    }
                    ast::CtorDelegationKind::Super => {
                        self.codegen_class_ctor_call_super(
                            span,
                            callee_span,
                            class,
                            deleg.args.as_slice(),
                            obj_ptr,
                            stack,
                            "class super delegation overload mismatch/ambiguous",
                        )?;

                        self.codegen_class_ctor_run_init_steps(
                            span,
                            callee_span,
                            class,
                            ctor_params,
                            stored_args.as_slice(),
                            obj_ptr,
                        )?;

                        if let Some(body) = ctor_body {
                            let _ = self.codegen_block_value(body)?;
                        }

                        self.env.pop_scope();
                        return Ok(());
                    }
                }
            }
        }

        // primary ctor / secondary ctor（无 delegation）路径：使用 class header 的 super ctor args。
        self.codegen_class_ctor_call_super(
            span,
            callee_span,
            class,
            class.super_ctor_args.as_slice(),
            obj_ptr,
            stack,
            "class super ctor call overload mismatch/ambiguous",
        )?;

        self.codegen_class_ctor_run_init_steps(
            span,
            callee_span,
            class,
            ctor_params,
            stored_args.as_slice(),
            obj_ptr,
        )?;

        if ctor_kind == hir::ClassCtorKind::Secondary {
            if let Some(body) = ctor_body {
                let _ = self.codegen_block_value(body)?;
            }
        }

        self.env.pop_scope();
        Ok(())
        })();

        stack.remove(&key);
        result
    }

    fn codegen_immediate_resume_call(
        &mut self,
        span: crate::span::Span,
        args: &[hir::CallArg],
        expected: Option<CgTy>,
        ctx: ImmediateResumeCtx<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        // 语义：写回 resume value + 更新 state + 跳回 dispatch。
        //
        // 当前阶段（T0616）约束：
        // - 仅支持一个位置实参：`resume(value)`；
        // - 多次 resume 先按运行期错误处理（exit(3)）。
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "resume() arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(value_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "resume() named arg",
                at: span.into(),
            });
        };

        let value = self.codegen_expr_in_expected_context(value_expr, Some(ctx.resume_value_ty))?;
        let value = self.coerce_value(value_expr.span, value, ctx.resume_value_ty)?;

        // one-shot（运行期断言）：重复调用 resume 直接退出。
        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: span.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: span.into(),
            })?;

        let ok_bb = self.context.append_basic_block(func, "resume_ok");
        let err_bb = self.context.append_basic_block(func, "resume_twice");
        let cont_bb = self.context.append_basic_block(func, "resume_cont");

        let used = self
            .builder
            .build_load(self.context.bool_type(), ctx.resume_used_ptr, "resume_used")?
            .into_int_value();
        self.builder.build_conditional_branch(used, err_bb, ok_bb)?;

        // --- err ---
        self.builder.position_at_end(err_bb);
        self.emit_exit_with_code(span, 3)?;

        // --- ok ---
        self.builder.position_at_end(ok_bb);
        let _ = self.builder.build_store(
            ctx.resume_used_ptr,
            self.context.bool_type().const_int(1, false),
        )?;

        if let Some(ptr) = ctx.resume_value_ptr {
            let Some(raw) = value.value else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "resume(value) arg value",
                    at: value_expr.span.into(),
                });
            };
            let _ = self.builder.build_store(ptr, raw)?;
        }

        let _ = self.builder.build_store(
            ctx.state_ptr,
            self.context
                .i32_type()
                .const_int(ctx.next_state as u64, false),
        )?;

        self.builder.build_unconditional_branch(cont_bb)?;

        // --- cont ---
        self.builder.position_at_end(cont_bb);

        Ok(match expected {
            Some(ty) => self.default_value(ty),
            None => CgValue::unit(),
        })
    }

    fn codegen_continuation_resume_call(
        &mut self,
        span: crate::span::Span,
        receiver: &hir::Expr,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        // spec §5.5：`k.resume(value)`。
        //
        // 约束（early stage）：
        // - 仅支持一个位置实参；
        // - `value` 会被编码为一个 `u64` word 传给 runtime（T0914：`scoop_continuation_resume_u64`）。
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "Continuation.resume arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(value_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "Continuation.resume named arg",
                at: span.into(),
            });
        };

        let recv = self.codegen_expr_in_expected_context(receiver, Some(CgTy::Ref))?;
        let recv = self.coerce_value(receiver.span, recv, CgTy::Ref)?;
        let Some(recv_raw) = recv.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "Continuation.resume receiver value",
                at: receiver.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(k_ptr) = recv_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "Continuation.resume receiver type",
                at: receiver.span.into(),
            });
        };

        let value = self.codegen_expr(value_expr)?;
        let word = self.coerce_u64_word(value_expr.span, value)?;

        let rt_resume = self.declare_runtime_continuation_resume_u64();
        let k_i8 = self
            .builder
            .build_pointer_cast(k_ptr, self.llvm_gc_i8_ptr_type(), "cont_k_i8")?;
        let _ = self
            .builder
            .build_call(rt_resume, &[k_i8.into(), word.into()], "cont_resume")?;
        // continuation resume 可能触发 `Raise<RuntimeError>`（例如 one-shot 违规），需要按 Raise 的最小约定传播。
        self.emit_effect_unwind_if_active(span)?;

        Ok(CgValue::unit())
    }

    fn coerce_u64_word(
        &mut self,
        at: crate::span::Span,
        value: CgValue<'ctx>,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        // 将一个可表示为 “word-sized u64 payload” 的值转换为 `i64`（在 ABI 层作为 `uint64_t` 使用）。
        //
        // 注意：这里不引入额外的 tag/布局；更复杂的 payload 由 TODO T0630 扩展。
        let i64_ty = self.context.i64_type();
        match value.ty {
            CgTy::Unit => Ok(i64_ty.const_int(0, false)),
            CgTy::Bool => {
                let b = value.as_bool().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "u64 word from bool",
                    at: at.into(),
                })?;
                Ok(self.builder.build_int_z_extend(b, i64_ty, "bool_to_u64")?)
            }
            CgTy::Int(_) => {
                let (raw, from) = value.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "u64 word from int",
                    at: at.into(),
                })?;
                let to = IntTy {
                    bits: 64,
                    signed: false,
                };
                Ok(self.cast_int(raw, from, to)?)
            }
            CgTy::String | CgTy::Ref => {
                let Some(raw) = value.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "u64 word from pointer value",
                        at: at.into(),
                    });
                };
                let BasicValueEnum::PointerValue(ptr) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "u64 word from pointer type",
                        at: at.into(),
                    });
                };
                Ok(self.builder.build_ptr_to_int(ptr, i64_ty, "ptr_to_u64")?)
            }
            CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
                Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "u64 word from composite value",
                    at: at.into(),
                })
            }
        }
    }

    fn codegen_string_trim_indent(
        &mut self,
        span: crate::span::Span,
        receiver: &hir::Expr,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if !args.is_empty() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "trimIndent arity mismatch",
                at: span.into(),
            });
        }

        let recv = self.codegen_expr_in_expected_context(receiver, Some(CgTy::String))?;
        let coerced = self.coerce_value(receiver.span, recv, CgTy::String)?;
        let Some(raw) = coerced.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "trimIndent receiver value",
                at: receiver.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(recv_ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "trimIndent receiver type",
                at: receiver.span.into(),
            });
        };

        let rt_fun = self.declare_runtime_trim_indent();
        let call = self
            .builder
            .build_call(rt_fun, &[recv_ptr.into()], "rt_trim_indent")?;
        let ret = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "trimIndent return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(out_ptr) = ret else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "trimIndent return type",
                at: span.into(),
            });
        };

        Ok(CgValue {
            ty: CgTy::String,
            value: Some(out_ptr.into()),
        })
    }

    fn codegen_sysroot_gc_pin(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        args: &[hir::CallArg],
        expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.pin arity mismatch",
                at: span.into(),
            });
        }
        let hir::CallArg::Positional(obj_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.pin named arg",
                at: span.into(),
            });
        };

        let Some(CgTy::Struct(pinned_ty)) = expected else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.pin call without expected pinned type",
                at: callee_span.into(),
            });
        };

        let (field_idx, field_cg_ty) =
            self.lookup_struct_field(pinned_ty, "scoop.core.Pinned.value", callee_span)?;

        let obj_v = self.codegen_expr_in_expected_context(obj_expr, Some(field_cg_ty))?;
        let obj_v = self.coerce_value(obj_expr.span, obj_v, field_cg_ty)?;

        // 运行期 pin 需要 `void*`：统一使用 `i8*`。
        let obj_ref = self.coerce_value(obj_expr.span, obj_v, CgTy::Ref)?;
        let Some(obj_raw) = obj_ref.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.pin arg value",
                at: obj_expr.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(obj_ptr) = obj_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.pin arg type",
                at: obj_expr.span.into(),
            });
        };

        let rt_pin = self.declare_runtime_gc_pin();
        let call = self
            .builder
            .build_call(rt_pin, &[obj_ptr.into()], "gc_pin")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.pin return value",
                at: span.into(),
            })?;
        let BasicValueEnum::IntValue(ok_i32) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.pin return type",
                at: span.into(),
            });
        };

        let ok_cond = self.builder.build_int_compare(
            IntPredicate::NE,
            ok_i32,
            self.context.i32_type().const_zero(),
            "gc_pin_ok",
        )?;

        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: span.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: span.into(),
            })?;

        let ok_bb = self.context.append_basic_block(func, "gc_pin_ok_bb");
        let err_bb = self.context.append_basic_block(func, "gc_pin_err_bb");
        let cont_bb = self.context.append_basic_block(func, "gc_pin_cont_bb");
        self.builder
            .build_conditional_branch(ok_cond, ok_bb, err_bb)?;

        // --- err ---
        self.builder.position_at_end(err_bb);
        self.emit_exit_with_code(span, 3)?;

        // --- ok ---
        self.builder.position_at_end(ok_bb);
        let llvm_struct_ty = self.llvm_struct_type(span, pinned_ty)?;
        let mut agg: AggregateValueEnum<'ctx> = llvm_struct_ty.get_undef().into();
        let raw_field: BasicValueEnum<'ctx> = match field_cg_ty {
            CgTy::Unit => self.context.i8_type().const_int(0, false).into(),
            _ => obj_v.value.ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.pin field value",
                at: obj_expr.span.into(),
            })?,
        };
        agg = self
            .builder
            .build_insert_value(agg, raw_field, field_idx, "pinned_value")?;
        self.builder.build_unconditional_branch(cont_bb)?;

        // --- cont ---
        self.builder.position_at_end(cont_bb);
        Ok(CgValue {
            ty: CgTy::Struct(pinned_ty),
            value: Some(agg.as_basic_value_enum()),
        })
    }

    fn codegen_sysroot_gc_unpin(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let _ = callee_span;
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.unpin arity mismatch",
                at: span.into(),
            });
        }
        let hir::CallArg::Positional(pinned_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.unpin named arg",
                at: span.into(),
            });
        };

        let pinned_v = self.codegen_expr(pinned_expr)?;
        let CgTy::Struct(pinned_ty) = pinned_v.ty else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.unpin arg type",
                at: pinned_expr.span.into(),
            });
        };
        let Some(raw) = pinned_v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.unpin arg value",
                at: pinned_expr.span.into(),
            });
        };
        let BasicValueEnum::StructValue(struct_v) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.unpin arg value type",
                at: pinned_expr.span.into(),
            });
        };

        let (field_idx, field_cg_ty) =
            self.lookup_struct_field(pinned_ty, "scoop.core.Pinned.value", pinned_expr.span)?;
        let extracted = self
            .builder
            .build_extract_value(struct_v, field_idx, "pinned_value")?;
        let field_v = self.cg_value_from_loaded(pinned_expr.span, field_cg_ty, extracted)?;
        let field_ref = self.coerce_value(pinned_expr.span, field_v, CgTy::Ref)?;

        let Some(field_raw) = field_ref.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.unpin value",
                at: pinned_expr.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(obj_ptr) = field_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.unpin value type",
                at: pinned_expr.span.into(),
            });
        };

        let rt_unpin = self.declare_runtime_gc_unpin();
        let call = self
            .builder
            .build_call(rt_unpin, &[obj_ptr.into()], "gc_unpin")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.unpin return value",
                at: span.into(),
            })?;
        let BasicValueEnum::IntValue(ok_i32) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "GC.unpin return type",
                at: span.into(),
            });
        };

        let ok_cond = self.builder.build_int_compare(
            IntPredicate::NE,
            ok_i32,
            self.context.i32_type().const_zero(),
            "gc_unpin_ok",
        )?;

        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: span.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: span.into(),
            })?;

        let ok_bb = self.context.append_basic_block(func, "gc_unpin_ok_bb");
        let err_bb = self.context.append_basic_block(func, "gc_unpin_err_bb");
        let cont_bb = self.context.append_basic_block(func, "gc_unpin_cont_bb");
        self.builder
            .build_conditional_branch(ok_cond, ok_bb, err_bb)?;

        // --- err ---
        self.builder.position_at_end(err_bb);
        self.emit_exit_with_code(span, 3)?;

        // --- ok ---
        self.builder.position_at_end(ok_bb);
        self.builder.build_unconditional_branch(cont_bb)?;

        // --- cont ---
        self.builder.position_at_end(cont_bb);
        Ok(CgValue::unit())
    }

    fn codegen_sysroot_print_like(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        fqn: &str,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sysroot print/println arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sysroot print/println named arg",
                at: span.into(),
            });
        };

        let rt_name = match fqn {
            "scoop.core.print" => "scoop_print",
            "scoop.core.println" => "scoop_println",
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "unknown sysroot print/println callee",
                    at: callee_span.into(),
                });
            }
        };

        // 说明：
        // - sysroot 中允许 `print/println` 以 overload set 的形式声明（例如 `String` 与 `Int`）；
        // - HIR 当前阶段不保留“已选定 overload”的信息，因此这里以实参 codegen 后的 `CgTy`
        //   来决定使用哪条 lowering 路径。
        //
        // 注意：这里**不要**强制把 expected type 设为 `String`：
        // - 对于 `when/if/block` 等表达式，expected 会导致其 arm/body 被强制 coercion 为 `String`，
        //   进而在 `Int -> String` 这类尚未实现的 coercion 上报错；
        // - `print/println` 的整数路径会在 codegen 后显式把 `Int` 转为 `String`（见下方分支），
        //   因此应先让表达式产出其“自然值类型”，再在这里做转换。
        let v = self.codegen_expr(expr)?;
        let str_ptr = match v.ty {
            CgTy::String => {
                let coerced = self.coerce_value(expr.span, v, CgTy::String)?;
                let Some(raw) = coerced.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "sysroot print/println arg value",
                        at: expr.span.into(),
                    });
                };
                let BasicValueEnum::PointerValue(str_ptr) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "sysroot print/println arg type",
                        at: expr.span.into(),
                    });
                };
                str_ptr
            }
            CgTy::Int(from_ty) => {
                let (raw_int, _) = v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "sysroot print/println int arg value",
                    at: expr.span.into(),
                })?;
                self.codegen_int_to_scoop_string(expr.span, raw_int, from_ty)?
            }
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "sysroot print/println arg type",
                    at: expr.span.into(),
                });
            }
        };

        let rt_fun = self.declare_runtime_print_like(rt_name);
        let _ = self
            .builder
            .build_call(rt_fun, &[str_ptr.into()], "rt_print")?;
        Ok(CgValue::unit())
    }

    fn codegen_sysroot_io_write_string(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        rt_name: &str,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "io writeString arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "io writeString named arg",
                at: span.into(),
            });
        };

        // v0：只支持 String 入参（与 sysroot 声明面一致）。
        let v = self.codegen_expr_in_expected_context(expr, Some(CgTy::String))?;
        let coerced = self.coerce_value(expr.span, v, CgTy::String)?;
        let Some(raw) = coerced.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "io writeString arg value",
                at: expr.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(str_ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "io writeString arg type",
                at: expr.span.into(),
            });
        };

        let rt_fun = self.declare_runtime_print_like(rt_name);
        let _ = self
            .builder
            .build_call(rt_fun, &[str_ptr.into()], "rt_io_write_string")?;
        Ok(CgValue::unit())
    }

    fn codegen_sysroot_io_stdin_read_line_utf8(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
        expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if !args.is_empty() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "stdin.readLine arity mismatch",
                at: span.into(),
            });
        }

        let rt = self.declare_runtime_io_stdin_read_line_utf8();
        let call = self.builder.build_call(rt, &[], "stdin_read_line")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "stdin.readLine return value",
                at: span.into(),
            })?;

        // 返回类型依赖 expected context（HIR v0 对大部分 call expr 仍用 `Any` 占位）。
        let Some(ret_cg_ty) = expected else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "stdin.readLine missing expected type context",
                at: span.into(),
            });
        };
        if !matches!(ret_cg_ty, CgTy::Enum(_)) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "stdin.readLine expected Option<String>",
                at: span.into(),
            });
        }

        Ok(CgValue {
            ty: ret_cg_ty,
            value: Some(raw),
        })
    }

    fn codegen_sysroot_env_get(
        &mut self,
        span: crate::span::Span,
        args: &[hir::CallArg],
        expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "env.getOrNull arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(key_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "env.getOrNull named arg",
                at: span.into(),
            });
        };

        let key_v = self.codegen_expr_in_expected_context(key_expr, Some(CgTy::String))?;
        let key_v = self.coerce_value(key_expr.span, key_v, CgTy::String)?;
        let Some(raw_key) = key_v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "env.getOrNull key value",
                at: key_expr.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(key_ptr) = raw_key else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "env.getOrNull key type",
                at: key_expr.span.into(),
            });
        };

        let rt = self.declare_runtime_env_get();
        let call = self
            .builder
            .build_call(rt, &[key_ptr.into()], "env_get")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "env.getOrNull return value",
                at: span.into(),
            })?;

        // 返回类型依赖 expected context（HIR v0 对大部分 call expr 仍用 `Any` 占位）。
        let Some(ret_cg_ty) = expected else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "env.getOrNull missing expected type context",
                at: span.into(),
            });
        };
        if !matches!(ret_cg_ty, CgTy::Enum(_)) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "env.getOrNull expected Option<String>",
                at: span.into(),
            });
        }
        Ok(CgValue {
            ty: ret_cg_ty,
            value: Some(raw),
        })
    }

    fn codegen_sysroot_time_now_unix_millis(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if !args.is_empty() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "time.nowUnixMillis arity mismatch",
                at: span.into(),
            });
        }

        let rt = self.declare_runtime_time_now_unix_millis();
        let call = self
            .builder
            .build_call(rt, &[], "time_now_unix_millis")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "time.nowUnixMillis return value",
                at: span.into(),
            })?;
        let BasicValueEnum::IntValue(raw_i64) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "time.nowUnixMillis return type",
                at: span.into(),
            });
        };

        let from = IntTy {
            bits: 64,
            signed: true,
        };
        let to = IntTy {
            bits: self.host.word_bit_width(),
            signed: true,
        };
        let casted = self.cast_int(raw_i64, from, to)?;
        Ok(CgValue::int(casted, to))
    }

    fn codegen_sysroot_fs_read_all_text_utf8(
        &mut self,
        span: crate::span::Span,
        args: &[hir::CallArg],
        expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "fs.readAllText arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(path_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "fs.readAllText named arg",
                at: span.into(),
            });
        };

        let path_v = self.codegen_expr_in_expected_context(path_expr, Some(CgTy::String))?;
        let path_v = self.coerce_value(path_expr.span, path_v, CgTy::String)?;
        let Some(raw_path) = path_v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "fs.readAllText path value",
                at: path_expr.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(path_ptr) = raw_path else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "fs.readAllText path type",
                at: path_expr.span.into(),
            });
        };

        let rt = self.declare_runtime_fs_read_all_text_utf8();
        let call = self
            .builder
            .build_call(rt, &[path_ptr.into()], "fs_read_all_text")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "fs.readAllText return value",
                at: span.into(),
            })?;

        // 返回类型依赖 expected context（HIR v0 对大部分 call expr 仍用 `Any` 占位）。
        let Some(ret_cg_ty) = expected else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "fs.readAllText missing expected type context",
                at: span.into(),
            });
        };
        if !matches!(ret_cg_ty, CgTy::Enum(_)) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "fs.readAllText expected Option<String>",
                at: span.into(),
            });
        }
        Ok(CgValue {
            ty: ret_cg_ty,
            value: Some(raw),
        })
    }

    fn codegen_sysroot_fs_write_all_text_utf8(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 2 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "fs.writeAllText arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(path_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "fs.writeAllText named arg (path)",
                at: span.into(),
            });
        };
        let hir::CallArg::Positional(content_expr) = &args[1] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "fs.writeAllText named arg (content)",
                at: span.into(),
            });
        };

        let path_v = self.codegen_expr_in_expected_context(path_expr, Some(CgTy::String))?;
        let path_v = self.coerce_value(path_expr.span, path_v, CgTy::String)?;
        let Some(raw_path) = path_v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "fs.writeAllText path value",
                at: path_expr.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(path_ptr) = raw_path else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "fs.writeAllText path type",
                at: path_expr.span.into(),
            });
        };

        let content_v = self.codegen_expr_in_expected_context(content_expr, Some(CgTy::String))?;
        let content_v = self.coerce_value(content_expr.span, content_v, CgTy::String)?;
        let Some(raw_content) = content_v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "fs.writeAllText content value",
                at: content_expr.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(content_ptr) = raw_content else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "fs.writeAllText content type",
                at: content_expr.span.into(),
            });
        };

        let rt = self.declare_runtime_fs_write_all_text_utf8();
        let call = self.builder.build_call(
            rt,
            &[path_ptr.into(), content_ptr.into()],
            "fs_write_all_text",
        )?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "fs.writeAllText return value",
                at: span.into(),
            })?;
        let BasicValueEnum::IntValue(raw_i64) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "fs.writeAllText return type",
                at: span.into(),
            });
        };

        // runtime 返回 i64：向 host word size 的 `Int` 做一次 cast（与 time API 一致）。
        let from = IntTy {
            bits: 64,
            signed: true,
        };
        let to = IntTy {
            bits: self.host.word_bit_width(),
            signed: true,
        };
        let casted = self.cast_int(raw_i64, from, to)?;
        Ok(CgValue::int(casted, to))
    }

    fn codegen_sysroot_process_exit(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "process.exit arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(code_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "process.exit named arg",
                at: span.into(),
            });
        };

        let value_word = IntTy {
            bits: self.host.word_bit_width(),
            signed: true,
        };
        let code_v = self
            .codegen_expr_in_expected_context(code_expr, Some(CgTy::Int(value_word)))?;
        let code_v = self.coerce_value(code_expr.span, code_v, CgTy::Int(value_word))?;
        let (code_raw, code_from) =
            code_v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "process.exit code value",
                at: code_expr.span.into(),
            })?;
        let code_to = IntTy {
            bits: 64,
            signed: true,
        };
        let code_i64 = self.cast_int(code_raw, code_from, code_to)?;

        let rt = self.declare_runtime_process_exit();
        let _ = self
            .builder
            .build_call(rt, &[code_i64.into()], "process_exit")?;
        Ok(CgValue::unit())
    }

    fn codegen_sysroot_process_args(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if !args.is_empty() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "process.args arity mismatch",
                at: span.into(),
            });
        }

        let rt = self.declare_runtime_process_args_array();
        let call = self.builder.build_call(rt, &[], "process_args")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "process.args return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(arr_ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "process.args return type",
                at: span.into(),
            });
        };

        Ok(CgValue {
            ty: CgTy::Ref,
            value: Some(arr_ptr.into()),
        })
    }

    fn codegen_sysroot_path_normalize(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "path.normalize arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(path_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "path.normalize named arg",
                at: span.into(),
            });
        };

        let path_v = self.codegen_expr_in_expected_context(path_expr, Some(CgTy::String))?;
        let path_v = self.coerce_value(path_expr.span, path_v, CgTy::String)?;
        let Some(raw_path) = path_v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "path.normalize path value",
                at: path_expr.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(path_ptr) = raw_path else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "path.normalize path type",
                at: path_expr.span.into(),
            });
        };

        let rt = self.declare_runtime_path_normalize();
        let call = self
            .builder
            .build_call(rt, &[path_ptr.into()], "path_normalize")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "path.normalize return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(out_ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "path.normalize return type",
                at: span.into(),
            });
        };

        Ok(CgValue {
            ty: CgTy::String,
            value: Some(out_ptr.into()),
        })
    }

    fn codegen_sysroot_path_join(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 2 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "path.join arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(base_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "path.join named arg (base)",
                at: span.into(),
            });
        };
        let hir::CallArg::Positional(child_expr) = &args[1] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "path.join named arg (child)",
                at: span.into(),
            });
        };

        let base_v = self.codegen_expr_in_expected_context(base_expr, Some(CgTy::String))?;
        let base_v = self.coerce_value(base_expr.span, base_v, CgTy::String)?;
        let Some(raw_base) = base_v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "path.join base value",
                at: base_expr.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(base_ptr) = raw_base else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "path.join base type",
                at: base_expr.span.into(),
            });
        };

        let child_v = self.codegen_expr_in_expected_context(child_expr, Some(CgTy::String))?;
        let child_v = self.coerce_value(child_expr.span, child_v, CgTy::String)?;
        let Some(raw_child) = child_v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "path.join child value",
                at: child_expr.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(child_ptr) = raw_child else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "path.join child type",
                at: child_expr.span.into(),
            });
        };

        let rt = self.declare_runtime_path_join();
        let call = self.builder.build_call(
            rt,
            &[base_ptr.into(), child_ptr.into()],
            "path_join",
        )?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "path.join return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(out_ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "path.join return type",
                at: span.into(),
            });
        };

        Ok(CgValue {
            ty: CgTy::String,
            value: Some(out_ptr.into()),
        })
    }

    fn codegen_sysroot_path_basename(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "path.basename arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(path_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "path.basename named arg",
                at: span.into(),
            });
        };

        let path_v = self.codegen_expr_in_expected_context(path_expr, Some(CgTy::String))?;
        let path_v = self.coerce_value(path_expr.span, path_v, CgTy::String)?;
        let Some(raw_path) = path_v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "path.basename path value",
                at: path_expr.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(path_ptr) = raw_path else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "path.basename path type",
                at: path_expr.span.into(),
            });
        };

        let rt = self.declare_runtime_path_basename();
        let call = self
            .builder
            .build_call(rt, &[path_ptr.into()], "path_basename")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "path.basename return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(out_ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "path.basename return type",
                at: span.into(),
            });
        };

        Ok(CgValue {
            ty: CgTy::String,
            value: Some(out_ptr.into()),
        })
    }

    fn codegen_sysroot_path_dirname(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "path.dirname arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(path_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "path.dirname named arg",
                at: span.into(),
            });
        };

        let path_v = self.codegen_expr_in_expected_context(path_expr, Some(CgTy::String))?;
        let path_v = self.coerce_value(path_expr.span, path_v, CgTy::String)?;
        let Some(raw_path) = path_v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "path.dirname path value",
                at: path_expr.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(path_ptr) = raw_path else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "path.dirname path type",
                at: path_expr.span.into(),
            });
        };

        let rt = self.declare_runtime_path_dirname();
        let call = self
            .builder
            .build_call(rt, &[path_ptr.into()], "path_dirname")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "path.dirname return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(out_ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "path.dirname return type",
                at: span.into(),
            });
        };

        Ok(CgValue {
            ty: CgTy::String,
            value: Some(out_ptr.into()),
        })
    }

    // --- std v3：sync（T1319b） ---

    fn codegen_sysroot_sync_mutex_create(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if !args.is_empty() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.mutexCreate arity mismatch",
                at: span.into(),
            });
        }

        let rt = self.declare_runtime_sync_mutex_create();
        let call = self
            .builder
            .build_call(rt, &[], "sync_mutex_create")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.mutexCreate return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.mutexCreate return type",
                at: span.into(),
            });
        };

        Ok(CgValue {
            ty: CgTy::Ref,
            value: Some(ptr.into()),
        })
    }

    fn codegen_sysroot_sync_mutex_lock(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.Mutex.lock arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(recv_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.Mutex.lock named arg (receiver)",
                at: span.into(),
            });
        };

        let recv_v = self.codegen_expr_in_expected_context(recv_expr, Some(CgTy::Ref))?;
        let recv_v = self.coerce_value(recv_expr.span, recv_v, CgTy::Ref)?;
        let Some(recv_raw) = recv_v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.Mutex.lock receiver value",
                at: recv_expr.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(recv_ptr) = recv_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.Mutex.lock receiver type",
                at: recv_expr.span.into(),
            });
        };

        let rt = self.declare_runtime_sync_mutex_lock();
        let _ = self
            .builder
            .build_call(rt, &[recv_ptr.into()], "sync_mutex_lock")?;
        Ok(CgValue::unit())
    }

    fn codegen_sysroot_sync_mutex_unlock(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.Mutex.unlock arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(recv_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.Mutex.unlock named arg (receiver)",
                at: span.into(),
            });
        };

        let recv_v = self.codegen_expr_in_expected_context(recv_expr, Some(CgTy::Ref))?;
        let recv_v = self.coerce_value(recv_expr.span, recv_v, CgTy::Ref)?;
        let Some(recv_raw) = recv_v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.Mutex.unlock receiver value",
                at: recv_expr.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(recv_ptr) = recv_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.Mutex.unlock receiver type",
                at: recv_expr.span.into(),
            });
        };

        let rt = self.declare_runtime_sync_mutex_unlock();
        let _ = self
            .builder
            .build_call(rt, &[recv_ptr.into()], "sync_mutex_unlock")?;
        Ok(CgValue::unit())
    }

    fn codegen_sysroot_sync_condvar_create(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if !args.is_empty() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.condVarCreate arity mismatch",
                at: span.into(),
            });
        }

        let rt = self.declare_runtime_sync_condvar_create();
        let call = self
            .builder
            .build_call(rt, &[], "sync_condvar_create")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.condVarCreate return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.condVarCreate return type",
                at: span.into(),
            });
        };

        Ok(CgValue {
            ty: CgTy::Ref,
            value: Some(ptr.into()),
        })
    }

    fn codegen_sysroot_sync_condvar_wait(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 2 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.CondVar.wait arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(cv_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.CondVar.wait named arg (receiver)",
                at: span.into(),
            });
        };
        let hir::CallArg::Positional(mutex_expr) = &args[1] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.CondVar.wait named arg (mutex)",
                at: span.into(),
            });
        };

        let cv_v = self.codegen_expr_in_expected_context(cv_expr, Some(CgTy::Ref))?;
        let cv_v = self.coerce_value(cv_expr.span, cv_v, CgTy::Ref)?;
        let Some(cv_raw) = cv_v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.CondVar.wait receiver value",
                at: cv_expr.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(cv_ptr) = cv_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.CondVar.wait receiver type",
                at: cv_expr.span.into(),
            });
        };

        let m_v = self.codegen_expr_in_expected_context(mutex_expr, Some(CgTy::Ref))?;
        let m_v = self.coerce_value(mutex_expr.span, m_v, CgTy::Ref)?;
        let Some(m_raw) = m_v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.CondVar.wait mutex value",
                at: mutex_expr.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(m_ptr) = m_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.CondVar.wait mutex type",
                at: mutex_expr.span.into(),
            });
        };

        let rt = self.declare_runtime_sync_condvar_wait();
        let _ = self.builder.build_call(
            rt,
            &[cv_ptr.into(), m_ptr.into()],
            "sync_condvar_wait",
        )?;
        Ok(CgValue::unit())
    }

    fn codegen_sysroot_sync_condvar_notify_one(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.CondVar.notifyOne arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(cv_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.CondVar.notifyOne named arg (receiver)",
                at: span.into(),
            });
        };

        let cv_v = self.codegen_expr_in_expected_context(cv_expr, Some(CgTy::Ref))?;
        let cv_v = self.coerce_value(cv_expr.span, cv_v, CgTy::Ref)?;
        let Some(cv_raw) = cv_v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.CondVar.notifyOne receiver value",
                at: cv_expr.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(cv_ptr) = cv_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.CondVar.notifyOne receiver type",
                at: cv_expr.span.into(),
            });
        };

        let rt = self.declare_runtime_sync_condvar_notify_one();
        let _ = self.builder.build_call(rt, &[cv_ptr.into()], "sync_condvar_notify_one")?;
        Ok(CgValue::unit())
    }

    fn codegen_sysroot_sync_condvar_notify_all(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.CondVar.notifyAll arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(cv_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.CondVar.notifyAll named arg (receiver)",
                at: span.into(),
            });
        };

        let cv_v = self.codegen_expr_in_expected_context(cv_expr, Some(CgTy::Ref))?;
        let cv_v = self.coerce_value(cv_expr.span, cv_v, CgTy::Ref)?;
        let Some(cv_raw) = cv_v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.CondVar.notifyAll receiver value",
                at: cv_expr.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(cv_ptr) = cv_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.CondVar.notifyAll receiver type",
                at: cv_expr.span.into(),
            });
        };

        let rt = self.declare_runtime_sync_condvar_notify_all();
        let _ = self.builder.build_call(rt, &[cv_ptr.into()], "sync_condvar_notify_all")?;
        Ok(CgValue::unit())
    }

    fn codegen_sysroot_sync_once_create(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if !args.is_empty() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.onceCreate arity mismatch",
                at: span.into(),
            });
        }

        let rt = self.declare_runtime_sync_once_create();
        let call = self.builder.build_call(rt, &[], "sync_once_create")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.onceCreate return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.onceCreate return type",
                at: span.into(),
            });
        };

        Ok(CgValue {
            ty: CgTy::Ref,
            value: Some(ptr.into()),
        })
    }

    fn codegen_sysroot_sync_once_is_done(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.Once.isDone arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(recv_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.Once.isDone named arg (receiver)",
                at: span.into(),
            });
        };

        let recv_v = self.codegen_expr_in_expected_context(recv_expr, Some(CgTy::Ref))?;
        let recv_v = self.coerce_value(recv_expr.span, recv_v, CgTy::Ref)?;
        let Some(recv_raw) = recv_v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.Once.isDone receiver value",
                at: recv_expr.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(recv_ptr) = recv_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.Once.isDone receiver type",
                at: recv_expr.span.into(),
            });
        };

        let rt = self.declare_runtime_sync_once_is_done();
        let call = self
            .builder
            .build_call(rt, &[recv_ptr.into()], "sync_once_is_done")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.Once.isDone return value",
                at: span.into(),
            })?;
        let BasicValueEnum::IntValue(done_i1) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.Once.isDone return type",
                at: span.into(),
            });
        };

        Ok(CgValue::bool(done_i1))
    }

    fn codegen_sysroot_sync_once_run(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        // `fun Once.run(block: () -> Unit): Unit`：`args = [once, block]`
        if args.len() != 2 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.Once.run arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(once_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.Once.run named arg (receiver)",
                at: span.into(),
            });
        };
        let hir::CallArg::Positional(block_expr) = &args[1] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.Once.run named arg (block)",
                at: span.into(),
            });
        };

        let once_v = self.codegen_expr_in_expected_context(once_expr, Some(CgTy::Ref))?;
        let once_v = self.coerce_value(once_expr.span, once_v, CgTy::Ref)?;
        let Some(once_raw) = once_v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.Once.run receiver value",
                at: once_expr.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(once_ptr) = once_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.Once.run receiver type",
                at: once_expr.span.into(),
            });
        };

        let block_v = match &block_expr.kind {
            hir::ExprKind::Closure(closure) => {
                // 说明：
                // - `Once.run` 的参数类型在 sysroot 中固定为 `() -> Unit`；
                // - 但 early stage 的 `fun_index` 只包含“本编译单元内有 body 的函数”，不含 sysroot 声明；
                // - 同时 HIR v0 对 closure expr 的 `ty` 也不总是可用作 expected type（需要 MIR/CFG 才能更稳）。
                //
                // 因此这里从 `TypeStore` 中查找一个“无参、返回 Unit、Pure”的函数类型作为 expected context。
                let expected_fun_ty =
                    self.lookup_pure_unit_closure_type().ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "sync.Once.run block fun type",
                        at: block_expr.span.into(),
                    })?;
                self.codegen_closure_expr(block_expr.span, closure, expected_fun_ty)?
            }
            _ => self.codegen_expr(block_expr)?,
        };
        let block_v = self.coerce_value(block_expr.span, block_v, CgTy::Ref)?;
        let Some(block_raw) = block_v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.Once.run block value",
                at: block_expr.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(block_obj_i8) = block_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.Once.run block type",
                at: block_expr.span.into(),
            });
        };

        // 抽取 closure object：`{ header, env_ptr, fn_ptr }`，把 env 与 typed fn 指针传给 runtime。
        let closure_ty = self.llvm_closure_object_type();
        let closure_ptr_ty = closure_ty.ptr_type(self.gc_address_space());
        let closure_ptr =
            self.builder
                .build_pointer_cast(block_obj_i8, closure_ptr_ty, "once_block_ptr")?;

        let i8_ptr_ty = self.llvm_i8_ptr_type();
        let env_gep =
            self.builder
                .build_struct_gep(closure_ty, closure_ptr, 1, "once_env_gep")?;
        let fn_gep =
            self.builder
                .build_struct_gep(closure_ty, closure_ptr, 2, "once_fn_gep")?;

        let env_ptr = self
            .builder
            .build_load(i8_ptr_ty, env_gep, "once_env")?
            .into_pointer_value();
        let fn_ptr_raw = self
            .builder
            .build_load(i8_ptr_ty, fn_gep, "once_fn_raw")?
            .into_pointer_value();

        let init_fn_ty = self
            .context
            .void_type()
            .fn_type(&[i8_ptr_ty.into()], false);
        let init_fn_ptr_ty = init_fn_ty.ptr_type(AddressSpace::default());
        let init_fn_ptr = self.builder.build_pointer_cast(
            fn_ptr_raw,
            init_fn_ptr_ty,
            "once_fn_typed",
        )?;

        let rt = self.declare_runtime_sync_once_run();
        let _ = self.builder.build_call(
            rt,
            &[once_ptr.into(), env_ptr.into(), init_fn_ptr.into()],
            "sync_once_run",
        )?;
        Ok(CgValue::unit())
    }

    fn codegen_sysroot_sync_destroy(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.destroy arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(recv_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.destroy named arg (receiver)",
                at: span.into(),
            });
        };

        // `destroy` 为 overload set：根据 receiver 的名义类型分派到不同 runtime 符号。
        //
        // 注意：HIR dump 当前阶段不保证 `expr.ty` 对 call/varref 总是精确，因此这里优先尝试
        // 从 local 绑定的 `hir_ty` 获取类型信息。
        let recv_hir_ty = match &recv_expr.kind {
            hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) => {
                self.env.get(*id).and_then(|l| l.hir_ty)
            }
            _ => Some(recv_expr.ty),
        };

        let Some(recv_hir_ty) = recv_hir_ty else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.destroy receiver hir type",
                at: recv_expr.span.into(),
            });
        };

        let TypeKind::Ref(RefTypeKind::Nominal(nominal)) = self.types.kind(recv_hir_ty) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.destroy receiver type kind",
                at: recv_expr.span.into(),
            });
        };

        let recv_v = self.codegen_expr_in_expected_context(recv_expr, Some(CgTy::Ref))?;
        let recv_v = self.coerce_value(recv_expr.span, recv_v, CgTy::Ref)?;
        let Some(recv_raw) = recv_v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.destroy receiver value",
                at: recv_expr.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(recv_ptr) = recv_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sync.destroy receiver type",
                at: recv_expr.span.into(),
            });
        };

        let rt = match nominal.fqn.as_str() {
            "scoop.sync.Mutex" => self.declare_runtime_sync_mutex_destroy(),
            "scoop.sync.CondVar" => self.declare_runtime_sync_condvar_destroy(),
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "sync.destroy receiver nominal",
                    at: recv_expr.span.into(),
                });
            }
        };
        let _ = self
            .builder
            .build_call(rt, &[recv_ptr.into()], "sync_destroy")?;
        Ok(CgValue::unit())
    }

    // --- std v3：thread（T1319c） ---

    fn codegen_sysroot_thread_spawn(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "thread.threadSpawn arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(block_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "thread.threadSpawn named arg (block)",
                at: span.into(),
            });
        };

        let block_v = match &block_expr.kind {
            hir::ExprKind::Closure(closure) => {
                // 说明：
                // - `thread.spawn` 的参数类型在 sysroot 中固定为 `() -> Unit`；
                // - 与 `sync.Once.run` 一致：为了在 early stage 稳定 codegen，这里从 `TypeStore` 中
                //   查找一个“无参、返回 Unit、Pure”的函数类型作为 expected context。
                let expected_fun_ty = self
                    .lookup_pure_unit_closure_type()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "thread.threadSpawn block fun type",
                        at: block_expr.span.into(),
                    })?;
                self.codegen_closure_expr(block_expr.span, closure, expected_fun_ty)?
            }
            _ => self.codegen_expr(block_expr)?,
        };
        let block_v = self.coerce_value(block_expr.span, block_v, CgTy::Ref)?;
        let Some(block_raw) = block_v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "thread.threadSpawn block value",
                at: block_expr.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(block_obj_i8) = block_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "thread.threadSpawn block type",
                at: block_expr.span.into(),
            });
        };

        // 抽取 closure object：`{ header, env_ptr, fn_ptr }`，把 env 与 typed fn 指针传给 runtime。
        let closure_ty = self.llvm_closure_object_type();
        let closure_ptr_ty = closure_ty.ptr_type(self.gc_address_space());
        let closure_ptr =
            self.builder
                .build_pointer_cast(block_obj_i8, closure_ptr_ty, "thread_block_ptr")?;

        let i8_ptr_ty = self.llvm_i8_ptr_type();
        let env_gep =
            self.builder
                .build_struct_gep(closure_ty, closure_ptr, 1, "thread_env_gep")?;
        let fn_gep =
            self.builder
                .build_struct_gep(closure_ty, closure_ptr, 2, "thread_fn_gep")?;

        let env_ptr = self
            .builder
            .build_load(i8_ptr_ty, env_gep, "thread_env")?
            .into_pointer_value();
        let fn_ptr_raw = self
            .builder
            .build_load(i8_ptr_ty, fn_gep, "thread_fn_raw")?
            .into_pointer_value();

        let start_fn_ty = self
            .context
            .void_type()
            .fn_type(&[i8_ptr_ty.into()], false);
        let start_fn_ptr_ty = start_fn_ty.ptr_type(AddressSpace::default());
        let start_fn_ptr = self.builder.build_pointer_cast(
            fn_ptr_raw,
            start_fn_ptr_ty,
            "thread_fn_typed",
        )?;

        let rt = self.declare_runtime_thread_spawn();
        let call = self.builder.build_call(
            rt,
            &[env_ptr.into(), start_fn_ptr.into()],
            "thread_spawn",
        )?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "thread.threadSpawn return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(thread_ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "thread.threadSpawn return type",
                at: span.into(),
            });
        };

        Ok(CgValue {
            ty: CgTy::Ref,
            value: Some(thread_ptr.into()),
        })
    }

    fn codegen_sysroot_thread_join(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "thread.Thread.join arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(recv_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "thread.Thread.join named arg (receiver)",
                at: span.into(),
            });
        };

        let recv_v = self.codegen_expr_in_expected_context(recv_expr, Some(CgTy::Ref))?;
        let recv_v = self.coerce_value(recv_expr.span, recv_v, CgTy::Ref)?;
        let Some(recv_raw) = recv_v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "thread.Thread.join receiver value",
                at: recv_expr.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(recv_ptr) = recv_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "thread.Thread.join receiver type",
                at: recv_expr.span.into(),
            });
        };

        let rt = self.declare_runtime_thread_join();
        let _ = self
            .builder
            .build_call(rt, &[recv_ptr.into()], "thread_join")?;
        Ok(CgValue::unit())
    }

    fn codegen_sysroot_thread_sleep_millis(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "thread.sleepMillis arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(ms_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "thread.sleepMillis named arg",
                at: span.into(),
            });
        };

        let value_word = IntTy {
            bits: self.host.word_bit_width(),
            signed: true,
        };
        let ms_v = self.codegen_expr_in_expected_context(ms_expr, Some(CgTy::Int(value_word)))?;
        let ms_v = self.coerce_value(ms_expr.span, ms_v, CgTy::Int(value_word))?;
        let (ms_raw, ms_from) = ms_v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "thread.sleepMillis ms value",
            at: ms_expr.span.into(),
        })?;

        let ms_to = IntTy {
            bits: 64,
            signed: true,
        };
        let ms_i64 = self.cast_int(ms_raw, ms_from, ms_to)?;

        let rt = self.declare_runtime_thread_sleep_millis();
        let _ = self
            .builder
            .build_call(rt, &[ms_i64.into()], "thread_sleep_millis")?;
        Ok(CgValue::unit())
    }

    fn codegen_sysroot_thread_yield(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if !args.is_empty() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "thread.yield arity mismatch",
                at: span.into(),
            });
        }

        let rt = self.declare_runtime_thread_yield();
        let _ = self.builder.build_call(rt, &[], "thread_yield")?;
        Ok(CgValue::unit())
    }

    fn codegen_sysroot_thread_current_id(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if !args.is_empty() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "thread.currentId arity mismatch",
                at: span.into(),
            });
        }

        let rt = self.declare_runtime_thread_current_id();
        let call = self
            .builder
            .build_call(rt, &[], "thread_current_id")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "thread.currentId return value",
                at: span.into(),
            })?;
        let BasicValueEnum::IntValue(raw_i64) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "thread.currentId return type",
                at: span.into(),
            });
        };

        let from = IntTy {
            bits: 64,
            signed: true,
        };
        let to = IntTy {
            bits: self.host.word_bit_width(),
            signed: true,
        };
        let casted = self.cast_int(raw_i64, from, to)?;
        Ok(CgValue::int(casted, to))
    }

    // --- std v3：channels（T1319d） ---

    fn codegen_sysroot_channels_channel_create(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if !args.is_empty() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "channels.channelCreate arity mismatch",
                at: span.into(),
            });
        }

        let rt = self.declare_runtime_channels_channel_create();
        let call = self
            .builder
            .build_call(rt, &[], "channels_channel_create")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "channels.channelCreate return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "channels.channelCreate return type",
                at: span.into(),
            });
        };

        Ok(CgValue {
            ty: CgTy::Ref,
            value: Some(ptr.into()),
        })
    }

    fn codegen_sysroot_channels_send(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 2 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "channels.Channel.send arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(channel_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "channels.Channel.send named arg (receiver)",
                at: span.into(),
            });
        };
        let hir::CallArg::Positional(value_expr) = &args[1] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "channels.Channel.send named arg (value)",
                at: span.into(),
            });
        };

        let channel_v = self.codegen_expr_in_expected_context(channel_expr, Some(CgTy::Ref))?;
        let channel_v = self.coerce_value(channel_expr.span, channel_v, CgTy::Ref)?;
        let Some(channel_raw) = channel_v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "channels.Channel.send receiver value",
                at: channel_expr.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(channel_ptr) = channel_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "channels.Channel.send receiver type",
                at: channel_expr.span.into(),
            });
        };

        // 优先从 receiver 的静态类型恢复 `T`，以便对 `value` 施加期望类型与编码方式。
        let elem_cg = match self.types.kind(channel_expr.ty) {
            TypeKind::Ref(RefTypeKind::Nominal(nominal))
                if nominal.fqn == "scoop.channels.Channel" && nominal.args.len() == 1 =>
            {
                self.cg_ty_of(nominal.args[0]).filter(|ty| {
                    matches!(ty, CgTy::Unit | CgTy::Bool | CgTy::Int(_) | CgTy::String | CgTy::Ref)
                })
            }
            _ => None,
        };

        let value_v = match elem_cg {
            Some(elem_cg) => {
                let v = self.codegen_expr_in_expected_context(value_expr, Some(elem_cg))?;
                self.coerce_value(value_expr.span, v, elem_cg)?
            }
            None => self.codegen_expr(value_expr)?,
        };
        let word_u64 = self.coerce_u64_word(value_expr.span, value_v)?;

        let rt = self.declare_runtime_channels_send_u64();
        let call = self.builder.build_call(
            rt,
            &[channel_ptr.into(), word_u64.into()],
            "channels_send_u64",
        )?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "channels.Channel.send return value",
                at: span.into(),
            })?;
        let BasicValueEnum::IntValue(ok_i32) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "channels.Channel.send return type",
                at: span.into(),
            });
        };

        let ok_cond = self.builder.build_int_compare(
            IntPredicate::NE,
            ok_i32,
            self.context.i32_type().const_zero(),
            "channels_send_ok",
        )?;
        Ok(CgValue::bool(ok_cond))
    }

    fn codegen_sysroot_channels_recv(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
        expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "channels.Channel.recv arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(channel_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "channels.Channel.recv named arg (receiver)",
                at: span.into(),
            });
        };

        let channel_v = self.codegen_expr_in_expected_context(channel_expr, Some(CgTy::Ref))?;
        let channel_v = self.coerce_value(channel_expr.span, channel_v, CgTy::Ref)?;
        let Some(channel_raw) = channel_v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "channels.Channel.recv receiver value",
                at: channel_expr.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(channel_ptr) = channel_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "channels.Channel.recv receiver type",
                at: channel_expr.span.into(),
            });
        };

        // 恢复 `T`：优先从 receiver 的静态类型 `Channel<T>` 得到；若无法恢复，则退化使用 expected context
        //（例如 `val v: Int? = ch.recv()`）从 `Option<T>` 里反推 `T`。
        let (option_ty, elem_ty) = match self.types.kind(channel_expr.ty) {
            TypeKind::Ref(RefTypeKind::Nominal(nominal))
                if nominal.fqn == "scoop.channels.Channel" && nominal.args.len() == 1 =>
            {
                let elem_ty = nominal.args[0];
                let option_ty = self
                    .types
                    .iter_ids()
                    .find(|id| match self.types.kind(*id) {
                        TypeKind::Value(ValueTypeKind::Option(inner)) => *inner == elem_ty,
                        _ => false,
                    })
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "channels.Channel.recv return Option<T> type",
                        at: span.into(),
                    })?;
                (option_ty, elem_ty)
            }
            _ => match expected {
                Some(CgTy::Enum(option_ty)) => match self.types.kind(option_ty) {
                    TypeKind::Value(ValueTypeKind::Option(inner)) => (option_ty, *inner),
                    _ => {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "channels.Channel.recv expected Option<T>",
                            at: span.into(),
                        });
                    }
                },
                _ => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "channels.Channel.recv receiver nominal",
                        at: channel_expr.span.into(),
                    });
                }
            },
        };

        // gate：确保元素是 “u64 word 可编码”的类型（与 `coerce_u64_word` 对齐）。
        let elem_cg = self.cg_ty_of(elem_ty).filter(|ty| {
            matches!(ty, CgTy::Unit | CgTy::Bool | CgTy::Int(_) | CgTy::String | CgTy::Ref)
        });
        if elem_cg.is_none() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "channels.Channel.recv element type",
                at: channel_expr.span.into(),
            });
        }

        // `uint32_t scoop_channels_recv_u64(void* channel, uint64_t* out_value)`
        let i64_ty = self.context.i64_type();
        let out_ptr = self.create_entry_alloca_raw(span, "channels_recv_out", i64_ty.into())?;

        let rt = self.declare_runtime_channels_recv_u64();
        let call = self.builder.build_call(
            rt,
            &[channel_ptr.into(), out_ptr.into()],
            "channels_recv_u64",
        )?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "channels.Channel.recv return value",
                at: span.into(),
            })?;
        let BasicValueEnum::IntValue(ok_i32) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "channels.Channel.recv return type",
                at: span.into(),
            });
        };

        let ok_cond = self.builder.build_int_compare(
            IntPredicate::NE,
            ok_i32,
            self.context.i32_type().const_zero(),
            "channels_recv_ok",
        )?;

        let option_cg = CgTy::Enum(option_ty);
        let option_llvm_ty = self.llvm_basic_type_of(span, option_cg)?;

        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: span.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: span.into(),
            })?;

        let some_bb = self.context.append_basic_block(func, "channels_recv_some");
        let none_bb = self.context.append_basic_block(func, "channels_recv_none");
        let merge_bb = self.context.append_basic_block(func, "channels_recv_merge");

        self.builder
            .build_conditional_branch(ok_cond, some_bb, none_bb)?;

        // some branch：读取 word，构造 `Some(value)`。
        self.builder.position_at_end(some_bb);
        let word_u64 = self
            .builder
            .build_load(i64_ty, out_ptr, "channels_recv_word")?
            .into_int_value();
        let from = IntTy {
            bits: 64,
            signed: false,
        };
        let payload_ty = self.enum_payload_ty();
        let payload_word = self.cast_int(word_u64, from, payload_ty)?;
        let some_v = self.build_enum_value(span, option_ty, 0, Some(payload_word))?;
        let some_raw = some_v.value.ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "channels.Channel.recv Some value",
            at: span.into(),
        })?;
        self.builder.build_unconditional_branch(merge_bb)?;
        let some_end = self
            .builder
            .get_insert_block()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no insert block",
                at: span.into(),
            })?;

        // none branch：构造 `None`。
        self.builder.position_at_end(none_bb);
        let none_v = self.build_enum_value(span, option_ty, 1, None)?;
        let none_raw = none_v.value.ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "channels.Channel.recv None value",
            at: span.into(),
        })?;
        self.builder.build_unconditional_branch(merge_bb)?;
        let none_end = self
            .builder
            .get_insert_block()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no insert block",
                at: span.into(),
            })?;

        // merge：phi 合并结果。
        self.builder.position_at_end(merge_bb);
        let phi = self.builder.build_phi(option_llvm_ty, "channels_recv_phi")?;
        phi.add_incoming(&[(&some_raw, some_end), (&none_raw, none_end)]);

        Ok(CgValue {
            ty: option_cg,
            value: Some(phi.as_basic_value()),
        })
    }

    fn codegen_sysroot_channels_close(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "channels.Channel.close arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(channel_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "channels.Channel.close named arg (receiver)",
                at: span.into(),
            });
        };

        let channel_v = self.codegen_expr_in_expected_context(channel_expr, Some(CgTy::Ref))?;
        let channel_v = self.coerce_value(channel_expr.span, channel_v, CgTy::Ref)?;
        let Some(channel_raw) = channel_v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "channels.Channel.close receiver value",
                at: channel_expr.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(channel_ptr) = channel_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "channels.Channel.close receiver type",
                at: channel_expr.span.into(),
            });
        };

        let rt = self.declare_runtime_channels_close();
        let _ = self
            .builder
            .build_call(rt, &[channel_ptr.into()], "channels_close")?;
        Ok(CgValue::unit())
    }

    // --- std v3：task/executor（T1319e） ---

    fn codegen_sysroot_task_executor_create(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if !args.is_empty() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "task.executorCreate arity mismatch",
                at: span.into(),
            });
        }

        let handle_word = IntTy {
            bits: self.host.word_bit_width(),
            signed: false,
        };

        let rt = self.declare_runtime_executor_create();
        let call = self.builder.build_call(rt, &[], "executor_create")?;
        let raw = call.try_as_basic_value().basic().ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "task.executorCreate return value",
                at: span.into(),
            },
        )?;
        let BasicValueEnum::IntValue(handle_u64) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "task.executorCreate return type",
                at: span.into(),
            });
        };

        let handle_word_val = self.cast_int(
            handle_u64,
            IntTy {
                bits: 64,
                signed: false,
            },
            handle_word,
        )?;
        Ok(CgValue::int(handle_word_val, handle_word))
    }

    fn codegen_sysroot_task_executor_destroy(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "task.Executor.destroy arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(executor_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "task.Executor.destroy named arg (receiver)",
                at: span.into(),
            });
        };

        let handle_word = IntTy {
            bits: self.host.word_bit_width(),
            signed: false,
        };

        let executor_v =
            self.codegen_expr_in_expected_context(executor_expr, Some(CgTy::Int(handle_word)))?;
        let executor_v = self.coerce_value(executor_expr.span, executor_v, CgTy::Int(handle_word))?;
        let (raw_handle, from) = executor_v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "task.Executor.destroy receiver value",
            at: executor_expr.span.into(),
        })?;

        let handle_u64 = self.cast_int(
            raw_handle,
            from,
            IntTy {
                bits: 64,
                signed: false,
            },
        )?;

        let rt = self.declare_runtime_executor_destroy();
        let _ = self
            .builder
            .build_call(rt, &[handle_u64.into()], "executor_destroy")?;
        Ok(CgValue::unit())
    }

    fn codegen_sysroot_task_executor_debug_pending_count(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "task.Executor.debugPendingCount arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(executor_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "task.Executor.debugPendingCount named arg (receiver)",
                at: span.into(),
            });
        };

        let handle_word = IntTy {
            bits: self.host.word_bit_width(),
            signed: false,
        };
        let value_word = IntTy {
            bits: self.host.word_bit_width(),
            signed: true,
        };

        let executor_v =
            self.codegen_expr_in_expected_context(executor_expr, Some(CgTy::Int(handle_word)))?;
        let executor_v = self.coerce_value(executor_expr.span, executor_v, CgTy::Int(handle_word))?;
        let (raw_handle, from) = executor_v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "task.Executor.debugPendingCount receiver value",
            at: executor_expr.span.into(),
        })?;

        let handle_u64 = self.cast_int(
            raw_handle,
            from,
            IntTy {
                bits: 64,
                signed: false,
            },
        )?;

        let rt = self.declare_runtime_executor_debug_pending_count();
        let call = self.builder.build_call(
            rt,
            &[handle_u64.into()],
            "executor_debug_pending_count",
        )?;
        let raw = call.try_as_basic_value().basic().ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "task.Executor.debugPendingCount return value",
                at: span.into(),
            },
        )?;
        let BasicValueEnum::IntValue(n_u64) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "task.Executor.debugPendingCount return type",
                at: span.into(),
            });
        };

        let n_word = self.cast_int(
            n_u64,
            IntTy {
                bits: 64,
                signed: false,
            },
            value_word,
        )?;
        Ok(CgValue::int(n_word, value_word))
    }

    fn codegen_sysroot_task_executor_run_next(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "task.Executor.runNext arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(executor_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "task.Executor.runNext named arg (receiver)",
                at: span.into(),
            });
        };

        let handle_word = IntTy {
            bits: self.host.word_bit_width(),
            signed: false,
        };

        let executor_v =
            self.codegen_expr_in_expected_context(executor_expr, Some(CgTy::Int(handle_word)))?;
        let executor_v = self.coerce_value(executor_expr.span, executor_v, CgTy::Int(handle_word))?;
        let (raw_handle, from) = executor_v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "task.Executor.runNext receiver value",
            at: executor_expr.span.into(),
        })?;

        let handle_u64 = self.cast_int(
            raw_handle,
            from,
            IntTy {
                bits: 64,
                signed: false,
            },
        )?;

        let rt = self.declare_runtime_executor_run_next();
        let call = self
            .builder
            .build_call(rt, &[handle_u64.into()], "executor_run_next")?;
        let raw = call.try_as_basic_value().basic().ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "task.Executor.runNext return value",
                at: span.into(),
            },
        )?;
        let BasicValueEnum::IntValue(ok_u64) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "task.Executor.runNext return type",
                at: span.into(),
            });
        };

        let ok_cond = self.builder.build_int_compare(
            IntPredicate::NE,
            ok_u64,
            self.context.i64_type().const_zero(),
            "executor_run_next_ok",
        )?;
        Ok(CgValue::bool(ok_cond))
    }

    fn codegen_sysroot_task_executor_run_until_idle(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 2 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "task.Executor.runUntilIdle arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(executor_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "task.Executor.runUntilIdle named arg (receiver)",
                at: span.into(),
            });
        };
        let hir::CallArg::Positional(max_steps_expr) = &args[1] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "task.Executor.runUntilIdle named arg (maxSteps)",
                at: span.into(),
            });
        };

        let handle_word = IntTy {
            bits: self.host.word_bit_width(),
            signed: false,
        };
        let value_word = IntTy {
            bits: self.host.word_bit_width(),
            signed: true,
        };

        let executor_v =
            self.codegen_expr_in_expected_context(executor_expr, Some(CgTy::Int(handle_word)))?;
        let executor_v = self.coerce_value(executor_expr.span, executor_v, CgTy::Int(handle_word))?;
        let (raw_handle, from) = executor_v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "task.Executor.runUntilIdle receiver value",
            at: executor_expr.span.into(),
        })?;
        let handle_u64 = self.cast_int(
            raw_handle,
            from,
            IntTy {
                bits: 64,
                signed: false,
            },
        )?;

        let max_steps_v =
            self.codegen_expr_in_expected_context(max_steps_expr, Some(CgTy::Int(value_word)))?;
        let max_steps_v =
            self.coerce_value(max_steps_expr.span, max_steps_v, CgTy::Int(value_word))?;
        let (raw_max_steps, from) =
            max_steps_v
                .as_int()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "task.Executor.runUntilIdle maxSteps value",
                    at: max_steps_expr.span.into(),
                })?;
        let max_steps_u64 = self.cast_int(
            raw_max_steps,
            from,
            IntTy {
                bits: 64,
                signed: false,
            },
        )?;

        let rt = self.declare_runtime_executor_run_until_idle();
        let call = self.builder.build_call(
            rt,
            &[handle_u64.into(), max_steps_u64.into()],
            "executor_run_until_idle",
        )?;
        let raw = call.try_as_basic_value().basic().ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "task.Executor.runUntilIdle return value",
                at: span.into(),
            },
        )?;
        let BasicValueEnum::IntValue(ran_u64) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "task.Executor.runUntilIdle return type",
                at: span.into(),
            });
        };

        let ran_word = self.cast_int(
            ran_u64,
            IntTy {
                bits: 64,
                signed: false,
            },
            value_word,
        )?;
        Ok(CgValue::int(ran_word, value_word))
    }

    fn codegen_sysroot_task_create(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "task.taskCreate arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(block_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "task.taskCreate named arg (block)",
                at: span.into(),
            });
        };

        let expected_fun_ty =
            self.lookup_pure_int_closure_type()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "task.taskCreate block fun type",
                    at: block_expr.span.into(),
                })?;

        let block_v = match &block_expr.kind {
            hir::ExprKind::Closure(closure) => {
                self.codegen_closure_expr(block_expr.span, closure, expected_fun_ty)?
            }
            _ => self.codegen_expr(block_expr)?,
        };
        let block_v = self.coerce_value(block_expr.span, block_v, CgTy::Ref)?;
        let Some(block_raw) = block_v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "task.taskCreate block value",
                at: block_expr.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(block_obj_i8) = block_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "task.taskCreate block type",
                at: block_expr.span.into(),
            });
        };

        // 抽取 closure object：`{ header, env_ptr, fn_ptr }`，把 env 与 typed fn 指针传给 runtime。
        let closure_ty = self.llvm_closure_object_type();
        let closure_ptr_ty = closure_ty.ptr_type(self.gc_address_space());
        let closure_ptr =
            self.builder
                .build_pointer_cast(block_obj_i8, closure_ptr_ty, "task_block_ptr")?;

        let i8_ptr_ty = self.llvm_i8_ptr_type();
        let env_gep = self
            .builder
            .build_struct_gep(closure_ty, closure_ptr, 1, "task_env_gep")?;
        let fn_gep = self
            .builder
            .build_struct_gep(closure_ty, closure_ptr, 2, "task_fn_gep")?;

        let env_ptr = self
            .builder
            .build_load(i8_ptr_ty, env_gep, "task_env")?
            .into_pointer_value();
        let fn_ptr_raw = self
            .builder
            .build_load(i8_ptr_ty, fn_gep, "task_fn_raw")?
            .into_pointer_value();

        // `uint64_t (*)(void*)`（LLVM: `i64 (i8*)`）
        let body_fn_ty = self.context.i64_type().fn_type(&[i8_ptr_ty.into()], false);
        let body_fn_ptr_ty = body_fn_ty.ptr_type(AddressSpace::default());
        let body_fn_ptr = self.builder.build_pointer_cast(
            fn_ptr_raw,
            body_fn_ptr_ty,
            "task_body_fn_typed",
        )?;

        let rt = self.declare_runtime_task_u64_create();
        let call = self.builder.build_call(
            rt,
            &[body_fn_ptr.into(), env_ptr.into()],
            "task_u64_create",
        )?;
        let raw = call.try_as_basic_value().basic().ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "task.taskCreate return value",
                at: span.into(),
            },
        )?;
        let BasicValueEnum::IntValue(task_handle_u64) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "task.taskCreate return type",
                at: span.into(),
            });
        };

        let handle_word = IntTy {
            bits: self.host.word_bit_width(),
            signed: false,
        };
        let task_handle_word = self.cast_int(
            task_handle_u64,
            IntTy {
                bits: 64,
                signed: false,
            },
            handle_word,
        )?;
        Ok(CgValue::int(task_handle_word, handle_word))
    }

    fn codegen_sysroot_task_create_manual(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if !args.is_empty() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "task.taskCreateManual arity mismatch",
                at: span.into(),
            });
        }

        let i8_ptr_ty = self.context.i8_type().ptr_type(AddressSpace::default());
        let body_fn_ty = self.context.i64_type().fn_type(&[i8_ptr_ty.into()], false);
        let body_fn_ptr_ty = body_fn_ty.ptr_type(AddressSpace::default());

        let rt = self.declare_runtime_task_u64_create();
        let call = self.builder.build_call(
            rt,
            &[
                body_fn_ptr_ty.const_null().into(),
                i8_ptr_ty.const_null().into(),
            ],
            "task_u64_create_manual",
        )?;
        let raw = call.try_as_basic_value().basic().ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "task.taskCreateManual return value",
                at: span.into(),
            },
        )?;
        let BasicValueEnum::IntValue(task_handle_u64) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "task.taskCreateManual return type",
                at: span.into(),
            });
        };

        let handle_word = IntTy {
            bits: self.host.word_bit_width(),
            signed: false,
        };
        let task_handle_word = self.cast_int(
            task_handle_u64,
            IntTy {
                bits: 64,
                signed: false,
            },
            handle_word,
        )?;
        Ok(CgValue::int(task_handle_word, handle_word))
    }

    fn codegen_sysroot_task_state(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "task.Task.state arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(task_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "task.Task.state named arg (receiver)",
                at: span.into(),
            });
        };

        let handle_word = IntTy {
            bits: self.host.word_bit_width(),
            signed: false,
        };
        let value_word = IntTy {
            bits: self.host.word_bit_width(),
            signed: true,
        };

        let task_v = self.codegen_expr_in_expected_context(task_expr, Some(CgTy::Int(handle_word)))?;
        let task_v = self.coerce_value(task_expr.span, task_v, CgTy::Int(handle_word))?;
        let (raw_handle, from) = task_v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "task.Task.state receiver value",
            at: task_expr.span.into(),
        })?;

        let handle_u64 = self.cast_int(
            raw_handle,
            from,
            IntTy {
                bits: 64,
                signed: false,
            },
        )?;

        let rt = self.declare_runtime_task_u64_state();
        let call = self
            .builder
            .build_call(rt, &[handle_u64.into()], "task_u64_state")?;
        let raw = call.try_as_basic_value().basic().ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "task.Task.state return value",
                at: span.into(),
            },
        )?;
        let BasicValueEnum::IntValue(state_u32) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "task.Task.state return type",
                at: span.into(),
            });
        };

        let state_word = self.cast_int(
            state_u32,
            IntTy {
                bits: 32,
                signed: false,
            },
            value_word,
        )?;
        Ok(CgValue::int(state_word, value_word))
    }

    fn codegen_sysroot_task_result(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "task.Task.result arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(task_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "task.Task.result named arg (receiver)",
                at: span.into(),
            });
        };

        let handle_word = IntTy {
            bits: self.host.word_bit_width(),
            signed: false,
        };
        let value_word = IntTy {
            bits: self.host.word_bit_width(),
            signed: true,
        };

        let task_v = self.codegen_expr_in_expected_context(task_expr, Some(CgTy::Int(handle_word)))?;
        let task_v = self.coerce_value(task_expr.span, task_v, CgTy::Int(handle_word))?;
        let (raw_handle, from) = task_v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "task.Task.result receiver value",
            at: task_expr.span.into(),
        })?;

        let handle_u64 = self.cast_int(
            raw_handle,
            from,
            IntTy {
                bits: 64,
                signed: false,
            },
        )?;

        let rt = self.declare_runtime_task_u64_result();
        let call = self
            .builder
            .build_call(rt, &[handle_u64.into()], "task_u64_result")?;
        let raw = call.try_as_basic_value().basic().ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "task.Task.result return value",
                at: span.into(),
            },
        )?;
        let BasicValueEnum::IntValue(result_u64) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "task.Task.result return type",
                at: span.into(),
            });
        };

        let result_word = self.cast_int(
            result_u64,
            IntTy {
                bits: 64,
                signed: false,
            },
            value_word,
        )?;
        Ok(CgValue::int(result_word, value_word))
    }

    fn codegen_sysroot_task_try_start(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 2 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "task.Task.tryStart arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(task_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "task.Task.tryStart named arg (receiver)",
                at: span.into(),
            });
        };
        let hir::CallArg::Positional(executor_expr) = &args[1] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "task.Task.tryStart named arg (executor)",
                at: span.into(),
            });
        };

        let handle_word = IntTy {
            bits: self.host.word_bit_width(),
            signed: false,
        };

        let task_v = self.codegen_expr_in_expected_context(task_expr, Some(CgTy::Int(handle_word)))?;
        let task_v = self.coerce_value(task_expr.span, task_v, CgTy::Int(handle_word))?;
        let (raw_task, from) = task_v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "task.Task.tryStart receiver value",
            at: task_expr.span.into(),
        })?;
        let task_u64 = self.cast_int(
            raw_task,
            from,
            IntTy {
                bits: 64,
                signed: false,
            },
        )?;

        let executor_v =
            self.codegen_expr_in_expected_context(executor_expr, Some(CgTy::Int(handle_word)))?;
        let executor_v =
            self.coerce_value(executor_expr.span, executor_v, CgTy::Int(handle_word))?;
        let (raw_exec, from) =
            executor_v
                .as_int()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "task.Task.tryStart executor value",
                    at: executor_expr.span.into(),
                })?;
        let exec_u64 = self.cast_int(
            raw_exec,
            from,
            IntTy {
                bits: 64,
                signed: false,
            },
        )?;

        let rt = self.declare_runtime_task_u64_try_start();
        let call = self.builder.build_call(
            rt,
            &[task_u64.into(), exec_u64.into()],
            "task_u64_try_start",
        )?;
        let raw = call.try_as_basic_value().basic().ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "task.Task.tryStart return value",
                at: span.into(),
            },
        )?;
        let BasicValueEnum::IntValue(ok_i32) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "task.Task.tryStart return type",
                at: span.into(),
            });
        };

        let ok_cond = self.builder.build_int_compare(
            IntPredicate::NE,
            ok_i32,
            self.context.i32_type().const_zero(),
            "task_try_start_ok",
        )?;
        Ok(CgValue::bool(ok_cond))
    }

    fn codegen_sysroot_task_complete(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 2 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "task.Task.complete arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(task_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "task.Task.complete named arg (receiver)",
                at: span.into(),
            });
        };
        let hir::CallArg::Positional(value_expr) = &args[1] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "task.Task.complete named arg (value)",
                at: span.into(),
            });
        };

        let handle_word = IntTy {
            bits: self.host.word_bit_width(),
            signed: false,
        };
        let value_word = IntTy {
            bits: self.host.word_bit_width(),
            signed: true,
        };

        let task_v = self.codegen_expr_in_expected_context(task_expr, Some(CgTy::Int(handle_word)))?;
        let task_v = self.coerce_value(task_expr.span, task_v, CgTy::Int(handle_word))?;
        let (raw_task, from) = task_v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "task.Task.complete receiver value",
            at: task_expr.span.into(),
        })?;
        let task_u64 = self.cast_int(
            raw_task,
            from,
            IntTy {
                bits: 64,
                signed: false,
            },
        )?;

        let value_v =
            self.codegen_expr_in_expected_context(value_expr, Some(CgTy::Int(value_word)))?;
        let value_v = self.coerce_value(value_expr.span, value_v, CgTy::Int(value_word))?;
        let (raw_value, from) =
            value_v
                .as_int()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "task.Task.complete value",
                    at: value_expr.span.into(),
                })?;
        let value_u64 = self.cast_int(
            raw_value,
            from,
            IntTy {
                bits: 64,
                signed: false,
            },
        )?;

        let rt = self.declare_runtime_task_u64_complete();
        let call = self.builder.build_call(
            rt,
            &[task_u64.into(), value_u64.into()],
            "task_u64_complete",
        )?;
        let raw = call.try_as_basic_value().basic().ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "task.Task.complete return value",
                at: span.into(),
            },
        )?;
        let BasicValueEnum::IntValue(ok_i32) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "task.Task.complete return type",
                at: span.into(),
            });
        };

        let ok_cond = self.builder.build_int_compare(
            IntPredicate::NE,
            ok_i32,
            self.context.i32_type().const_zero(),
            "task_complete_ok",
        )?;
        Ok(CgValue::bool(ok_cond))
    }

    fn codegen_sysroot_task_on_complete(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 3 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "task.Task.onComplete arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(task_expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "task.Task.onComplete named arg (receiver)",
                at: span.into(),
            });
        };
        let hir::CallArg::Positional(executor_expr) = &args[1] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "task.Task.onComplete named arg (executor)",
                at: span.into(),
            });
        };
        let hir::CallArg::Positional(k_expr) = &args[2] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "task.Task.onComplete named arg (continuation)",
                at: span.into(),
            });
        };

        let handle_word = IntTy {
            bits: self.host.word_bit_width(),
            signed: false,
        };

        let task_v = self.codegen_expr_in_expected_context(task_expr, Some(CgTy::Int(handle_word)))?;
        let task_v = self.coerce_value(task_expr.span, task_v, CgTy::Int(handle_word))?;
        let (raw_task, from) = task_v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "task.Task.onComplete receiver value",
            at: task_expr.span.into(),
        })?;
        let task_u64 = self.cast_int(
            raw_task,
            from,
            IntTy {
                bits: 64,
                signed: false,
            },
        )?;

        let executor_v =
            self.codegen_expr_in_expected_context(executor_expr, Some(CgTy::Int(handle_word)))?;
        let executor_v =
            self.coerce_value(executor_expr.span, executor_v, CgTy::Int(handle_word))?;
        let (raw_exec, from) =
            executor_v
                .as_int()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "task.Task.onComplete executor value",
                    at: executor_expr.span.into(),
                })?;
        let exec_u64 = self.cast_int(
            raw_exec,
            from,
            IntTy {
                bits: 64,
                signed: false,
            },
        )?;

        let k_v = self.codegen_expr_in_expected_context(k_expr, Some(CgTy::Ref))?;
        let k_v = self.coerce_value(k_expr.span, k_v, CgTy::Ref)?;
        let Some(k_raw) = k_v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "task.Task.onComplete continuation value",
                at: k_expr.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(k_ptr) = k_raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "task.Task.onComplete continuation type",
                at: k_expr.span.into(),
            });
        };

        let rt = self.declare_runtime_task_u64_on_complete_resume_u64();
        let call = self.builder.build_call(
            rt,
            &[task_u64.into(), exec_u64.into(), k_ptr.into()],
            "task_u64_on_complete",
        )?;
        let raw = call.try_as_basic_value().basic().ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "task.Task.onComplete return value",
                at: span.into(),
            },
        )?;
        let BasicValueEnum::IntValue(ok_i32) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "task.Task.onComplete return type",
                at: span.into(),
            });
        };

        let ok_cond = self.builder.build_int_compare(
            IntPredicate::NE,
            ok_i32,
            self.context.i32_type().const_zero(),
            "task_on_complete_ok",
        )?;
        Ok(CgValue::bool(ok_cond))
    }

    fn codegen_sysroot_array_builder_intrinsics(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        fqn: &str,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match fqn {
            "scoop.core.__scoop_array_builder_new" => {
                if !args.is_empty() {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "array_builder_new arity mismatch",
                        at: span.into(),
                    });
                }

                let rt = self.declare_runtime_array_builder_new();
                let call = self.builder.build_call(rt, &[], "array_builder_new")?;
                let raw = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "array_builder_new return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::PointerValue(ptr) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "array_builder_new return type",
                        at: span.into(),
                    });
                };
                Ok(CgValue {
                    ty: CgTy::Ref,
                    value: Some(ptr.into()),
                })
            }
            "scoop.core.__scoop_array_builder_push" => {
                if args.len() != 2 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "array_builder_push arity mismatch",
                        at: span.into(),
                    });
                }

                let hir::CallArg::Positional(builder_expr) = &args[0] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "array_builder_push builder named arg",
                        at: span.into(),
                    });
                };
                let hir::CallArg::Positional(value_expr) = &args[1] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "array_builder_push value named arg",
                        at: span.into(),
                    });
                };

                let builder_v =
                    self.codegen_expr_in_expected_context(builder_expr, Some(CgTy::Ref))?;
                let builder_v = self.coerce_value(builder_expr.span, builder_v, CgTy::Ref)?;
                let Some(builder_raw) = builder_v.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "array_builder_push builder value",
                        at: builder_expr.span.into(),
                    });
                };
                let BasicValueEnum::PointerValue(builder_ptr) = builder_raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "array_builder_push builder type",
                        at: builder_expr.span.into(),
                    });
                };

                let value_v = self.codegen_expr(value_expr)?;
                match value_v.ty {
                    CgTy::Ref | CgTy::String => {
                        // ref/string 元素：保持为 `addrspace(1)` 指针，避免 ptr->u64 编码（为 statepoint/stackmap 做准备）。
                        let v = self.coerce_value(value_expr.span, value_v, CgTy::Ref)?;
                        let Some(raw) = v.value else {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "array_builder_push ref value",
                                at: value_expr.span.into(),
                            });
                        };
                        let BasicValueEnum::PointerValue(ptr) = raw else {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "array_builder_push ref type",
                                at: value_expr.span.into(),
                            });
                        };

                        let rt = self.declare_runtime_array_builder_push_ref();
                        let _ = self.builder.build_call(
                            rt,
                            &[builder_ptr.into(), ptr.into()],
                            "array_builder_push_ref",
                        )?;
                    }
                    _ => {
                        // word 元素：沿用旧 ABI（u64）。
                        let word_u64 = self.coerce_u64_word(value_expr.span, value_v)?;
                        let rt = self.declare_runtime_array_builder_push_u64();
                        let _ = self.builder.build_call(
                            rt,
                            &[builder_ptr.into(), word_u64.into()],
                            "array_builder_push_u64",
                        )?;
                    }
                }
                Ok(CgValue::unit())
            }
            "scoop.core.__scoop_array_builder_build_array"
            | "scoop.core.__scoop_array_builder_build_mutable_array" => {
                if args.len() != 1 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "array_builder_build arity mismatch",
                        at: span.into(),
                    });
                }

                let hir::CallArg::Positional(builder_expr) = &args[0] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "array_builder_build builder named arg",
                        at: span.into(),
                    });
                };

                let builder_v =
                    self.codegen_expr_in_expected_context(builder_expr, Some(CgTy::Ref))?;
                let builder_v = self.coerce_value(builder_expr.span, builder_v, CgTy::Ref)?;
                let Some(builder_raw) = builder_v.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "array_builder_build builder value",
                        at: builder_expr.span.into(),
                    });
                };
                let BasicValueEnum::PointerValue(builder_ptr) = builder_raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "array_builder_build builder type",
                        at: builder_expr.span.into(),
                    });
                };

                let rt = match fqn {
                    "scoop.core.__scoop_array_builder_build_array" => {
                        self.declare_runtime_array_builder_build_array()
                    }
                    "scoop.core.__scoop_array_builder_build_mutable_array" => {
                        self.declare_runtime_array_builder_build_mutable_array()
                    }
                    _ => unreachable!("match arms cover all cases"),
                };

                let call = self
                    .builder
                    .build_call(rt, &[builder_ptr.into()], "array_builder_build")?;
                let raw = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "array_builder_build return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::PointerValue(ptr) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "array_builder_build return type",
                        at: span.into(),
                    });
                };
                Ok(CgValue {
                    ty: CgTy::Ref,
                    value: Some(ptr.into()),
                })
            }
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "unknown array builder intrinsic",
                at: callee_span.into(),
            }),
        }
    }

    fn codegen_sysroot_array_intrinsics(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        fqn: &str,
        args: &[hir::CallArg],
        expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let value_word = IntTy {
            bits: self.host.word_bit_width(),
            signed: true,
        };
        let from_u64 = IntTy {
            bits: 64,
            signed: false,
        };
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();

        // helper：从 args[i] 取出位置参数 expr
        let positional = |idx: usize| -> Result<&hir::Expr, LlvmEmitError> {
            let Some(arg) = args.get(idx) else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "array intrinsic missing arg",
                    at: span.into(),
                });
            };
            match arg {
                hir::CallArg::Positional(expr) => Ok(expr),
                hir::CallArg::Named { .. } => Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "array intrinsic named arg",
                    at: span.into(),
                }),
            }
        };

        match fqn {
            "scoop.core.size" => {
                if args.len() != 1 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "Array.size arity mismatch",
                        at: span.into(),
                    });
                }

                let recv_expr = positional(0)?;
                let recv_v = self.codegen_expr_in_expected_context(recv_expr, Some(CgTy::Ref))?;
                let recv_v = self.coerce_value(recv_expr.span, recv_v, CgTy::Ref)?;
                let Some(recv_raw) = recv_v.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "Array.size receiver value",
                        at: recv_expr.span.into(),
                    });
                };
                let BasicValueEnum::PointerValue(arr_ptr) = recv_raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "Array.size receiver type",
                        at: recv_expr.span.into(),
                    });
                };

                let rt = self.declare_runtime_array_len();
                let call = self
                    .builder
                    .build_call(rt, &[arr_ptr.into()], "array_len")?;
                let raw = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "Array.size return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(len_u64) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "Array.size return type",
                        at: span.into(),
                    });
                };

                let len_word = self.cast_int(len_u64, from_u64, value_word)?;
                Ok(CgValue::int(len_word, value_word))
            }
            "scoop.core.get" => {
                if args.len() != 2 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "Array.get arity mismatch",
                        at: span.into(),
                    });
                }

                let recv_expr = positional(0)?;
                let idx_expr = positional(1)?;

                let recv_v = self.codegen_expr_in_expected_context(recv_expr, Some(CgTy::Ref))?;
                let recv_v = self.coerce_value(recv_expr.span, recv_v, CgTy::Ref)?;
                let Some(recv_raw) = recv_v.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "Array.get receiver value",
                        at: recv_expr.span.into(),
                    });
                };
                let BasicValueEnum::PointerValue(arr_ptr) = recv_raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "Array.get receiver type",
                        at: recv_expr.span.into(),
                    });
                };

                let idx_v = self
                    .codegen_expr_in_expected_context(idx_expr, Some(CgTy::Int(value_word)))?;
                let idx_v = self.coerce_value(idx_expr.span, idx_v, CgTy::Int(value_word))?;
                let (idx_raw, idx_from) = idx_v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "Array.get index value",
                    at: idx_expr.span.into(),
                })?;
                let idx_to = IntTy {
                    bits: 64,
                    signed: true,
                };
                let idx_i64 = self.cast_int(idx_raw, idx_from, idx_to)?;

                let elem_ty = self
                    .infer_array_element_word_cg_ty(recv_expr)
                    .or_else(|| expected.filter(|ty| matches!(ty, CgTy::Unit | CgTy::Bool | CgTy::Int(_) | CgTy::String | CgTy::Ref)))
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "Array.get element type",
                        at: callee_span.into(),
                    })?;

                match elem_ty {
                    CgTy::Ref | CgTy::String => {
                        let rt = self.declare_runtime_array_get_ref();
                        let call = self.builder.build_call(
                            rt,
                            &[arr_ptr.into(), idx_i64.into()],
                            "array_get_ref",
                        )?;
                        let raw = call.try_as_basic_value().basic().ok_or(
                            LlvmEmitError::UnsupportedMainBody {
                                kind: "Array.get return value",
                                at: span.into(),
                            },
                        )?;
                        let BasicValueEnum::PointerValue(ptr) = raw else {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "Array.get return type",
                                at: span.into(),
                            });
                        };

                        match elem_ty {
                            CgTy::Ref => Ok(CgValue {
                                ty: CgTy::Ref,
                                value: Some(ptr.into()),
                            }),
                            CgTy::String => {
                                let str_ptr_ty = self.llvm_scoop_string_ptr_type();
                                let casted =
                                    self.builder.build_pointer_cast(ptr, str_ptr_ty, "ref_to_str")?;
                                Ok(CgValue {
                                    ty: CgTy::String,
                                    value: Some(casted.into()),
                                })
                            }
                            _ => unreachable!("match arms cover all pointer element types"),
                        }
                    }
                    _ => {
                        let rt = self.declare_runtime_array_get_u64();
                        let call = self.builder.build_call(
                            rt,
                            &[arr_ptr.into(), idx_i64.into()],
                            "array_get_u64",
                        )?;
                        let raw = call.try_as_basic_value().basic().ok_or(
                            LlvmEmitError::UnsupportedMainBody {
                                kind: "Array.get return value",
                                at: span.into(),
                            },
                        )?;
                        let BasicValueEnum::IntValue(word_u64) = raw else {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "Array.get return type",
                                at: span.into(),
                            });
                        };
                        self.decode_u64_word_to_cg_value(span, word_u64, elem_ty, gc_i8_ptr_ty)
                    }
                }
            }
            "scoop.core.set" => {
                if args.len() != 3 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "MutableArray.set arity mismatch",
                        at: span.into(),
                    });
                }

                let recv_expr = positional(0)?;
                let idx_expr = positional(1)?;
                let value_expr = positional(2)?;

                let recv_v = self.codegen_expr_in_expected_context(recv_expr, Some(CgTy::Ref))?;
                let recv_v = self.coerce_value(recv_expr.span, recv_v, CgTy::Ref)?;
                let Some(recv_raw) = recv_v.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "MutableArray.set receiver value",
                        at: recv_expr.span.into(),
                    });
                };
                let BasicValueEnum::PointerValue(arr_ptr) = recv_raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "MutableArray.set receiver type",
                        at: recv_expr.span.into(),
                    });
                };

                let idx_v = self
                    .codegen_expr_in_expected_context(idx_expr, Some(CgTy::Int(value_word)))?;
                let idx_v = self.coerce_value(idx_expr.span, idx_v, CgTy::Int(value_word))?;
                let (idx_raw, idx_from) = idx_v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "MutableArray.set index value",
                    at: idx_expr.span.into(),
                })?;
                let idx_to = IntTy {
                    bits: 64,
                    signed: true,
                };
                let idx_i64 = self.cast_int(idx_raw, idx_from, idx_to)?;

                // 尽量使用 receiver 的静态类型（type args）来决定 value 的 codegen/编码方式；
                // 若无法恢复，则退化为“按 value 表达式自身的 codegen 类型编码为 u64”。
                let elem_ty = self.infer_array_element_word_cg_ty(recv_expr);
                match elem_ty {
                    Some(CgTy::Ref) | Some(CgTy::String) => {
                        let expected_elem_ty = elem_ty.unwrap();
                        let v = self
                            .codegen_expr_in_expected_context(value_expr, Some(expected_elem_ty))?;
                        let v = self.coerce_value(value_expr.span, v, expected_elem_ty)?;
                        let v = self.coerce_value(value_expr.span, v, CgTy::Ref)?;
                        let Some(raw) = v.value else {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "MutableArray.set ref value",
                                at: value_expr.span.into(),
                            });
                        };
                        let BasicValueEnum::PointerValue(ptr) = raw else {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "MutableArray.set ref type",
                                at: value_expr.span.into(),
                            });
                        };

                        let rt = self.declare_runtime_array_set_ref();
                        let _ = self.builder.build_call(
                            rt,
                            &[arr_ptr.into(), idx_i64.into(), ptr.into()],
                            "array_set_ref",
                        )?;
                    }
                    _ => {
                        let value_v = match elem_ty {
                            Some(elem_ty) => {
                                let v =
                                    self.codegen_expr_in_expected_context(value_expr, Some(elem_ty))?;
                                self.coerce_value(value_expr.span, v, elem_ty)?
                            }
                            None => self.codegen_expr(value_expr)?,
                        };
                        let word_u64 = self.coerce_u64_word(value_expr.span, value_v)?;

                        let rt = self.declare_runtime_array_set_u64();
                        let _ = self.builder.build_call(
                            rt,
                            &[arr_ptr.into(), idx_i64.into(), word_u64.into()],
                            "array_set_u64",
                        )?;
                    }
                }
                Ok(CgValue::unit())
            }
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "unknown array intrinsic",
                at: callee_span.into(),
            }),
        }
    }

    fn infer_array_element_word_cg_ty(&self, receiver: &hir::Expr) -> Option<CgTy> {
        let receiver_ty = match &receiver.kind {
            hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) => self.env.get(*id)?.hir_ty?,
            hir::ExprKind::VarRef(hir::ValueRef::TopLevel { fqn, .. }) => {
                self.top_level_vars.get(fqn)?.ty
            }
            _ => return None,
        };

        let TypeKind::Ref(RefTypeKind::Nominal(nominal)) = self.types.kind(receiver_ty) else {
            return None;
        };
        // T1317f2：`List/MutableList` 在 sysroot 中作为 `Array/MutableArray` 的 typealias。
        // codegen 侧需要把它们视为“array-like”，否则 `xs.get(i)` 在被 `print/println` 等
        // 以 `String` expected context 调用时，可能会错误地把元素解码为 `String`。
        if !matches!(
            nominal.fqn.as_str(),
            "scoop.core.Array"
                | "scoop.core.MutableArray"
                | "scoop.core.List"
                | "scoop.core.MutableList"
        ) {
            return None;
        }
        let elem_ty = *nominal.args.first()?;
        let cg = self.cg_ty_of(elem_ty)?;

        // 当前 runtime array 以 “u64 word buffer” 表示元素，因此这里限制为可编码为 u64 的类型。
        match cg {
            CgTy::Unit | CgTy::Bool | CgTy::Int(_) | CgTy::String | CgTy::Ref => Some(cg),
            CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => None,
        }
    }

    fn decode_u64_word_to_cg_value(
        &mut self,
        at: crate::span::Span,
        word_u64: IntValue<'ctx>,
        to: CgTy,
        i8_ptr_ty: PointerType<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let from_u64 = IntTy {
            bits: 64,
            signed: false,
        };

        match to {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Bool => {
                let is_true = self.builder.build_int_compare(
                    IntPredicate::NE,
                    word_u64,
                    self.context.i64_type().const_zero(),
                    "u64_to_bool",
                )?;
                Ok(CgValue::bool(is_true))
            }
            CgTy::Int(int_ty) => {
                let decoded = self.cast_int(word_u64, from_u64, int_ty)?;
                Ok(CgValue::int(decoded, int_ty))
            }
            CgTy::Ref => {
                let ptr = self
                    .builder
                    .build_int_to_ptr(word_u64, i8_ptr_ty, "u64_to_ref")?;
                Ok(CgValue {
                    ty: CgTy::Ref,
                    value: Some(ptr.into()),
                })
            }
            CgTy::String => {
                let str_ptr_ty = self.llvm_scoop_string_ptr_type();
                let ptr =
                    self.builder
                        .build_int_to_ptr(word_u64, str_ptr_ty, "u64_to_string")?;
                Ok(CgValue {
                    ty: CgTy::String,
                    value: Some(ptr.into()),
                })
            }
            CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
                Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "decode u64 word to composite value",
                    at: at.into(),
                })
            }
        }
    }

    fn store_size_bytes_of_basic_type(&self, ty: BasicTypeEnum<'ctx>) -> u64 {
        match ty {
            BasicTypeEnum::ArrayType(t) => self.target_data.get_store_size(&t),
            BasicTypeEnum::FloatType(t) => self.target_data.get_store_size(&t),
            BasicTypeEnum::IntType(t) => self.target_data.get_store_size(&t),
            BasicTypeEnum::PointerType(t) => self.target_data.get_store_size(&t),
            BasicTypeEnum::StructType(t) => self.target_data.get_store_size(&t),
            BasicTypeEnum::VectorType(t) => self.target_data.get_store_size(&t),
            BasicTypeEnum::ScalableVectorType(t) => self.target_data.get_store_size(&t),
        }
    }

    fn codegen_sysroot_size_of(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        // 语义：`sizeOf(x)` 在当前阶段返回 `x` 的静态类型在目标 ABI 下的 store size（bytes）。
        //
        // 说明：
        // - 规范中的 `sizeOf<T>()` 是 comptime 反射 intrinsic（spec §6.4）；
        // - 当前阶段尚未实现 comptime 执行链路，因此该 intrinsic 先作为 codegen 内建：
        //   直接把结果 lowering 为编译期常量（不产生对 `scoop.core.sizeOf` 的函数调用）。
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sizeOf() arity mismatch",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sizeOf() named arg",
                at: span.into(),
            });
        };

        let arg_cg = self
            .cg_ty_of(expr.ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "sizeOf() arg type",
                at: callee_span.into(),
            })?;
        let llvm_ty = self.llvm_basic_type_of(expr.span, arg_cg)?;
        let bytes = self.store_size_bytes_of_basic_type(llvm_ty);

        let value_word = IntTy {
            bits: self.host.word_bit_width(),
            signed: true,
        };
        let raw = self.int_type(value_word).const_int(bytes, false);
        Ok(CgValue::int(raw, value_word))
    }

    fn codegen_sysroot_effect_intrinsics(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        fqn: &str,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let value_word = IntTy {
            bits: self.host.word_bit_width(),
            signed: true,
        };
        let _handle_word = IntTy {
            bits: self.host.word_bit_width(),
            signed: false,
        };

        match fqn {
            "scoop.core.__scoop_effect_is_active" => {
                if !args.is_empty() {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect is_active arity mismatch",
                        at: span.into(),
                    });
                }

                let rt = self.declare_runtime_effect_is_active();
                let call = self.builder.build_call(rt, &[], "effect_is_active")?;
                let raw = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "effect is_active return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(raw_int) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect is_active return type",
                        at: span.into(),
                    });
                };

                let from = IntTy {
                    bits: 32,
                    signed: false,
                };
                let casted = self.cast_int(raw_int, from, value_word)?;
                Ok(CgValue::int(casted, value_word))
            }
            "scoop.core.__scoop_effect_set_active" => {
                if !args.is_empty() {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect set_active arity mismatch",
                        at: span.into(),
                    });
                }

                let rt = self.declare_runtime_effect_set_active();
                let _ = self.builder.build_call(rt, &[], "effect_set_active")?;
                Ok(CgValue::unit())
            }
            "scoop.core.__scoop_effect_clear" => {
                if !args.is_empty() {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect clear arity mismatch",
                        at: span.into(),
                    });
                }

                let rt = self.declare_runtime_effect_clear();
                let _ = self.builder.build_call(rt, &[], "effect_clear")?;
                Ok(CgValue::unit())
            }
            "scoop.core.__scoop_effect_slot_write" => {
                if args.len() != 2 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_write arity mismatch",
                        at: span.into(),
                    });
                }

                let hir::CallArg::Positional(tag_expr) = &args[0] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_write tag named arg",
                        at: span.into(),
                    });
                };
                let hir::CallArg::Positional(value_expr) = &args[1] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_write value named arg",
                        at: span.into(),
                    });
                };

                let tag_v =
                    self.codegen_expr_in_expected_context(tag_expr, Some(CgTy::Int(value_word)))?;
                let tag_v = self.coerce_value(tag_expr.span, tag_v, CgTy::Int(value_word))?;
                let (tag_raw, tag_from) =
                    tag_v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_write tag value",
                        at: tag_expr.span.into(),
                    })?;
                let tag_to = IntTy {
                    bits: 32,
                    signed: false,
                };
                let tag_i32 = self.cast_int(tag_raw, tag_from, tag_to)?;

                let value_v =
                    self.codegen_expr_in_expected_context(value_expr, Some(CgTy::Int(value_word)))?;
                let value_v = self.coerce_value(value_expr.span, value_v, CgTy::Int(value_word))?;
                let (value_raw, value_from) =
                    value_v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_write value",
                        at: value_expr.span.into(),
                    })?;
                let value_to = IntTy {
                    bits: 64,
                    signed: false,
                };
                let value_i64 = self.cast_int(value_raw, value_from, value_to)?;

                let rt = self.declare_runtime_effect_perform_slot_write_u64();
                let _ = self.builder.build_call(
                    rt,
                    &[tag_i32.into(), value_i64.into()],
                    "effect_slot_write",
                )?;
                Ok(CgValue::unit())
            }
            "scoop.core.__scoop_effect_slot_write2" => {
                if args.len() != 3 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_write2 arity mismatch",
                        at: span.into(),
                    });
                }

                let hir::CallArg::Positional(tag_expr) = &args[0] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_write2 tag named arg",
                        at: span.into(),
                    });
                };
                let hir::CallArg::Positional(word0_expr) = &args[1] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_write2 word0 named arg",
                        at: span.into(),
                    });
                };
                let hir::CallArg::Positional(word1_expr) = &args[2] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_write2 word1 named arg",
                        at: span.into(),
                    });
                };

                let tag_v =
                    self.codegen_expr_in_expected_context(tag_expr, Some(CgTy::Int(value_word)))?;
                let tag_v = self.coerce_value(tag_expr.span, tag_v, CgTy::Int(value_word))?;
                let (tag_raw, tag_from) =
                    tag_v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_write2 tag value",
                        at: tag_expr.span.into(),
                    })?;
                let tag_to = IntTy {
                    bits: 32,
                    signed: false,
                };
                let tag_i32 = self.cast_int(tag_raw, tag_from, tag_to)?;

                let word0_v =
                    self.codegen_expr_in_expected_context(word0_expr, Some(CgTy::Int(value_word)))?;
                let word0_v = self.coerce_value(word0_expr.span, word0_v, CgTy::Int(value_word))?;
                let (word0_raw, word0_from) =
                    word0_v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_write2 word0 value",
                        at: word0_expr.span.into(),
                    })?;

                let word1_v =
                    self.codegen_expr_in_expected_context(word1_expr, Some(CgTy::Int(value_word)))?;
                let word1_v = self.coerce_value(word1_expr.span, word1_v, CgTy::Int(value_word))?;
                let (word1_raw, word1_from) =
                    word1_v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_write2 word1 value",
                        at: word1_expr.span.into(),
                    })?;

                let word_to = IntTy {
                    bits: 64,
                    signed: false,
                };
                let word0_i64 = self.cast_int(word0_raw, word0_from, word_to)?;
                let word1_i64 = self.cast_int(word1_raw, word1_from, word_to)?;

                let rt = self.declare_runtime_effect_perform_slot_write_u64_2();
                let _ = self.builder.build_call(
                    rt,
                    &[tag_i32.into(), word0_i64.into(), word1_i64.into()],
                    "effect_slot_write2",
                )?;
                Ok(CgValue::unit())
            }
            "scoop.core.__scoop_effect_slot_read_op_tag" => {
                if !args.is_empty() {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_op_tag arity mismatch",
                        at: span.into(),
                    });
                }

                let rt = self.declare_runtime_effect_perform_slot_read_op_tag();
                let call = self
                    .builder
                    .build_call(rt, &[], "effect_slot_read_op_tag")?;
                let raw = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_op_tag return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(raw_int) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_op_tag return type",
                        at: span.into(),
                    });
                };

                let from = IntTy {
                    bits: 32,
                    signed: false,
                };
                let casted = self.cast_int(raw_int, from, value_word)?;
                Ok(CgValue::int(casted, value_word))
            }
            "scoop.core.__scoop_effect_slot_read_len_words" => {
                if !args.is_empty() {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_len_words arity mismatch",
                        at: span.into(),
                    });
                }

                let rt = self.declare_runtime_effect_perform_slot_read_len_words();
                let call = self
                    .builder
                    .build_call(rt, &[], "effect_slot_read_len_words")?;
                let raw = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_len_words return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(raw_int) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_len_words return type",
                        at: span.into(),
                    });
                };

                let from = IntTy {
                    bits: 32,
                    signed: false,
                };
                let casted = self.cast_int(raw_int, from, value_word)?;
                Ok(CgValue::int(casted, value_word))
            }
            "scoop.core.__scoop_effect_slot_read_value" => {
                if !args.is_empty() {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_value arity mismatch",
                        at: span.into(),
                    });
                }

                let rt = self.declare_runtime_effect_perform_slot_read_u64();
                let call = self.builder.build_call(rt, &[], "effect_slot_read_u64")?;
                let raw = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_value return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(raw_int) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_value return type",
                        at: span.into(),
                    });
                };

                let from = IntTy {
                    bits: 64,
                    signed: false,
                };
                let casted = self.cast_int(raw_int, from, value_word)?;
                Ok(CgValue::int(casted, value_word))
            }
            "scoop.core.__scoop_effect_slot_read_word" => {
                if args.len() != 1 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_word arity mismatch",
                        at: span.into(),
                    });
                }

                let hir::CallArg::Positional(index_expr) = &args[0] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_word index named arg",
                        at: span.into(),
                    });
                };

                let index_v =
                    self.codegen_expr_in_expected_context(index_expr, Some(CgTy::Int(value_word)))?;
                let index_v = self.coerce_value(index_expr.span, index_v, CgTy::Int(value_word))?;
                let (index_raw, index_from) =
                    index_v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_word index value",
                        at: index_expr.span.into(),
                    })?;
                let index_to = IntTy {
                    bits: 32,
                    signed: false,
                };
                let index_i32 = self.cast_int(index_raw, index_from, index_to)?;

                let rt = self.declare_runtime_effect_perform_slot_read_u64_at();
                let call = self.builder.build_call(
                    rt,
                    &[index_i32.into()],
                    "effect_slot_read_word_u64",
                )?;
                let raw = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_word return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(raw_int) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "effect slot_read_word return type",
                        at: span.into(),
                    });
                };

                let from = IntTy {
                    bits: 64,
                    signed: false,
                };
                let casted = self.cast_int(raw_int, from, value_word)?;
                Ok(CgValue::int(casted, value_word))
            }
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "unknown sysroot effect intrinsic callee",
                at: callee_span.into(),
            }),
        }
    }

    fn codegen_sysroot_task_intrinsics(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        fqn: &str,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let value_word = IntTy {
            bits: self.host.word_bit_width(),
            signed: true,
        };
        let handle_word = IntTy {
            bits: self.host.word_bit_width(),
            signed: false,
        };

        match fqn {
            "scoop.core.__scoop_task_spawn_int" => {
                if args.len() != 1 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "task spawn arity mismatch",
                        at: span.into(),
                    });
                }

                let hir::CallArg::Positional(expr) = &args[0] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "task spawn named arg",
                        at: span.into(),
                    });
                };

                // `spawn { ... }` 当前阶段只支持 `Int`；typecheck 已保证类型，但这里仍做一次显式期望。
                let v = self.codegen_expr_in_expected_context(expr, Some(CgTy::Int(value_word)))?;
                let v = self.coerce_value(expr.span, v, CgTy::Int(value_word))?;
                let (raw_int, from) = v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "task spawn arg value",
                    at: expr.span.into(),
                })?;

                // runtime ABI：`uint64_t scoop_task_spawn_int(int64_t value)`
                let value_i64 = self.cast_int(
                    raw_int,
                    from,
                    IntTy {
                        bits: 64,
                        signed: true,
                    },
                )?;

                let rt = self.declare_runtime_task_spawn_int();
                let call = self
                    .builder
                    .build_call(rt, &[value_i64.into()], "task_spawn_int")?;
                let raw = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "task spawn return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(handle_u64) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "task spawn return type",
                        at: span.into(),
                    });
                };

                let handle_word_val = self.cast_int(
                    handle_u64,
                    IntTy {
                        bits: 64,
                        signed: false,
                    },
                    handle_word,
                )?;
                Ok(CgValue::int(handle_word_val, handle_word))
            }
            "scoop.core.__scoop_task_join_int" => {
                if args.len() != 1 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "task join arity mismatch",
                        at: span.into(),
                    });
                }

                let hir::CallArg::Positional(expr) = &args[0] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "task join named arg",
                        at: span.into(),
                    });
                };

                let v =
                    self.codegen_expr_in_expected_context(expr, Some(CgTy::Int(handle_word)))?;
                let v = self.coerce_value(expr.span, v, CgTy::Int(handle_word))?;
                let (raw_handle, from) = v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "task join arg value",
                    at: expr.span.into(),
                })?;

                // runtime ABI：`int64_t scoop_task_join_int(uint64_t handle)`
                let handle_u64 = self.cast_int(
                    raw_handle,
                    from,
                    IntTy {
                        bits: 64,
                        signed: false,
                    },
                )?;

                let rt = self.declare_runtime_task_join_int();
                let call = self
                    .builder
                    .build_call(rt, &[handle_u64.into()], "task_join_int")?;
                let raw = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "task join return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(value_i64) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "task join return type",
                        at: span.into(),
                    });
                };

                let value_word_val = self.cast_int(
                    value_i64,
                    IntTy {
                        bits: 64,
                        signed: true,
                    },
                    value_word,
                )?;
                Ok(CgValue::int(value_word_val, value_word))
            }
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "unknown sysroot task intrinsic callee",
                at: callee_span.into(),
            }),
        }
    }

    fn codegen_sysroot_thread_intrinsics(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        fqn: &str,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match fqn {
            "scoop.core.__scoop_thread_spawn_join_resume_u64" => {
                if args.len() != 2 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "thread spawn+resume arity mismatch",
                        at: span.into(),
                    });
                }

                let hir::CallArg::Positional(k_expr) = &args[0] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "thread spawn+resume named arg (continuation)",
                        at: span.into(),
                    });
                };
                let hir::CallArg::Positional(value_expr) = &args[1] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "thread spawn+resume named arg (value)",
                        at: span.into(),
                    });
                };

                let k_v = self.codegen_expr_in_expected_context(k_expr, Some(CgTy::Ref))?;
                let k_v = self.coerce_value(k_expr.span, k_v, CgTy::Ref)?;
                let Some(k_raw) = k_v.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "thread spawn+resume continuation value",
                        at: k_expr.span.into(),
                    });
                };
                let BasicValueEnum::PointerValue(k_ptr) = k_raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "thread spawn+resume continuation type",
                        at: k_expr.span.into(),
                    });
                };

                let value_v = self.codegen_expr(value_expr)?;
                let value_word = self.coerce_u64_word(value_expr.span, value_v)?;

                // runtime ABI：`void scoop_thread_spawn_join_resume_u64(void* k, uint64_t resume_value)`
                let rt = self.declare_runtime_thread_spawn_join_resume_u64();
                let k_i8 =
                    self.builder
                        .build_pointer_cast(k_ptr, self.llvm_gc_i8_ptr_type(), "thread_resume_k_i8")?;
                let _ = self.builder.build_call(
                    rt,
                    &[k_i8.into(), value_word.into()],
                    "thread_spawn_join_resume",
                )?;
                Ok(CgValue::unit())
            }
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "unknown sysroot thread intrinsic callee",
                at: callee_span.into(),
            }),
        }
    }

    fn codegen_sysroot_atomic_int_intrinsics(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        fqn: &str,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let atomic_word = IntTy {
            bits: self.host.word_bit_width(),
            signed: true,
        };

        match fqn {
            "scoop.unsafe.__atomicIntLoad" => {
                if args.len() != 1 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicIntLoad arity mismatch",
                        at: span.into(),
                    });
                }

                let hir::CallArg::Positional(target_expr) = &args[0] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicIntLoad named arg",
                        at: span.into(),
                    });
                };

                let ptr = self.codegen_atomic_int_lvalue_ptr(
                    target_expr.span,
                    target_expr,
                    AtomicIntLvalueMode::ReadOnly,
                )?;

                let llvm_ty = self.int_type(atomic_word);
                let loaded = self
                    .builder
                    .build_load(llvm_ty, ptr, "atomic_int_load")?;
                let inst = loaded.as_instruction_value().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicIntLoad load instruction",
                        at: target_expr.span.into(),
                    },
                )?;
                inst.set_atomic_ordering(AtomicOrdering::SequentiallyConsistent)
                    .map_err(|_| LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicIntLoad set ordering",
                        at: target_expr.span.into(),
                    })?;

                let BasicValueEnum::IntValue(raw) = loaded else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicIntLoad return type",
                        at: target_expr.span.into(),
                    });
                };
                Ok(CgValue::int(raw, atomic_word))
            }
            "scoop.unsafe.__atomicIntStore" => {
                if args.len() != 2 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicIntStore arity mismatch",
                        at: span.into(),
                    });
                }

                let hir::CallArg::Positional(target_expr) = &args[0] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicIntStore named arg (target)",
                        at: span.into(),
                    });
                };
                let hir::CallArg::Positional(value_expr) = &args[1] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicIntStore named arg (value)",
                        at: span.into(),
                    });
                };

                let ptr = self.codegen_atomic_int_lvalue_ptr(
                    target_expr.span,
                    target_expr,
                    AtomicIntLvalueMode::ReadWrite,
                )?;

                let v = self.codegen_expr_in_expected_context(value_expr, Some(CgTy::Int(atomic_word)))?;
                let v = self.coerce_value(value_expr.span, v, CgTy::Int(atomic_word))?;
                let (raw_int, from) = v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "atomicIntStore value",
                    at: value_expr.span.into(),
                })?;
                let raw_int = self.cast_int(raw_int, from, atomic_word)?;

                let inst = self.builder.build_store(ptr, raw_int)?;
                inst.set_atomic_ordering(AtomicOrdering::SequentiallyConsistent)
                    .map_err(|_| LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicIntStore set ordering",
                        at: target_expr.span.into(),
                    })?;
                Ok(CgValue::unit())
            }
            "scoop.unsafe.__atomicIntCompareExchange" => {
                if args.len() != 3 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicIntCompareExchange arity mismatch",
                        at: span.into(),
                    });
                }

                let hir::CallArg::Positional(target_expr) = &args[0] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicIntCompareExchange named arg (target)",
                        at: span.into(),
                    });
                };
                let hir::CallArg::Positional(expected_expr) = &args[1] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicIntCompareExchange named arg (expected)",
                        at: span.into(),
                    });
                };
                let hir::CallArg::Positional(desired_expr) = &args[2] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicIntCompareExchange named arg (desired)",
                        at: span.into(),
                    });
                };

                let ptr = self.codegen_atomic_int_lvalue_ptr(
                    target_expr.span,
                    target_expr,
                    AtomicIntLvalueMode::ReadWrite,
                )?;

                let expected_v =
                    self.codegen_expr_in_expected_context(expected_expr, Some(CgTy::Int(atomic_word)))?;
                let expected_v =
                    self.coerce_value(expected_expr.span, expected_v, CgTy::Int(atomic_word))?;
                let (expected_raw, expected_from) =
                    expected_v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicIntCompareExchange expected",
                        at: expected_expr.span.into(),
                    })?;
                let expected_raw = self.cast_int(expected_raw, expected_from, atomic_word)?;

                let desired_v =
                    self.codegen_expr_in_expected_context(desired_expr, Some(CgTy::Int(atomic_word)))?;
                let desired_v = self.coerce_value(desired_expr.span, desired_v, CgTy::Int(atomic_word))?;
                let (desired_raw, desired_from) =
                    desired_v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicIntCompareExchange desired",
                        at: desired_expr.span.into(),
                    })?;
                let desired_raw = self.cast_int(desired_raw, desired_from, atomic_word)?;

                // LLVM: `cmpxchg ptr, expected, desired` returns `{ T, i1 }`.
                let cx = self.builder.build_cmpxchg(
                    ptr,
                    expected_raw,
                    desired_raw,
                    AtomicOrdering::SequentiallyConsistent,
                    AtomicOrdering::SequentiallyConsistent,
                )?;
                let success = self
                    .builder
                    .build_extract_value(cx, 1, "cmpxchg_success")?;
                let BasicValueEnum::IntValue(ok) = success else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicIntCompareExchange success type",
                        at: span.into(),
                    });
                };
                Ok(CgValue::bool(ok))
            }
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "unknown sysroot atomicInt intrinsic callee",
                at: callee_span.into(),
            }),
        }
    }

    fn codegen_atomic_int_lvalue_ptr(
        &mut self,
        at: crate::span::Span,
        target_expr: &hir::Expr,
        mode: AtomicIntLvalueMode,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let expected = IntTy {
            bits: self.host.word_bit_width(),
            signed: true,
        };

        match &target_expr.kind {
            hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) => {
                let local = self
                    .env
                    .get(*id)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicInt lvalue local",
                        at: at.into(),
                    })?;

                if mode == AtomicIntLvalueMode::ReadWrite && !local.mutable {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicInt requires mutable lvalue",
                        at: at.into(),
                    });
                }

                let CgTy::Int(int_ty) = local.ty else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicInt target type",
                        at: at.into(),
                    });
                };
                if int_ty != expected {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicInt target width",
                        at: at.into(),
                    });
                }

                Ok(local.ptr)
            }
            hir::ExprKind::VarRef(hir::ValueRef::TopLevel { fqn, .. }) => {
                let Some(var) = self.top_level_vars.get(fqn) else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicInt lvalue top-level var",
                        at: at.into(),
                    });
                };

                let gv = self.declare_top_level_var_global(var)?;
                let cg_ty =
                    self.cg_ty_of(var.ty)
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "atomicInt top-level var type",
                            at: at.into(),
                        })?;
                let CgTy::Int(int_ty) = cg_ty else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicInt top-level var type",
                        at: at.into(),
                    });
                };
                if int_ty != expected {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "atomicInt top-level var width",
                        at: at.into(),
                    });
                }

                Ok(gv.as_pointer_value())
            }
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "atomicInt target must be an lvalue",
                at: target_expr.span.into(),
            }),
        }
    }

    fn codegen_int_to_scoop_string(
        &mut self,
        at: crate::span::Span,
        raw_int: IntValue<'ctx>,
        from_ty: IntTy,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        if from_ty.bits > 64 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "integer width for print/println",
                at: at.into(),
            });
        }

        let i64_ty = self.context.i64_type();
        let i8_ty = self.context.i8_type();

        // 先把整数提升/截断到 i64/u64，再调用 runtime 格式化到临时 buffer。
        let to_ty = IntTy {
            bits: 64,
            signed: from_ty.signed,
        };
        let int64 = self.cast_int(raw_int, from_ty, to_ty)?;

        // i64 最长：`-9223372036854775808`（20 字符），预留更宽裕的 cap。
        let cap = i64_ty.const_int(64, false);
        let buf = self
            .builder
            .build_array_alloca(i8_ty, cap, "print_int_buf")?;

        let fmt_name = if from_ty.signed {
            "scoop_format_i64"
        } else {
            "scoop_format_u64"
        };
        let fmt_fun = self.declare_runtime_format_int(fmt_name);
        let call_site = self.builder.build_call(
            fmt_fun,
            &[int64.into(), buf.into(), cap.into()],
            "print_fmt_int",
        )?;
        let len = call_site
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "print int length",
                at: at.into(),
            })?
            .into_int_value();

        // 构造一个临时 `ScoopString`：
        // - 为了不污染 GC 统计（例如 `__scoop_gc_debug_heap_object_count()` 的 fixtures），这里不做 heap 分配；
        // - 在栈上构造 `ScoopString`（addrspace(0)），再用 `addrspacecast` 转为 `addrspace(1)` 传给 runtime。
        //
        // 注意：该指针只在本次 `print/println` 调用期间有效，不会写入 GC roots slot。
        let scoop_str_ty = self.llvm_scoop_string_type();
        let stack_str_ptr =
            self.create_entry_alloca_raw(at, "print_int_scoop_string", scoop_str_ty.into())?;
        let len_ptr = self.builder.build_struct_gep(
            scoop_str_ty,
            stack_str_ptr,
            1,
            "print_int_len_gep",
        )?;
        let data_ptr = self.builder.build_struct_gep(
            scoop_str_ty,
            stack_str_ptr,
            2,
            "print_int_data_gep",
        )?;

        let _ = self.builder.build_store(len_ptr, len)?;
        let _ = self.builder.build_store(data_ptr, buf)?;

        let str_ptr_ty = self.llvm_scoop_string_ptr_type();
        let str_ptr = self.builder.build_address_space_cast(
            stack_str_ptr,
            str_ptr_ty,
            "print_int_str_ptr",
        )?;
        Ok(str_ptr)
    }

    fn codegen_top_level_fun_call(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        fqn: &str,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let sig_fun =
            self.fun_index
                .get(fqn)
                .copied()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "call callee type",
                    at: callee_span.into(),
                })?;

        if args.len() != sig_fun.params.len() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "call arity mismatch",
                at: span.into(),
            });
        }

        let mut llvm_args = Vec::with_capacity(args.len());
        for (idx, arg) in args.iter().enumerate() {
            let hir::CallArg::Positional(expr) = arg else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "named call arg",
                    at: span.into(),
                });
            };

            let target_cg = self.cg_ty_of(sig_fun.params[idx].ty).ok_or(
                LlvmEmitError::UnsupportedMainBody {
                    kind: "call arg type",
                    at: expr.span.into(),
                },
            )?;
            let v = match &expr.kind {
                hir::ExprKind::Closure(closure) => {
                    self.codegen_closure_expr(expr.span, closure, sig_fun.params[idx].ty)?
                }
                _ => self.codegen_expr_in_expected_context(expr, Some(target_cg))?,
            };
            let coerced = self.coerce_value(expr.span, v, target_cg)?;
            llvm_args.push(self.as_llvm_arg_value(expr.span, target_cg, coerced)?);
        }

        let llvm_name = self
            .extern_funs
            .get(fqn)
            .map(|e| e.symbol.as_str())
            .unwrap_or(fqn);

        let llvm_fun = match self.module.get_function(llvm_name) {
            Some(f) => f,
            None => self.declare_top_level_fun(sig_fun)?,
        };
        let call_site = self.builder.build_call(llvm_fun, &llvm_args, "call")?;
        call_site.set_call_convention(self.llvm_call_convention_for_fqn(fqn));
        // T0614：flag-based unwinding（最小 Raise）：
        // - callee 可能执行 `Raise.raise` 并通过“设置 flag + 返回默认值”向外传播；
        // - 因此 call site 必须检查 flag，并跳转到最近的 handler boundary（或继续向外 return）。
        self.emit_effect_unwind_if_active(span)?;

        let ret_cg =
            self.cg_ty_of(sig_fun.return_ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "call return type",
                    at: span.into(),
                })?;

        match ret_cg {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Bool => {
                let value = call_site
                    .try_as_basic_value()
                    .basic()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "call return value",
                        at: span.into(),
                    })?
                    .into_int_value();
                Ok(CgValue::bool(value))
            }
            CgTy::Int(int_ty) => {
                let value = call_site
                    .try_as_basic_value()
                    .basic()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "call return value",
                        at: span.into(),
                    })?
                    .into_int_value();
                Ok(CgValue::int(value, int_ty))
            }
            CgTy::String | CgTy::Ref => {
                let raw = call_site.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "call return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::PointerValue(ptr) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "call return type",
                        at: span.into(),
                    });
                };
                Ok(CgValue {
                    ty: ret_cg,
                    value: Some(ptr.into()),
                })
            }
            CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
                let raw = call_site.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "call return value",
                        at: span.into(),
                    },
                )?;
                Ok(CgValue {
                    ty: ret_cg,
                    value: Some(raw),
                })
            }
        }
    }

    fn codegen_funptr_value_call(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        funptr_addr: inkwell::values::IntValue<'ctx>,
        funptr_int_ty: IntTy,
        fun_ty: &crate::ty::FunctionType,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if fun_ty.receiver.is_some() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "receiver funptr call",
                at: callee_span.into(),
            });
        }

        if args.len() != fun_ty.params.len() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "funptr call arity mismatch",
                at: span.into(),
            });
        }

        // 1) 组装 indirect call 的 LLVM 函数类型与参数列表。
        let mut llvm_param_tys: Vec<BasicMetadataTypeEnum<'ctx>> =
            Vec::with_capacity(fun_ty.params.len());
        for ty in &fun_ty.params {
            llvm_param_tys.push(self.llvm_param_ty(callee_span, *ty)?);
        }

        let ret_cg =
            self.cg_ty_of(fun_ty.return_ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "funptr call return type",
                    at: callee_span.into(),
                })?;

        let llvm_fun_ty = match ret_cg {
            CgTy::Unit => self.context.void_type().fn_type(&llvm_param_tys, false),
            other => self
                .llvm_basic_type_of(callee_span, other)?
                .fn_type(&llvm_param_tys, false),
        };

        // 2) 将 `word-sized address` 转换为函数指针。
        //
        // 说明：
        // - 当前阶段我们把 `scoop.unsafe.FunPtr<F>` 视为 “opaque native function address”；
        // - `fp(args...)` 会在 codegen 阶段执行 `inttoptr` 并生成 indirect call；
        // - v0 阶段仅支持 C ABI（callconv 0）。
        let fun_ptr_ty = llvm_fun_ty.ptr_type(AddressSpace::default());
        let casted_addr = if funptr_int_ty.bits == self.host.word_bit_width() {
            funptr_addr
        } else {
            // 理论上不会发生：`FunPtr` 的 codegen 表示就是 `word-sized`。
            let from = funptr_int_ty;
            let to = IntTy {
                bits: self.host.word_bit_width(),
                signed: false,
            };
            self.cast_int(funptr_addr, from, to)?
        };
        let typed_fn_ptr = self
            .builder
            .build_int_to_ptr(casted_addr, fun_ptr_ty, "funptr_typed")?;

        // 3) 求值实参并执行调用。
        let mut llvm_args: Vec<inkwell::values::BasicMetadataValueEnum<'ctx>> =
            Vec::with_capacity(args.len());
        for (idx, arg) in args.iter().enumerate() {
            let hir::CallArg::Positional(expr) = arg else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "named funptr call arg",
                    at: span.into(),
                });
            };

            let param_ty = fun_ty.params[idx];
            let target_cg = self
                .cg_ty_of(param_ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "funptr call arg type",
                    at: expr.span.into(),
                })?;

            let v = match &expr.kind {
                hir::ExprKind::Closure(closure) => {
                    self.codegen_closure_expr(expr.span, closure, param_ty)?
                }
                _ => self.codegen_expr(expr)?,
            };
            let coerced = self.coerce_value(expr.span, v, target_cg)?;
            llvm_args.push(self.as_llvm_arg_value(expr.span, target_cg, coerced)?);
        }

        let call_site = self.builder.build_indirect_call(
            llvm_fun_ty,
            typed_fn_ptr,
            &llvm_args,
            "call_funptr",
        )?;
        call_site.set_call_convention(0);
        self.emit_effect_unwind_if_active(span)?;

        match ret_cg {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Bool => {
                let value = call_site
                    .try_as_basic_value()
                    .basic()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "funptr call return value",
                        at: span.into(),
                    })?
                    .into_int_value();
                Ok(CgValue::bool(value))
            }
            CgTy::Int(int_ty) => {
                let value = call_site
                    .try_as_basic_value()
                    .basic()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "funptr call return value",
                        at: span.into(),
                    })?
                    .into_int_value();
                Ok(CgValue::int(value, int_ty))
            }
            CgTy::String | CgTy::Ref => {
                let raw = call_site.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "funptr call return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::PointerValue(ptr) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "funptr call return type",
                        at: span.into(),
                    });
                };
                Ok(CgValue {
                    ty: ret_cg,
                    value: Some(ptr.into()),
                })
            }
            CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
                let raw = call_site.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "funptr call return value",
                        at: span.into(),
                    },
                )?;
                Ok(CgValue {
                    ty: ret_cg,
                    value: Some(raw),
                })
            }
        }
    }

    fn codegen_function_value_call(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        local: &CgLocal<'ctx>,
        fun_ty: &crate::ty::FunctionType,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if fun_ty.receiver.is_some() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "receiver function value call",
                at: callee_span.into(),
            });
        }

        if args.len() != fun_ty.params.len() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "function value call arity mismatch",
                at: span.into(),
            });
        }

        // 1) 读取 closure object：`{ header, env_ptr, fn_ptr }`
        let CgTy::Ref = local.ty else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "function value local type",
                at: callee_span.into(),
            });
        };

        let llvm_local_ty = self.llvm_basic_type_of(callee_span, local.ty)?;
        let closure_obj_i8 = self
            .builder
            .build_load(llvm_local_ty, local.ptr, "load_closure_obj")?
            .into_pointer_value();

        let closure_ty = self.llvm_closure_object_type();
        let closure_ptr_ty = closure_ty.ptr_type(self.gc_address_space());
        let closure_ptr =
            self.builder
                .build_pointer_cast(closure_obj_i8, closure_ptr_ty, "closure_obj_ptr")?;

        let i8_ptr_ty = self.llvm_i8_ptr_type();
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let env_ptr_gep =
            self.builder
                .build_struct_gep(closure_ty, closure_ptr, 1, "closure_env_gep")?;
        let fn_ptr_gep =
            self.builder
                .build_struct_gep(closure_ty, closure_ptr, 2, "closure_fn_gep")?;

        let env_ptr = self
            .builder
            .build_load(i8_ptr_ty, env_ptr_gep, "closure_env")?
            .into_pointer_value();
        let fn_ptr_raw = self
            .builder
            .build_load(i8_ptr_ty, fn_ptr_gep, "closure_fn")?
            .into_pointer_value();

        // 2) 组装 indirect call 的 LLVM 函数类型与参数。
        let mut llvm_param_tys: Vec<BasicMetadataTypeEnum<'ctx>> =
            Vec::with_capacity(1 + fun_ty.params.len());
        llvm_param_tys.push(i8_ptr_ty.into());
        for ty in &fun_ty.params {
            llvm_param_tys.push(self.llvm_param_ty(callee_span, *ty)?);
        }

        let ret_cg = self
            .cg_ty_of(fun_ty.return_ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "function value call return type",
                at: callee_span.into(),
            })?;

        let llvm_fun_ty = match ret_cg {
            CgTy::Unit => self.context.void_type().fn_type(&llvm_param_tys, false),
            CgTy::Bool => self.context.bool_type().fn_type(&llvm_param_tys, false),
            CgTy::Int(int_ty) => self.int_type(int_ty).fn_type(&llvm_param_tys, false),
            CgTy::String => self
                .llvm_scoop_string_ptr_type()
                .fn_type(&llvm_param_tys, false),
            CgTy::Ref => gc_i8_ptr_ty.fn_type(&llvm_param_tys, false),
            CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "function value call return type",
                    at: callee_span.into(),
                });
            }
        };

        let typed_fn_ptr = self.builder.build_pointer_cast(
            fn_ptr_raw,
            llvm_fun_ty.ptr_type(AddressSpace::default()),
            "closure_fn_typed",
        )?;

        // 3) 求值实参并执行调用（env 作为第一个参数）。
        let mut llvm_args: Vec<inkwell::values::BasicMetadataValueEnum<'ctx>> =
            Vec::with_capacity(1 + args.len());
        llvm_args.push(env_ptr.into());
        for (idx, arg) in args.iter().enumerate() {
            let hir::CallArg::Positional(expr) = arg else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "named function value call arg",
                    at: span.into(),
                });
            };

            let param_ty = fun_ty.params[idx];
            let target_cg = self
                .cg_ty_of(param_ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "function value call arg type",
                    at: expr.span.into(),
                })?;

            let v = match &expr.kind {
                hir::ExprKind::Closure(closure) => {
                    self.codegen_closure_expr(expr.span, closure, param_ty)?
                }
                _ => self.codegen_expr(expr)?,
            };
            let coerced = self.coerce_value(expr.span, v, target_cg)?;
            llvm_args.push(self.as_llvm_arg_value(expr.span, target_cg, coerced)?);
        }

        let call_site = self.builder.build_indirect_call(
            llvm_fun_ty,
            typed_fn_ptr,
            &llvm_args,
            "call_closure",
        )?;
        self.emit_effect_unwind_if_active(span)?;

        match ret_cg {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Bool => {
                let value = call_site
                    .try_as_basic_value()
                    .basic()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "function value call return value",
                        at: span.into(),
                    })?
                    .into_int_value();
                Ok(CgValue::bool(value))
            }
            CgTy::Int(int_ty) => {
                let value = call_site
                    .try_as_basic_value()
                    .basic()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "function value call return value",
                        at: span.into(),
                    })?
                    .into_int_value();
                Ok(CgValue::int(value, int_ty))
            }
            CgTy::String | CgTy::Ref => {
                let raw = call_site.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "function value call return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::PointerValue(ptr) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "function value call return type",
                        at: span.into(),
                    });
                };
                Ok(CgValue {
                    ty: ret_cg,
                    value: Some(ptr.into()),
                })
            }
            CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
                Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "function value call return type",
                    at: span.into(),
                })
            }
        }
    }

    fn codegen_closure_expr(
        &mut self,
        span: crate::span::Span,
        closure: &hir::ClosureExpr,
        expected_fun_ty: TypeId,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let TypeKind::Ref(RefTypeKind::Function(fun_ty)) = self.types.kind(expected_fun_ty) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "lambda without expected function type",
                at: span.into(),
            });
        };

        if fun_ty.receiver.is_some() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "receiver lambda (not supported)",
                at: span.into(),
            });
        }

        // 1) 确定参数绑定（显式 params 或 Kotlin-like 隐式 `it`）。
        let (param_bindings, captures) = self.closure_param_bindings(span, closure, fun_ty)?;
        if captures.iter().any(|c| c.mutable) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "mutable capture (not supported yet)",
                at: span.into(),
            });
        }

        let fun_name = format!("scoop.lambda${}", closure.id.as_u32());

        // 2) 确保 closure 函数本体存在（module-level function）。
        //
        // 注意：我们会在“第一次 codegen 到该 lambda 表达式”时生成其函数体；之后复用同名符号。
        let saved_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: span.into(),
                })?;

        let llvm_fun = if let Some(existing) = self.module.get_function(&fun_name) {
            existing
        } else {
            let i8_ptr_ty = self.llvm_i8_ptr_type();
            let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
            let mut llvm_param_tys: Vec<BasicMetadataTypeEnum<'ctx>> =
                Vec::with_capacity(1 + fun_ty.params.len());
            // env ptr（当前阶段不支持捕获，但 ABI 预留）。
            llvm_param_tys.push(i8_ptr_ty.into());
            for ty in &fun_ty.params {
                llvm_param_tys.push(self.llvm_param_ty(span, *ty)?);
            }

            let ret_cg =
                self.cg_ty_of(fun_ty.return_ty)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "lambda return type",
                        at: span.into(),
                    })?;

            let fn_ty = match ret_cg {
                CgTy::Unit => self.context.void_type().fn_type(&llvm_param_tys, false),
                CgTy::Bool => self.context.bool_type().fn_type(&llvm_param_tys, false),
                CgTy::Int(int_ty) => self.int_type(int_ty).fn_type(&llvm_param_tys, false),
                CgTy::String => self
                    .llvm_scoop_string_ptr_type()
                    .fn_type(&llvm_param_tys, false),
                CgTy::Ref => gc_i8_ptr_ty.fn_type(&llvm_param_tys, false),
                CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "lambda return type",
                        at: span.into(),
                    });
                }
            };

            let llvm_fun = self.module.add_function(&fun_name, fn_ty, None);
            llvm_fun.set_call_conventions(0);

            let mut cg = MainCodegen::new(
                self.context,
                self.module,
                self.builder,
                self.target_data,
                self.host,
                self.source,
                self.types,
                self.struct_layouts,
                self.enum_layouts,
                self.top_level_vars,
                self.object_inits,
                self.class_inits,
                self.ctor_call_sites,
                self.extern_funs,
                self.fun_index,
            );
            // 说明：closure 捕获信息里没有类型；这里在外层 codegen 阶段用 env 中的 locals 恢复 type id，
            // 再传给 closure fun body 用于 env layout 与绑定。
            let mut capture_bindings: Vec<(hir::SymbolId, String, TypeId)> =
                Vec::with_capacity(captures.len());
            for cap in &captures {
                let Some(local) = self.env.get(cap.id) else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "capture local not found",
                        at: cap.decl_span.into(),
                    });
                };
                let Some(ty_id) = local.hir_ty else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "capture local type",
                        at: cap.decl_span.into(),
                    });
                };
                capture_bindings.push((cap.id, cap.name.clone(), ty_id));
            }

            cg.codegen_closure_fun_body(
                closure,
                fun_ty,
                &param_bindings,
                &capture_bindings,
                llvm_fun,
            )?;

            // 恢复外层插入点（closure 函数 codegen 会移动 builder 的 position）。
            self.builder.position_at_end(saved_block);
            llvm_fun
        };

        // 3) 创建 closure object：`{ header, env_ptr, fn_ptr=&lambda }`
        let closure_obj_ty = self.llvm_closure_object_type();
        let obj_size_bytes = self.target_data.get_store_size(&closure_obj_ty);

        let rt_alloc = self.declare_runtime_alloc();
        let size_v = self.context.i64_type().const_int(obj_size_bytes, false);
        let call = self
            .builder
            .build_call(rt_alloc, &[size_v.into()], "rt_alloc_closure")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(obj_i8) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc return type",
                at: span.into(),
            });
        };

        let i8_ptr_ty = self.llvm_i8_ptr_type();
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let obj_ptr_ty = closure_obj_ty.ptr_type(self.gc_address_space());
        let obj_ptr = self
            .builder
            .build_pointer_cast(obj_i8, obj_ptr_ty, "closure_obj_ptr")?;

        let env_gep =
            self.builder
                .build_struct_gep(closure_obj_ty, obj_ptr, 1, "closure_env_gep")?;
        let fn_gep = self
            .builder
            .build_struct_gep(closure_obj_ty, obj_ptr, 2, "closure_fn_gep")?;

        // 若有捕获，则分配 env 并写入捕获值；否则 env_ptr 为 NULL。
        let env_i8 = if captures.is_empty() {
            i8_ptr_ty.const_null()
        } else {
            // 说明（early stage）：
            // - 目前 closure object 的 type descriptor 尚未接入，GC 不会从 closure object 扫描到 env；
            // - 为避免 env 被 mark-sweep 误回收，这里先用 libc `malloc` 分配 env（不会被 GC 管理）。
            //   这会泄漏 env 内存，但能保持语义可回归；更完整的释放策略留给 type descriptor/release hook。
            let mut capture_bindings: Vec<(hir::SymbolId, String, TypeId)> =
                Vec::with_capacity(captures.len());
            for cap in &captures {
                let Some(local) = self.env.get(cap.id) else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "capture local not found",
                        at: cap.decl_span.into(),
                    });
                };
                let Some(ty_id) = local.hir_ty else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "capture local type",
                        at: cap.decl_span.into(),
                    });
                };
                capture_bindings.push((cap.id, cap.name.clone(), ty_id));
            }

            let env_ty = self.llvm_closure_env_type(span, closure.id, &capture_bindings)?;
            let env_size_bytes = self.target_data.get_store_size(&env_ty);

            let malloc = self.declare_libc_malloc();
            let size_v = self.context.i64_type().const_int(env_size_bytes, false);
            let call = self.builder.build_call(malloc, &[size_v.into()], "malloc_env")?;
            let raw = call
                .try_as_basic_value()
                .basic()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "malloc return value",
                    at: span.into(),
                })?;
            let BasicValueEnum::PointerValue(env_i8) = raw else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "malloc return type",
                    at: span.into(),
                });
            };

            let env_ptr_ty = env_ty.ptr_type(AddressSpace::default());
            let env_ptr = self
                .builder
                .build_pointer_cast(env_i8, env_ptr_ty, "closure_env_ptr")?;

            for (idx, (id, name, ty_id)) in capture_bindings.iter().enumerate() {
                let Some(local) = self.env.get(*id) else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "capture local not found",
                        at: span.into(),
                    });
                };

                let cg_ty = self.cg_ty_of(*ty_id).ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "capture local type",
                    at: span.into(),
                })?;
                if !matches!(
                    cg_ty,
                    CgTy::Unit | CgTy::Bool | CgTy::Int(_) | CgTy::String | CgTy::Ref
                ) {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "capture local (non-scalar)",
                        at: span.into(),
                    });
                }

                let llvm_ty = self.llvm_basic_type_of(span, cg_ty)?;
                let loaded = self
                    .builder
                    .build_load(llvm_ty, local.ptr, &format!("capture_load_{name}"))?;

                let field_gep = self.builder.build_struct_gep(
                    env_ty,
                    env_ptr,
                    idx as u32,
                    &format!("capture_gep_{name}"),
                )?;
                let _ = self.builder.build_store(field_gep, loaded)?;
            }

            env_i8
        };
        let _ = self.builder.build_store(env_gep, env_i8)?;

        let fn_ptr = llvm_fun.as_global_value().as_pointer_value();
        let fn_i8 = self
            .builder
            .build_pointer_cast(fn_ptr, i8_ptr_ty, "closure_fn_i8")?;
        let _ = self.builder.build_store(fn_gep, fn_i8)?;

        let obj_i8 = self
            .builder
            .build_pointer_cast(obj_ptr, gc_i8_ptr_ty, "closure_obj_i8")?;

        Ok(CgValue {
            ty: CgTy::Ref,
            value: Some(obj_i8.into()),
        })
    }

    fn closure_param_bindings(
        &self,
        at: crate::span::Span,
        closure: &hir::ClosureExpr,
        fun_ty: &crate::ty::FunctionType,
    ) -> Result<(Vec<(hir::SymbolId, String, TypeId)>, Vec<hir::Capture>), LlvmEmitError> {
        if fun_ty.receiver.is_some() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "receiver lambda",
                at: at.into(),
            });
        }

        // 显式 params：`{ x -> ... }`
        if closure.params.len() == fun_ty.params.len() {
            let params = closure
                .params
                .iter()
                .zip(fun_ty.params.iter())
                .map(|(p, ty)| (p.id, p.name.clone(), *ty))
                .collect::<Vec<_>>();
            return Ok((params, closure.captures.clone()));
        }

        // 隐式 `it`：`{ body }` + expected `(T) -> R`
        if closure.params.is_empty() && fun_ty.params.len() == 1 {
            let Some(it_cap) = closure.captures.iter().find(|c| c.name == "it") else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "implicit it lambda missing it binder",
                    at: at.into(),
                });
            };

            let params = vec![(it_cap.id, "it".to_string(), fun_ty.params[0])];
            let captures = closure
                .captures
                .iter()
                .filter(|c| c.id != it_cap.id)
                .cloned()
                .collect::<Vec<_>>();
            return Ok((params, captures));
        }

        Err(LlvmEmitError::UnsupportedMainBody {
            kind: "lambda param arity mismatch",
            at: at.into(),
        })
    }

    fn codegen_closure_fun_body(
        &mut self,
        closure: &hir::ClosureExpr,
        fun_ty: &crate::ty::FunctionType,
        param_bindings: &[(hir::SymbolId, String, TypeId)],
        capture_bindings: &[(hir::SymbolId, String, TypeId)],
        llvm_fun: FunctionValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let entry = self.context.append_basic_block(llvm_fun, "entry");
        self.builder.position_at_end(entry);

        // GC roots：captures + closure params + body locals（nested closure 自身会单独插桩）。
        let mut root_ids: Vec<hir::SymbolId> = Vec::new();
        let mut seen: HashSet<hir::SymbolId> = HashSet::new();
        for (id, _name, ty) in capture_bindings {
            if matches!(self.cg_ty_of(*ty), Some(CgTy::Ref)) && seen.insert(*id) {
                root_ids.push(*id);
            }
        }
        for (id, _name, ty) in param_bindings {
            if matches!(self.cg_ty_of(*ty), Some(CgTy::Ref)) && seen.insert(*id) {
                root_ids.push(*id);
            }
        }
        self.collect_gc_root_ids_in_expr(closure.body.as_ref(), &mut root_ids, &mut seen);
        self.setup_gc_frame_for_root_ids(closure.span, &root_ids)?;

        self.env.push_scope();

        // 入口的返回类型由期望函数类型决定（用于 Raise 的“早退默认值”）。
        let declared_return_cg =
            self.cg_ty_of(fun_ty.return_ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "lambda return type",
                    at: closure.span.into(),
                })?;
        self.current_fun_return_ty = Some(declared_return_cg);

        // captures：从 env（第 0 个 LLVM param）读取并绑定为 locals。
        if !capture_bindings.is_empty() {
            let env_i8 = llvm_fun
                .get_nth_param(0)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "missing llvm lambda env param",
                    at: closure.span.into(),
                })?
                .into_pointer_value();

            let env_ty = self.llvm_closure_env_type(closure.span, closure.id, capture_bindings)?;
            let env_ptr_ty = env_ty.ptr_type(AddressSpace::default());
            let env_ptr = self
                .builder
                .build_pointer_cast(env_i8, env_ptr_ty, "closure_env_ptr")?;

            for (idx, (id, name, ty_id)) in capture_bindings.iter().enumerate() {
                let target_ty = self
                    .cg_ty_of(*ty_id)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "capture type",
                        at: closure.span.into(),
                    })?;
                if !matches!(
                    target_ty,
                    CgTy::Unit | CgTy::Bool | CgTy::Int(_) | CgTy::String | CgTy::Ref
                ) {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "capture local (non-scalar)",
                        at: closure.span.into(),
                    });
                }

                let llvm_ty = self.llvm_basic_type_of(closure.span, target_ty)?;
                let field_gep = self.builder.build_struct_gep(
                    env_ty,
                    env_ptr,
                    idx as u32,
                    &format!("capture_gep_{name}"),
                )?;
                let loaded =
                    self.builder
                        .build_load(llvm_ty, field_gep, &format!("capture_{name}"))?;

                let ptr = self.create_entry_alloca(closure.span, name, target_ty)?;
                let init = CgValue {
                    ty: target_ty,
                    value: Some(loaded),
                };
                let stored = self.store_local_value(closure.span, ptr, target_ty, init)?;
                let gc_root_slot = self.gc_root_slot_for(*id);
                if let Some(slot_ptr) = gc_root_slot {
                    self.store_gc_root_slot_value(closure.span, slot_ptr, stored)?;
                }

                self.env.insert(
                    *id,
                    CgLocal {
                        hir_ty: Some(*ty_id),
                        ty: target_ty,
                        ptr,
                        mutable: false,
                        gc_root_slot,
                    },
                );
            }
        }

        // params：env（第 0 个 LLVM param）；用户 params 从第 1 个开始。
        for (idx, (id, name, ty_id)) in param_bindings.iter().enumerate() {
            let target_ty = self
                .cg_ty_of(*ty_id)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "lambda param type",
                    at: closure.span.into(),
                })?;

            let ptr = self.create_entry_alloca(closure.span, name, target_ty)?;

            let init = match target_ty {
                CgTy::Unit => CgValue::unit(),
                CgTy::Bool => {
                    let raw = llvm_fun
                        .get_nth_param((idx + 1) as u32)
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "missing llvm lambda param",
                            at: closure.span.into(),
                        })?
                        .into_int_value();
                    CgValue::bool(raw)
                }
                CgTy::Int(int_ty) => {
                    let raw = llvm_fun
                        .get_nth_param((idx + 1) as u32)
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "missing llvm lambda param",
                            at: closure.span.into(),
                        })?
                        .into_int_value();
                    CgValue::int(raw, int_ty)
                }
                CgTy::String => {
                    let raw = llvm_fun
                        .get_nth_param((idx + 1) as u32)
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "missing llvm lambda param",
                            at: closure.span.into(),
                        })?
                        .into_pointer_value();
                    CgValue {
                        ty: CgTy::String,
                        value: Some(raw.into()),
                    }
                }
                CgTy::Ref => {
                    let raw = llvm_fun
                        .get_nth_param((idx + 1) as u32)
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "missing llvm lambda param",
                            at: closure.span.into(),
                        })?
                        .into_pointer_value();
                    CgValue {
                        ty: CgTy::Ref,
                        value: Some(raw.into()),
                    }
                }
                CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "lambda param type",
                        at: closure.span.into(),
                    });
                }
            };

            let stored = self.store_local_value(closure.span, ptr, target_ty, init)?;
            let gc_root_slot = self.gc_root_slot_for(*id);
            if let Some(slot_ptr) = gc_root_slot {
                self.store_gc_root_slot_value(closure.span, slot_ptr, stored)?;
            }

            self.env.insert(
                *id,
                CgLocal {
                    hir_ty: Some(*ty_id),
                    ty: target_ty,
                    ptr,
                    mutable: false,
                    gc_root_slot,
                },
            );
        }

        let body_expr = closure.body.as_ref();
        let ret_v = match &body_expr.kind {
            hir::ExprKind::Block(block) => {
                self.codegen_block_as_return_value(block, declared_return_cg)?
            }
            _ => {
                let v =
                    self.codegen_expr_in_expected_context(body_expr, Some(declared_return_cg))?;
                if declared_return_cg == CgTy::Unit {
                    CgValue::unit()
                } else {
                    self.coerce_value(body_expr.span, v, declared_return_cg)?
                }
            }
        };

        self.emit_return(closure.span, declared_return_cg, ret_v)?;

        self.env.pop_scope();
        Ok(())
    }

    fn llvm_closure_env_type(
        &mut self,
        at: crate::span::Span,
        closure_id: hir::ClosureId,
        capture_bindings: &[(hir::SymbolId, String, TypeId)],
    ) -> Result<StructType<'ctx>, LlvmEmitError> {
        let name = format!("scoop.lambda_env${}", closure_id.as_u32());
        if let Some(existing) = self.context.get_struct_type(&name) {
            return Ok(existing);
        }

        let env_ty = self.context.opaque_struct_type(&name);
        let mut fields: Vec<BasicTypeEnum<'ctx>> = Vec::with_capacity(capture_bindings.len());
        for (_id, _name, ty_id) in capture_bindings {
            let cg_ty = self.cg_ty_of(*ty_id).ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "capture type",
                at: at.into(),
            })?;
            if !matches!(
                cg_ty,
                CgTy::Unit | CgTy::Bool | CgTy::Int(_) | CgTy::String | CgTy::Ref
            ) {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "capture local (non-scalar)",
                    at: at.into(),
                });
            }
            fields.push(self.llvm_basic_type_of(at, cg_ty)?);
        }
        env_ty.set_body(&fields, false);
        Ok(env_ty)
    }

    fn codegen_unresolved_ident(
        &mut self,
        span: crate::span::Span,
        name: &str,
        expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        // 0-参数 enum variant 值：`None`
        let Some(CgTy::Enum(enum_ty)) = expected else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "unresolved ident without expected enum type",
                at: span.into(),
            });
        };

        let cg_layout = self.cg_enum_layout(span, enum_ty)?;
        let variant = cg_layout.variants.iter().find(|v| v.name == name).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "unknown enum variant",
                at: span.into(),
            },
        )?;
        let tag = variant.tag;
        let field_count = variant.fields.len();

        if field_count != 0 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "non-zero-arity enum variant used as value",
                at: span.into(),
            });
        }

        self.build_enum_value(span, enum_ty, tag, None)
    }

    fn codegen_enum_variant_ctor_call(
        &mut self,
        span: crate::span::Span,
        enum_ty: TypeId,
        variant_name: &str,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let layout = self.cg_enum_layout(span, enum_ty)?;
        let variant = layout
            .variants
            .iter()
            .find(|v| v.name == variant_name)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "unknown enum variant",
                at: span.into(),
            })?
            .clone();

        if variant.fields.len() != args.len() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "enum variant ctor arity mismatch",
                at: span.into(),
            });
        }

        // 先把所有实参在“字段期望类型”下 codegen 并做最小 coercion，避免后续重复走 codegen。
        let mut field_values: Vec<(CgTy, CgValue<'ctx>)> = Vec::with_capacity(args.len());
        for (idx, (field_cg, arg)) in variant.fields.iter().copied().zip(args.iter()).enumerate() {
            let hir::CallArg::Positional(arg_expr) = arg else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "named enum ctor arg",
                    at: span.into(),
                });
            };

            let v = self.codegen_expr_in_expected_context(arg_expr, Some(field_cg))?;
            let coerced = self.coerce_value(arg_expr.span, v, field_cg)?;
            field_values.push((field_cg, coerced));

            // 提前在 debug 名称里体现 index，便于排查（不影响语义）。
            let _ = idx;
        }

        // 1) boxed variant：把 payload fields 聚合成一个 payload struct，存到栈上并把指针写入 enum payload。
        if variant.boxed {
            let payload_struct_ty =
                self.llvm_enum_boxed_payload_struct_type(span, enum_ty, &variant)?;
            let mut payload: AggregateValueEnum<'ctx> = payload_struct_ty.get_undef().into();

            for (idx, (field_cg, field_v)) in field_values.iter().enumerate() {
                // Unit 没有运行期值；当前阶段不允许把 Unit 作为 enum payload 字段。
                if matches!(field_cg, CgTy::Unit) {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "enum boxed payload field (unit)",
                        at: span.into(),
                    });
                }
                let raw = field_v.value.ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "enum boxed payload field value",
                    at: span.into(),
                })?;
                payload = self.builder.build_insert_value(
                    payload,
                    raw,
                    idx as u32,
                    &format!("enum_payload_field_{idx}"),
                )?;
            }

            let tmp_name = format!(
                "boxed_enum_payload_{}_{}",
                enum_ty.as_u32(),
                sanitize_llvm_ident(&variant.name)
            );
            let payload_ptr =
                self.create_entry_alloca_raw(span, &tmp_name, payload_struct_ty.into())?;
            let _ = self
                .builder
                .build_store(payload_ptr, payload.as_basic_value_enum())?;

            let word_ty = self.int_type(self.enum_payload_ty());
            let payload_word =
                self.builder
                    .build_ptr_to_int(payload_ptr, word_ty, "boxed_enum_payload_ptr")?;
            return self.build_enum_value(span, enum_ty, variant.tag, Some(payload_word));
        }

        // 2) inline（非 boxed）variant：当前阶段仍采用 “word payload” 承载的小 payload。
        if variant.fields.len() > 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "enum variant payload (multi-field, not boxed)",
                at: span.into(),
            });
        }

        let payload = if let Some((field_cg, field_v)) = field_values.first().copied() {
            Some(self.coerce_enum_payload_word(span, field_v, field_cg)?)
        } else {
            None
        };

        self.build_enum_value(span, enum_ty, variant.tag, payload)
    }

    fn target_layout(&self) -> TargetLayout {
        // 说明：与 typecheck::layout.rs 一致，当前阶段用 host pointer size/align 作为 layout。
        TargetLayout::host()
    }

    fn type_layout(&mut self, ty: TypeId) -> TypeLayout {
        if let Some(layout) = self.type_layout_cache.get(&ty).copied() {
            return layout;
        }

        let target = self.target_layout();

        let layout = match self.types.kind(ty) {
            TypeKind::Ref(_) => TypeLayout::new(target.pointer_size, target.pointer_align)
                .with_niche(NicheDomain {
                    storage: NicheStorage::Pointer,
                    next: 0,
                    end: target.pointer_align.max(1),
                }),
            TypeKind::Param(_) => TypeLayout::new(target.pointer_size, target.pointer_align),
            TypeKind::Value(v) => match v {
                ValueTypeKind::Unit | ValueTypeKind::Nothing => TypeLayout::new(0, 1),
                ValueTypeKind::Bool => TypeLayout::new(1, 1).with_niche(NicheDomain {
                    storage: NicheStorage::U8,
                    next: 2,
                    end: 256,
                }),
                ValueTypeKind::Int | ValueTypeKind::UInt => {
                    TypeLayout::new(target.pointer_size, target.pointer_align)
                }
                ValueTypeKind::IntN(bits) | ValueTypeKind::UIntN(bits) => {
                    let size = (u64::from(*bits) + 7) / 8;
                    let align = size.clamp(1, target.pointer_align.max(1));
                    TypeLayout::new(size, align)
                }
                ValueTypeKind::Tuple(elements) => {
                    self.aggregate_fields_layout_for_type_ids(elements)
                }
                ValueTypeKind::Option(inner) => self.option_type_layout(ty, *inner),
                ValueTypeKind::Nominal(_) => {
                    // 当前 codegen 只在 niche/boxing 决策里需要 layout 信息；nominal struct/enum 的精确布局
                    // 将在对应任务里补齐。这里按“opaque word-sized”兜底，避免过度耦合。
                    TypeLayout::new(target.pointer_size, target.pointer_align)
                }
            },
        };

        self.type_layout_cache.insert(ty, layout);
        layout
    }

    fn option_type_layout(&mut self, option_ty: TypeId, inner: TypeId) -> TypeLayout {
        // 注意：该函数只负责“niche 传播”与 `None` 编码缓存（供后续 codegen 使用）。
        if self.option_niche_cache.contains_key(&option_ty) {
            return *self
                .type_layout_cache
                .get(&option_ty)
                .unwrap_or(&TypeLayout::new(
                    self.target_layout().pointer_size,
                    self.target_layout().pointer_align,
                ));
        }

        let target = self.target_layout();
        let inner_layout = self.type_layout(inner);

        // niche path：inner 提供可用 niche domain。
        if let Some(mut domain) = inner_layout.niche {
            if let Some(none_value) = domain.take_one() {
                self.option_niche_cache
                    .insert(option_ty, Some((domain.storage, none_value)));

                let layout =
                    TypeLayout::new(inner_layout.size, inner_layout.align).with_niche(domain);
                self.type_layout_cache.insert(option_ty, layout);
                return layout;
            }
        }

        // tagged union fallback：不携带 niche。
        self.option_niche_cache.insert(option_ty, None);

        // 说明：当前 codegen 的 enum 表示仍采用 `{ tag: i32, payload: word }`，因此这里返回一个
        // “足够大”的布局即可；精确大小与 tag type 选择后续任务再统一。
        let tag_size = 4u64;
        let tag_align = 4u64;
        let payload_size = target.pointer_size;
        let payload_align = target.pointer_align;
        let payload_offset = align_to(tag_size, payload_align);
        let align = payload_align.max(tag_align);
        let size = align_to(payload_offset + payload_size, align);
        let layout = TypeLayout::new(size, align);
        self.type_layout_cache.insert(option_ty, layout);
        layout
    }

    fn aggregate_fields_layout_for_type_ids(&mut self, fields: &[TypeId]) -> TypeLayout {
        let mut size = 0u64;
        let mut align = 1u64;
        for &field in fields {
            let l = self.type_layout(field);
            size = align_to(size, l.align);
            size = size.saturating_add(l.size);
            align = align.max(l.align);
        }
        size = align_to(size, align);
        TypeLayout::new(size, align)
    }

    fn cg_enum_layout(
        &mut self,
        at: crate::span::Span,
        enum_ty: TypeId,
    ) -> Result<&CgEnumLayout, LlvmEmitError> {
        if !self.enum_cg_layout_cache.contains_key(&enum_ty) {
            let computed = self.compute_cg_enum_layout(at, enum_ty)?;
            self.enum_cg_layout_cache.insert(enum_ty, computed);
        }
        Ok(self
            .enum_cg_layout_cache
            .get(&enum_ty)
            .expect("just inserted"))
    }

    fn compute_cg_enum_layout(
        &mut self,
        at: crate::span::Span,
        enum_ty: TypeId,
    ) -> Result<CgEnumLayout, LlvmEmitError> {
        match self.types.kind(enum_ty) {
            TypeKind::Value(ValueTypeKind::Option(inner)) => {
                // 确保 option niche 缓存已被填充（用于 nested niche）。
                let _ = self.type_layout(enum_ty);
                let repr = match self.option_niche_cache.get(&enum_ty).copied().flatten() {
                    Some((storage, none_value)) => CgEnumRepr::Niche {
                        storage,
                        none_value,
                    },
                    None => CgEnumRepr::TaggedUnion,
                };

                let inner_cg = self
                    .cg_ty_of(*inner)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "Option<T> inner type",
                        at: at.into(),
                    })?;

                Ok(CgEnumLayout {
                    repr,
                    variants: vec![
                        CgEnumVariant {
                            name: "Some".to_string(),
                            tag: 0,
                            boxed: false,
                            fields: vec![inner_cg],
                        },
                        CgEnumVariant {
                            name: "None".to_string(),
                            tag: 1,
                            boxed: false,
                            fields: Vec::new(),
                        },
                    ],
                })
            }
            TypeKind::Value(ValueTypeKind::Nominal(nominal)) => {
                let hir_layout = self.enum_layouts.get(&nominal.fqn).ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "enum layout",
                        at: at.into(),
                    },
                )?;

                let mut repr = CgEnumRepr::TaggedUnion;
                if let hir::EnumRepr::ValueOnly { underlying_ty_fqn } = &hir_layout.repr {
                    let Some(underlying_ty_fqn) = underlying_ty_fqn.as_deref() else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "value-only enum underlying type",
                            at: at.into(),
                        });
                    };

                    let underlying_cg = self.cg_ty_of_type_fqn(at, Some(underlying_ty_fqn))?;
                    let CgTy::Int(underlying) = underlying_cg else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "value-only enum underlying type",
                            at: at.into(),
                        });
                    };

                    repr = CgEnumRepr::ValueOnly { underlying };
                }

                let mut variants: Vec<CgEnumVariant> =
                    Vec::with_capacity(hir_layout.variants.len());
                let mut payload_layouts: Vec<TypeLayout> =
                    Vec::with_capacity(hir_layout.variants.len());
                for v in &hir_layout.variants {
                    let mut fields = Vec::with_capacity(v.fields.len());
                    for f in &v.fields {
                        let cg = self.cg_ty_of_type_fqn(f.span, f.ty_fqn.as_deref())?;
                        fields.push(cg);
                    }
                    variants.push(CgEnumVariant {
                        name: v.name.clone(),
                        tag: v.tag,
                        boxed: false,
                        fields,
                    });

                    // value-only enum 的 ABI/layout 由底层整型决定：不做 payload/boxing 决策。
                    if !matches!(repr, CgEnumRepr::ValueOnly { .. }) {
                        payload_layouts.push(self.aggregate_fields_layout_for_cg_tys(
                            &variants.last().expect("just pushed").fields,
                        )?);
                    }
                }

                // boxing：复用 typecheck 的启发式规则（ratio + inline threshold）。
                if !matches!(repr, CgEnumRepr::ValueOnly { .. }) {
                    let target = self.target_layout();
                    let (max_size, second_size) = largest_two_sizes(&payload_layouts);
                    let inline_threshold = target
                        .pointer_size
                        .saturating_mul(ENUM_BOX_INLINE_THRESHOLD_WORDS);
                    let disparity = if second_size == 0 {
                        max_size >= inline_threshold
                    } else {
                        max_size >= inline_threshold
                            && max_size >= second_size.saturating_mul(ENUM_BOX_DISPARITY_RATIO)
                    };

                    if disparity {
                        for (v, payload) in variants.iter_mut().zip(payload_layouts.iter()) {
                            if payload.size == max_size && max_size > target.pointer_size {
                                v.boxed = true;
                            }
                        }
                    }
                }

                Ok(CgEnumLayout { repr, variants })
            }
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "enum type id",
                at: at.into(),
            }),
        }
    }

    fn aggregate_fields_layout_for_cg_tys(
        &self,
        fields: &[CgTy],
    ) -> Result<TypeLayout, LlvmEmitError> {
        let mut size = 0u64;
        let mut align = 1u64;
        for &field in fields {
            let field_layout = self.cg_ty_layout(field)?;
            size = align_to(size, field_layout.align);
            size = size.saturating_add(field_layout.size);
            align = align.max(field_layout.align);
        }
        size = align_to(size, align);
        Ok(TypeLayout::new(size, align))
    }

    fn cg_ty_layout(&self, ty: CgTy) -> Result<TypeLayout, LlvmEmitError> {
        let target = self.target_layout();
        Ok(match ty {
            CgTy::Unit => TypeLayout::new(0, 1),
            // 当前阶段 Bool 在 LLVM 中用 i1 表示，但 layout/lint/niche 计算按“存储为 u8”建模。
            CgTy::Bool => TypeLayout::new(1, 1),
            CgTy::Int(int_ty) => {
                let size = (u64::from(int_ty.bits) + 7) / 8;
                let align = size.clamp(1, target.pointer_align.max(1));
                TypeLayout::new(size, align)
            }
            CgTy::String => TypeLayout::new(target.pointer_size, target.pointer_align),
            CgTy::Ref => TypeLayout::new(target.pointer_size, target.pointer_align),
            // 兜底：composite 在当前阶段按 word-sized opaque 处理，避免错误放大。
            CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
                TypeLayout::new(target.pointer_size, target.pointer_align)
            }
        })
    }

    fn enum_payload_ty(&self) -> IntTy {
        IntTy {
            bits: self.host.word_bit_width(),
            signed: false,
        }
    }

    fn coerce_enum_payload_word(
        &mut self,
        at: crate::span::Span,
        value: CgValue<'ctx>,
        value_ty: CgTy,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let payload_ty = self.enum_payload_ty();
        let payload_int_ty = self.int_type(payload_ty);

        match value_ty {
            CgTy::Unit => Ok(payload_int_ty.const_int(0, false)),
            CgTy::Bool => {
                let b = value.as_bool().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "enum payload bool",
                    at: at.into(),
                })?;
                Ok(self
                    .builder
                    .build_int_z_extend(b, payload_int_ty, "enum_payload_bool")?)
            }
            CgTy::Int(from) => {
                let (v, _) = value.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "enum payload int",
                    at: at.into(),
                })?;
                if from.bits > payload_ty.bits {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "enum payload larger than word",
                        at: at.into(),
                    });
                }
                Ok(self.cast_int(v, from, payload_ty)?)
            }
            CgTy::String => {
                let Some(raw) = value.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "enum payload string",
                        at: at.into(),
                    });
                };
                let BasicValueEnum::PointerValue(ptr) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "enum payload string",
                        at: at.into(),
                    });
                };
                Ok(self
                    .builder
                    .build_ptr_to_int(ptr, payload_int_ty, "enum_payload_str_ptr")?)
            }
            CgTy::Ref => {
                let Some(raw) = value.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "enum payload ref",
                        at: at.into(),
                    });
                };
                let BasicValueEnum::PointerValue(ptr) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "enum payload ref",
                        at: at.into(),
                    });
                };
                Ok(self
                    .builder
                    .build_ptr_to_int(ptr, payload_int_ty, "enum_payload_ref_ptr")?)
            }
            CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
                Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "enum payload (non-scalar)",
                    at: at.into(),
                })
            }
        }
    }

    fn build_enum_value(
        &mut self,
        at: crate::span::Span,
        enum_ty: TypeId,
        tag: u64,
        payload: Option<IntValue<'ctx>>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        // 注意：`cg_enum_layout(...)` 返回的是对缓存表的引用；为了避免与后续 `&mut self` 调用产生借用冲突，
        // 这里先把需要的字段拷贝出来再继续。
        let (repr, some_field) = {
            let layout = self.cg_enum_layout(at, enum_ty)?;
            let repr = layout.repr;
            let some_field = layout
                .variants
                .iter()
                .find(|v| v.name == "Some")
                .and_then(|v| v.fields.first())
                .copied();
            (repr, some_field)
        };

        match repr {
            CgEnumRepr::TaggedUnion => {
                let llvm_enum_ty = self.llvm_enum_value_type(at, enum_ty)?;
                let llvm_enum_ty = llvm_enum_ty.into_struct_type();
                let mut agg: AggregateValueEnum<'ctx> = llvm_enum_ty.get_undef().into();

                let tag_ty = self.context.i32_type();
                let payload_ty = self.int_type(self.enum_payload_ty());

                agg = self.builder.build_insert_value(
                    agg,
                    tag_ty.const_int(tag, false),
                    0,
                    "enum_tag",
                )?;

                let payload_v = payload.unwrap_or_else(|| payload_ty.const_int(0, false));
                agg = self
                    .builder
                    .build_insert_value(agg, payload_v, 1, "enum_payload")?;

                Ok(CgValue {
                    ty: CgTy::Enum(enum_ty),
                    value: Some(agg.as_basic_value_enum()),
                })
            }
            CgEnumRepr::Niche {
                storage,
                none_value,
            } => {
                // 说明：niche 表示下 `tag` 不参与运行期布局；caller 只需要保证：
                // - `None`：payload 传 None（使用 `none_value` 作为编码）；
                // - `Some(x)`：payload 传 Some(word(x))。
                let word_ty = self.int_type(self.enum_payload_ty());
                let encoded = payload.unwrap_or_else(|| word_ty.const_int(none_value, false));

                let raw: BasicValueEnum<'ctx> = match storage {
                    NicheStorage::Pointer => {
                        // 存储类型取 `Some` variant 的字段类型（通常为指针）。
                        let some_field = some_field.ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "Option niche payload type",
                            at: at.into(),
                        })?;
                        let llvm_storage_ty = self.llvm_basic_type_of(at, some_field)?;
                        let BasicTypeEnum::PointerType(ptr_ty) = llvm_storage_ty else {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "Option niche storage (non-pointer)",
                                at: at.into(),
                            });
                        };
                        self.builder
                            .build_int_to_ptr(encoded, ptr_ty, "option_niche_ptr")?
                            .into()
                    }
                    NicheStorage::U8 => self
                        .builder
                        .build_int_truncate(encoded, self.context.i8_type(), "option_niche_u8")?
                        .into(),
                };

                Ok(CgValue {
                    ty: CgTy::Enum(enum_ty),
                    value: Some(raw),
                })
            }
            CgEnumRepr::ValueOnly { underlying } => {
                let llvm_ty = self.int_type(underlying);
                let v = llvm_ty.const_int(tag, false);
                Ok(CgValue {
                    ty: CgTy::Enum(enum_ty),
                    value: Some(v.into()),
                })
            }
        }
    }

    fn codegen_if_expr(
        &mut self,
        span: crate::span::Span,
        out_ty: TypeId,
        cond: &hir::Expr,
        then_branch: &hir::Expr,
        else_branch: Option<&hir::Expr>,
        expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let out_cg = expected
            .or_else(|| self.cg_ty_of(out_ty))
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "if output type",
                at: span.into(),
            })?;

        let cond_v = self.codegen_expr_in_expected_context(cond, Some(CgTy::Bool))?;
        let cond_v = self.coerce_value(cond.span, cond_v, CgTy::Bool)?;
        let cond_i1 = cond_v.as_bool().ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "if condition value",
            at: cond.span.into(),
        })?;

        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: span.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: span.into(),
            })?;

        let then_bb = self.context.append_basic_block(func, "if_then");
        let else_bb = self.context.append_basic_block(func, "if_else");
        let merge_bb = self.context.append_basic_block(func, "if_merge");

        self.builder
            .build_conditional_branch(cond_i1, then_bb, else_bb)?;

        let result_ptr = match out_cg {
            CgTy::Unit => None,
            _ => Some(self.create_entry_alloca(span, "if_result", out_cg)?),
        };

        // --- then ---
        self.builder.position_at_end(then_bb);
        let then_v = self.codegen_expr_in_expected_context(then_branch, Some(out_cg))?;
        let then_v = if out_cg == CgTy::Unit {
            CgValue::unit()
        } else {
            self.coerce_value(then_branch.span, then_v, out_cg)?
        };
        if let Some(ptr) = result_ptr {
            let _ = self.store_local_value(then_branch.span, ptr, out_cg, then_v)?;
        }
        if let Some(bb) = self.builder.get_insert_block() {
            if bb.get_terminator().is_none() {
                self.builder.build_unconditional_branch(merge_bb)?;
            }
        }

        // --- else ---
        self.builder.position_at_end(else_bb);
        let else_v = match else_branch {
            Some(expr) => {
                let v = self.codegen_expr_in_expected_context(expr, Some(out_cg))?;
                if out_cg == CgTy::Unit {
                    CgValue::unit()
                } else {
                    self.coerce_value(expr.span, v, out_cg)?
                }
            }
            None => {
                if out_cg != CgTy::Unit {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "if without else (non-Unit)",
                        at: span.into(),
                    });
                }
                CgValue::unit()
            }
        };
        if let Some(ptr) = result_ptr {
            let _ = self.store_local_value(span, ptr, out_cg, else_v)?;
        }
        if let Some(bb) = self.builder.get_insert_block() {
            if bb.get_terminator().is_none() {
                self.builder.build_unconditional_branch(merge_bb)?;
            }
        }

        // --- merge ---
        self.builder.position_at_end(merge_bb);
        match out_cg {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Bool | CgTy::Int(_) | CgTy::String | CgTy::Ref => {
                let Some(ptr) = result_ptr else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "if result slot",
                        at: span.into(),
                    });
                };
                let llvm_ty = self.llvm_basic_type_of(span, out_cg)?;
                let loaded = self.builder.build_load(llvm_ty, ptr, "if_result")?;
                self.cg_value_from_loaded(span, out_cg, loaded)
            }
            CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "if result type",
                at: span.into(),
            }),
        }
    }

    fn codegen_when_expr(
        &mut self,
        span: crate::span::Span,
        subject: &hir::Expr,
        arms: &[hir::WhenArm],
        expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if arms.is_empty() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "when (no arms)",
                at: span.into(),
            });
        }

        let subject_v = self.codegen_expr(subject)?;
        let subject_ty = subject_v.ty;
        let subject_raw = subject_v.value.ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "when subject value",
            at: subject.span.into(),
        })?;

        // 将 subject 落到一个栈 slot：便于在各 arm 中做 payload 解构（避免跨 block 的 dominance 细节）。
        let subject_ptr = self.create_entry_alloca(span, "when_subject", subject_ty)?;
        let _ = self.store_local_value(span, subject_ptr, subject_ty, subject_v)?;

        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: span.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: span.into(),
            })?;

        let merge_bb = self.context.append_basic_block(func, "when_merge");
        let arm_bbs = (0..arms.len())
            .map(|i| {
                self.context
                    .append_basic_block(func, &format!("when_arm_{i}"))
            })
            .collect::<Vec<_>>();

        let expected_out_ty = expected;

        let needs_chain = arms
            .iter()
            .any(|arm| arm.guard.is_some() || self.when_pat_contains_or(&arm.pat));

        if needs_chain {
            // guard / or-pattern：用“链式判别 + guard 失败回落到下一个分支”的 CFG。
            //
            // 说明：这条路径不追求最优 CFG（TODO T0825：目标是语义正确）。
            let check_bbs = (0..arms.len())
                .map(|i| {
                    self.context
                        .append_basic_block(func, &format!("when_check_{i}"))
                })
                .collect::<Vec<_>>();
            let bind_bbs = (0..arms.len())
                .map(|i| {
                    self.context
                        .append_basic_block(func, &format!("when_bind_{i}"))
                })
                .collect::<Vec<_>>();
            let no_match_bb = self.context.append_basic_block(func, "when_no_match");

            self.builder.build_unconditional_branch(check_bbs[0])?;

            for (idx, arm) in arms.iter().enumerate() {
                self.builder.position_at_end(check_bbs[idx]);
                let cond = self.codegen_when_pat_cond(span, subject_ty, &arm.pat, subject_ptr)?;
                let else_bb = if idx + 1 < arms.len() {
                    check_bbs[idx + 1]
                } else {
                    no_match_bb
                };
                self.builder
                    .build_conditional_branch(cond, bind_bbs[idx], else_bb)?;
            }

            self.builder.position_at_end(no_match_bb);
            self.builder.build_unreachable()?;

            // 生成各 arm body，并把结果汇合到 merge。
            let mut out_ty: Option<CgTy> = expected_out_ty;
            let mut incoming: Vec<(inkwell::basic_block::BasicBlock<'ctx>, CgValue<'ctx>)> =
                Vec::new();

            for (idx, arm) in arms.iter().enumerate() {
                let else_bb = if idx + 1 < arms.len() {
                    check_bbs[idx + 1]
                } else {
                    no_match_bb
                };

                // 先在 bind block 中完成 pattern binder + guard 判定，再决定是否进入 arm body。
                self.builder.position_at_end(bind_bbs[idx]);

                self.env.push_scope();
                self.bind_when_pat(span, subject_ty, &arm.pat, subject_ptr)?;

                if let Some(guard) = &arm.guard {
                    let gv = self.codegen_expr_in_expected_context(guard, Some(CgTy::Bool))?;
                    let gb = gv.as_bool().ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "when guard value",
                        at: guard.span.into(),
                    })?;
                    self.builder
                        .build_conditional_branch(gb, arm_bbs[idx], else_bb)?;
                } else {
                    self.builder.build_unconditional_branch(arm_bbs[idx])?;
                }

                // arm body：在同一作用域内生成（binder 可用）。
                self.builder.position_at_end(arm_bbs[idx]);

                let mut v = match expected_out_ty {
                    Some(target) => {
                        let v = self.codegen_expr_in_expected_context(&arm.body, Some(target))?;
                        if target == CgTy::Unit {
                            CgValue::unit()
                        } else if v.ty != target {
                            self.coerce_value(arm.body.span, v, target)?
                        } else {
                            v
                        }
                    }
                    None => self.codegen_expr(&arm.body)?,
                };

                if expected_out_ty.is_none() {
                    match out_ty {
                        None => out_ty = Some(v.ty),
                        Some(prev) if prev == v.ty => {}
                        Some(_) => {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "when arm type mismatch",
                                at: arm.body.span.into(),
                            });
                        }
                    }
                } else {
                    // 已在 expected-context 下生成并按需 coercion：确保 `v.ty == expected_out_ty`。
                    if let Some(target) = expected_out_ty {
                        v.ty = target;
                    }
                }

                let tail_bb =
                    self.builder
                        .get_insert_block()
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "when arm tail block",
                            at: arm.body.span.into(),
                        })?;
                self.builder.build_unconditional_branch(merge_bb)?;
                self.env.pop_scope();

                incoming.push((tail_bb, v));
            }

            self.builder.position_at_end(merge_bb);

            let out_ty = out_ty.unwrap_or(CgTy::Unit);
            return match out_ty {
                CgTy::Unit => Ok(CgValue::unit()),
                CgTy::Bool | CgTy::Int(_) | CgTy::String | CgTy::Ref => {
                    let phi_ty = self.llvm_basic_type_of(span, out_ty)?;
                    let phi = self.builder.build_phi(phi_ty, "when_phi")?;

                    for (bb, v) in incoming {
                        let raw = v.value.ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "when arm value",
                            at: span.into(),
                        })?;
                        phi.add_incoming(&[(&raw, bb)]);
                    }

                    Ok(CgValue {
                        ty: out_ty,
                        value: Some(phi.as_basic_value()),
                    })
                }
                CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
                    Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "when result type",
                        at: span.into(),
                    })
                }
            };
        }

        // 生成分派：enum/bool 优先降到 LLVM switch；tuple 仍用分支链并做字段比较。
        match subject_ty {
            CgTy::Enum(enum_ty) => {
                for arm in arms {
                    match &arm.pat {
                        hir::WhenPat::Else { .. }
                        | hir::WhenPat::Wildcard { .. }
                        | hir::WhenPat::Bind { .. }
                        | hir::WhenPat::Variant { .. } => {}
                        _ => {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "when pattern (enum)",
                                at: arm.pat.span().into(),
                            });
                        }
                    }
                }

                // 注意：避免持有 `cg_enum_layout(...)` 的借用跨越后续 builder 调用。
                let (repr, variants) = {
                    let cg_layout = self.cg_enum_layout(span, enum_ty)?;
                    (cg_layout.repr, cg_layout.variants.clone())
                };

                let tag = match repr {
                    CgEnumRepr::TaggedUnion => {
                        let subject_struct = subject_raw.into_struct_value();
                        self.builder
                            .build_extract_value(subject_struct, 0, "when_tag")?
                            .into_int_value()
                    }
                    CgEnumRepr::Niche {
                        storage,
                        none_value,
                    } => {
                        let is_none = match storage {
                            NicheStorage::Pointer => {
                                let ptr = subject_raw.into_pointer_value();
                                let word_ty = self.int_type(self.enum_payload_ty());
                                let as_int = self.builder.build_ptr_to_int(
                                    ptr,
                                    word_ty,
                                    "option_ptr_as_int",
                                )?;
                                let expected = word_ty.const_int(none_value, false);
                                self.builder.build_int_compare(
                                    IntPredicate::EQ,
                                    as_int,
                                    expected,
                                    "option_is_none",
                                )?
                            }
                            NicheStorage::U8 => {
                                let v = subject_raw.into_int_value();
                                let expected = self.context.i8_type().const_int(none_value, false);
                                self.builder.build_int_compare(
                                    IntPredicate::EQ,
                                    v,
                                    expected,
                                    "option_is_none",
                                )?
                            }
                        };

                        let some_tag = self.context.i32_type().const_int(0, false);
                        let none_tag = self.context.i32_type().const_int(1, false);
                        self.builder
                            .build_select(is_none, none_tag, some_tag, "option_tag")?
                            .into_int_value()
                    }
                    CgEnumRepr::ValueOnly { .. } => subject_raw.into_int_value(),
                };

                let tag_ty = tag.get_type();
                let default_bb = self.context.append_basic_block(func, "when_no_match");

                let mut cases: Vec<(IntValue<'ctx>, inkwell::basic_block::BasicBlock<'ctx>)> =
                    Vec::with_capacity(variants.len());
                for variant in &variants {
                    let Some(target_idx) =
                        self.when_first_matching_arm_for_enum_variant(arms, &variant.name)
                    else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "when missing enum arm",
                            at: span.into(),
                        });
                    };
                    cases.push((tag_ty.const_int(variant.tag, false), arm_bbs[target_idx]));
                }

                self.builder.build_switch(tag, default_bb, &cases)?;
                self.builder.position_at_end(default_bb);
                self.builder.build_unreachable()?;
            }
            CgTy::Bool => {
                for arm in arms {
                    match &arm.pat {
                        hir::WhenPat::Else { .. }
                        | hir::WhenPat::Wildcard { .. }
                        | hir::WhenPat::Bind { .. }
                        | hir::WhenPat::BoolLit { .. } => {}
                        _ => {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "when pattern (bool)",
                                at: arm.pat.span().into(),
                            });
                        }
                    }
                }

                let b = subject_raw.into_int_value();
                let bool_ty = self.context.bool_type();
                let default_bb = self.context.append_basic_block(func, "when_no_match");

                let Some(false_idx) = self.when_first_matching_arm_for_bool(arms, false) else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "when missing bool arm (false)",
                        at: span.into(),
                    });
                };
                let Some(true_idx) = self.when_first_matching_arm_for_bool(arms, true) else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "when missing bool arm (true)",
                        at: span.into(),
                    });
                };

                let cases = [
                    (bool_ty.const_int(0, false), arm_bbs[false_idx]),
                    (bool_ty.const_int(1, false), arm_bbs[true_idx]),
                ];
                self.builder.build_switch(b, default_bb, &cases)?;
                self.builder.position_at_end(default_bb);
                self.builder.build_unreachable()?;
            }
            CgTy::Tuple(tuple_ty) => {
                for arm in arms {
                    match &arm.pat {
                        hir::WhenPat::Else { .. }
                        | hir::WhenPat::Wildcard { .. }
                        | hir::WhenPat::Bind { .. }
                        | hir::WhenPat::Tuple { .. } => {}
                        _ => {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "when pattern (tuple)",
                                at: arm.pat.span().into(),
                            });
                        }
                    }
                }

                let check_bbs = (0..arms.len())
                    .map(|i| {
                        self.context
                            .append_basic_block(func, &format!("when_check_{i}"))
                    })
                    .collect::<Vec<_>>();
                let no_match_bb = self.context.append_basic_block(func, "when_no_match");

                self.builder.build_unconditional_branch(check_bbs[0])?;

                for (idx, arm) in arms.iter().enumerate() {
                    self.builder.position_at_end(check_bbs[idx]);

                    match &arm.pat {
                        hir::WhenPat::Else { .. }
                        | hir::WhenPat::Wildcard { .. }
                        | hir::WhenPat::Bind { .. } => {
                            self.builder.build_unconditional_branch(arm_bbs[idx])?;
                        }
                        hir::WhenPat::Tuple { elements, .. } => {
                            let cond = self.codegen_when_tuple_pat_cond(
                                span,
                                tuple_ty,
                                elements,
                                subject_ptr,
                            )?;
                            let else_bb = if idx + 1 < arms.len() {
                                check_bbs[idx + 1]
                            } else {
                                no_match_bb
                            };
                            self.builder
                                .build_conditional_branch(cond, arm_bbs[idx], else_bb)?;
                        }
                        _ => unreachable!("tuple patterns validated above"),
                    }
                }

                self.builder.position_at_end(no_match_bb);
                self.builder.build_unreachable()?;
            }
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "when subject type",
                    at: subject.span.into(),
                });
            }
        }

        // 生成各 arm body，并把结果汇合到 merge。
        let mut out_ty: Option<CgTy> = expected_out_ty;
        let mut incoming: Vec<(inkwell::basic_block::BasicBlock<'ctx>, CgValue<'ctx>)> = Vec::new();

        for (idx, arm) in arms.iter().enumerate() {
            self.builder.position_at_end(arm_bbs[idx]);

            self.env.push_scope();
            self.bind_when_pat(span, subject_ty, &arm.pat, subject_ptr)?;

            let mut v = match expected_out_ty {
                Some(target) => {
                    let v = self.codegen_expr_in_expected_context(&arm.body, Some(target))?;
                    if target == CgTy::Unit {
                        CgValue::unit()
                    } else if v.ty != target {
                        self.coerce_value(arm.body.span, v, target)?
                    } else {
                        v
                    }
                }
                None => self.codegen_expr(&arm.body)?,
            };

            if expected_out_ty.is_none() {
                match out_ty {
                    None => out_ty = Some(v.ty),
                    Some(prev) if prev == v.ty => {}
                    Some(_) => {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "when arm type mismatch",
                            at: arm.body.span.into(),
                        });
                    }
                }
            } else {
                if let Some(target) = expected_out_ty {
                    v.ty = target;
                }
            }

            let tail_bb =
                self.builder
                    .get_insert_block()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "when arm tail block",
                        at: arm.body.span.into(),
                    })?;
            self.builder.build_unconditional_branch(merge_bb)?;
            self.env.pop_scope();

            incoming.push((tail_bb, v));
        }

        self.builder.position_at_end(merge_bb);

        let out_ty = out_ty.unwrap_or(CgTy::Unit);
        match out_ty {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Bool | CgTy::Int(_) | CgTy::String | CgTy::Ref => {
                let phi_ty = self.llvm_basic_type_of(span, out_ty)?;
                let phi = self.builder.build_phi(phi_ty, "when_phi")?;

                for (bb, v) in incoming {
                    let raw = v.value.ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "when arm value",
                        at: span.into(),
                    })?;
                    phi.add_incoming(&[(&raw, bb)]);
                }

                Ok(CgValue {
                    ty: out_ty,
                    value: Some(phi.as_basic_value()),
                })
            }
            CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
                Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "when result type",
                    at: span.into(),
                })
            }
        }
    }

    fn bind_when_pat(
        &mut self,
        at: crate::span::Span,
        subject_ty: CgTy,
        pat: &hir::WhenPat,
        subject_ptr: PointerValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        match pat {
            hir::WhenPat::Else { .. }
            | hir::WhenPat::Or { .. }
            | hir::WhenPat::Wildcard { .. }
            | hir::WhenPat::Rest { .. }
            | hir::WhenPat::Is { .. }
            | hir::WhenPat::IntLit { .. }
            | hir::WhenPat::StringLit { .. }
            | hir::WhenPat::BoolLit { .. } => Ok(()),
            hir::WhenPat::Bind { id, name, .. } => {
                // `x -> ...`：绑定整个 subject。
                let ptr = self.create_entry_alloca(at, name, subject_ty)?;
                let llvm_ty = self.llvm_basic_type_of(at, subject_ty)?;
                let loaded = self
                    .builder
                    .build_load(llvm_ty, subject_ptr, "bind_subject")?;
                let v = CgValue {
                    ty: subject_ty,
                    value: Some(loaded),
                };
                let _ = self.store_local_value(at, ptr, subject_ty, v)?;
                self.env.insert(
                    *id,
                    CgLocal {
                        hir_ty: None,
                        ty: subject_ty,
                        ptr,
                        mutable: false,
                        gc_root_slot: None,
                    },
                );
                Ok(())
            }
            hir::WhenPat::Variant { name, args, .. } => {
                let CgTy::Enum(enum_ty) = subject_ty else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "when variant pattern subject type",
                        at: pat.span().into(),
                    });
                };

                let (repr, variant) = {
                    let cg_layout = self.cg_enum_layout(at, enum_ty)?;
                    let repr = cg_layout.repr;
                    let variant = cg_layout
                        .variants
                        .iter()
                        .find(|v| v.name == *name)
                        .cloned()
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "when unknown enum variant",
                            at: pat.span().into(),
                        })?;
                    (repr, variant)
                };

                // 解析 `..`：parser/typecheck 已保证它最多出现一次且必须出现在最后一个位置。
                let (prefix_pats, has_rest) = match args.last() {
                    Some(hir::WhenPat::Rest { .. }) => {
                        (&args[..args.len().saturating_sub(1)], true)
                    }
                    _ => (args.as_slice(), false),
                };

                let expected_arity = variant.fields.len();
                let found_arity = prefix_pats.len();
                if (!has_rest && expected_arity != found_arity)
                    || (has_rest && found_arity > expected_arity)
                {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "when variant arity mismatch",
                        at: pat.span().into(),
                    });
                }

                if prefix_pats.is_empty() {
                    return Ok(());
                }

                // boxed variant：payload 是指向“payload struct”的指针（存放所有字段）。
                if variant.boxed {
                    let llvm_enum_ty = self.llvm_enum_value_type(at, enum_ty)?.into_struct_type();
                    let loaded =
                        self.builder
                            .build_load(llvm_enum_ty, subject_ptr, "load_when_subject")?;
                    let raw_struct = loaded.into_struct_value();
                    let payload_word = self
                        .builder
                        .build_extract_value(raw_struct, 1, "when_payload")?
                        .into_int_value();

                    let payload_struct_ty =
                        self.llvm_enum_boxed_payload_struct_type(at, enum_ty, &variant)?;
                    let payload_ptr = self.builder.build_int_to_ptr(
                        payload_word,
                        payload_struct_ty.ptr_type(AddressSpace::default()),
                        "when_payload_ptr",
                    )?;
                    let payload_loaded = self
                        .builder
                        .build_load(payload_struct_ty, payload_ptr, "load_when_payload")?
                        .into_struct_value();

                    for (idx, arg_pat) in prefix_pats.iter().enumerate() {
                        let field_cg =
                            *variant
                                .fields
                                .get(idx)
                                .ok_or(LlvmEmitError::UnsupportedMainBody {
                                    kind: "when boxed payload field index",
                                    at: arg_pat.span().into(),
                                })?;

                        match arg_pat {
                            hir::WhenPat::Bind { id, name, .. } => {
                                let raw = self.builder.build_extract_value(
                                    payload_loaded,
                                    idx as u32,
                                    "when_payload_field",
                                )?;
                                let extracted =
                                    self.cg_value_from_loaded(arg_pat.span(), field_cg, raw)?;

                                let ptr = self.create_entry_alloca(at, name, field_cg)?;
                                let _ = self.store_local_value(at, ptr, field_cg, extracted)?;
                                self.env.insert(
                                    *id,
                                    CgLocal {
                                        hir_ty: None,
                                        ty: field_cg,
                                        ptr,
                                        mutable: false,
                                        gc_root_slot: None,
                                    },
                                );
                            }
                            hir::WhenPat::Wildcard { .. } => {}
                            hir::WhenPat::Rest { .. } => break,
                            _ => {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "when variant arg pattern",
                                    at: arg_pat.span().into(),
                                });
                            }
                        }
                    }

                    return Ok(());
                }

                // niche enum（当前仅 Option<T>）：payload 就是 enum 本身。
                if matches!(repr, CgEnumRepr::Niche { .. }) {
                    if variant.fields.len() != 1 {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "niche enum variant arity",
                            at: pat.span().into(),
                        });
                    }

                    let field_cg = variant.fields[0];
                    let llvm_enum_ty = self.llvm_enum_value_type(at, enum_ty)?;
                    let loaded =
                        self.builder
                            .build_load(llvm_enum_ty, subject_ptr, "load_when_subject")?;

                    // 存储类型可能与字段类型不同（例如 `Option<Bool>` 存储为 u8）。
                    let extracted = match field_cg {
                        CgTy::Bool => {
                            let b = self.builder.build_int_truncate(
                                loaded.into_int_value(),
                                self.context.bool_type(),
                                "option_bool_from_u8",
                            )?;
                            CgValue::bool(b)
                        }
                        CgTy::String => CgValue {
                            ty: CgTy::String,
                            value: Some(loaded.into_pointer_value().into()),
                        },
                        CgTy::Ref => CgValue {
                            ty: CgTy::Ref,
                            value: Some(loaded.into_pointer_value().into()),
                        },
                        CgTy::Unit
                        | CgTy::Int(_)
                        | CgTy::Tuple(_)
                        | CgTy::Struct(_)
                        | CgTy::Enum(_) => {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "niche enum payload type",
                                at: pat.span().into(),
                            });
                        }
                    };

                    // niche enum 的 binder 只能绑定第一个字段（且 rest 可能忽略其余）。
                    let Some(first_pat) = prefix_pats.first() else {
                        return Ok(());
                    };
                    match first_pat {
                        hir::WhenPat::Bind { id, name, .. } => {
                            let ptr = self.create_entry_alloca(at, name, field_cg)?;
                            let _ = self.store_local_value(at, ptr, field_cg, extracted)?;
                            self.env.insert(
                                *id,
                                CgLocal {
                                    hir_ty: None,
                                    ty: field_cg,
                                    ptr,
                                    mutable: false,
                                    gc_root_slot: None,
                                },
                            );
                        }
                        hir::WhenPat::Wildcard { .. } | hir::WhenPat::Rest { .. } => {}
                        _ => {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "when variant arg pattern",
                                at: first_pat.span().into(),
                            });
                        }
                    }

                    return Ok(());
                }

                // inline tagged union：仍只支持 “小 payload”（单字段标量）。
                if variant.fields.len() != 1 || prefix_pats.len() != 1 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "when variant payload (inline, unsupported arity)",
                        at: pat.span().into(),
                    });
                }

                let field_cg = variant.fields[0];
                let arg_pat = &prefix_pats[0];

                let llvm_enum_ty = self.llvm_enum_value_type(at, enum_ty)?.into_struct_type();
                let loaded =
                    self.builder
                        .build_load(llvm_enum_ty, subject_ptr, "load_when_subject")?;
                let raw_struct = loaded.into_struct_value();
                let payload_raw = self
                    .builder
                    .build_extract_value(raw_struct, 1, "when_payload")?
                    .into_int_value();

                // 当前阶段 payload 固定为 word-sized int；按字段类型截断/转换。
                let extracted = match field_cg {
                    CgTy::Unit => CgValue::unit(),
                    CgTy::Bool => {
                        let b = self.builder.build_int_truncate(
                            payload_raw,
                            self.context.bool_type(),
                            "payload_to_bool",
                        )?;
                        CgValue::bool(b)
                    }
                    CgTy::Int(int_ty) => {
                        let from = self.enum_payload_ty();
                        let casted = self.cast_int(payload_raw, from, int_ty)?;
                        CgValue::int(casted, int_ty)
                    }
                    CgTy::String => {
                        let ptr = self.builder.build_int_to_ptr(
                            payload_raw,
                            self.llvm_scoop_string_ptr_type(),
                            "payload_to_str_ptr",
                        )?;
                        CgValue {
                            ty: CgTy::String,
                            value: Some(ptr.into()),
                        }
                    }
                    CgTy::Ref => {
                        let ptr = self.builder.build_int_to_ptr(
                            payload_raw,
                            self.context.i8_type().ptr_type(AddressSpace::default()),
                            "payload_to_ref_ptr",
                        )?;
                        CgValue {
                            ty: CgTy::Ref,
                            value: Some(ptr.into()),
                        }
                    }
                    CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "when payload (non-scalar)",
                            at: arg_pat.span().into(),
                        });
                    }
                };

                match arg_pat {
                    hir::WhenPat::Bind { id, name, .. } => {
                        let ptr = self.create_entry_alloca(at, name, field_cg)?;
                        let _ = self.store_local_value(at, ptr, field_cg, extracted)?;
                        self.env.insert(
                            *id,
                            CgLocal {
                                hir_ty: None,
                                ty: field_cg,
                                ptr,
                                mutable: false,
                                gc_root_slot: None,
                            },
                        );
                    }
                    hir::WhenPat::Wildcard { .. } | hir::WhenPat::Rest { .. } => {}
                    _ => {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "when variant arg pattern",
                            at: arg_pat.span().into(),
                        });
                    }
                }

                Ok(())
            }
            hir::WhenPat::Tuple { elements, .. } => {
                let CgTy::Tuple(tuple_ty) = subject_ty else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "when tuple pattern subject type",
                        at: pat.span().into(),
                    });
                };

                let TypeKind::Value(ValueTypeKind::Tuple(tuple_elems)) = self.types.kind(tuple_ty)
                else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "tuple type id",
                        at: pat.span().into(),
                    });
                };

                let mut has_rest = false;
                for (idx, elem_pat) in elements.iter().enumerate() {
                    if matches!(elem_pat, hir::WhenPat::Rest { .. }) {
                        if idx + 1 != elements.len() {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "when tuple pattern rest position",
                                at: elem_pat.span().into(),
                            });
                        }
                        has_rest = true;
                        break;
                    }
                }

                let pat_arity = if has_rest {
                    elements.len().saturating_sub(1)
                } else {
                    elements.len()
                };

                if (!has_rest && pat_arity != tuple_elems.len())
                    || (has_rest && pat_arity > tuple_elems.len())
                {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "when tuple pattern arity mismatch",
                        at: pat.span().into(),
                    });
                }

                let llvm_tuple_ty = self.llvm_tuple_type(at, tuple_ty)?;
                let loaded =
                    self.builder
                        .build_load(llvm_tuple_ty, subject_ptr, "load_when_tuple")?;
                let tuple_v = loaded.into_struct_value();

                for (idx, elem_pat) in elements.iter().enumerate() {
                    if matches!(elem_pat, hir::WhenPat::Rest { .. }) {
                        break;
                    }
                    let elem_ty =
                        self.lookup_tuple_element(tuple_ty, idx as u32, elem_pat.span())?;

                    let extracted_v = if elem_ty == CgTy::Unit {
                        CgValue::unit()
                    } else {
                        let raw = self.builder.build_extract_value(
                            tuple_v,
                            idx as u32,
                            "when_tuple_elem",
                        )?;
                        self.cg_value_from_loaded(elem_pat.span(), elem_ty, raw)?
                    };

                    match elem_pat {
                        hir::WhenPat::Bind { .. } => {
                            // 直接把元素作为 subject 绑定（避免额外临时 slot）。
                            let hir::WhenPat::Bind { id, name, .. } = elem_pat else {
                                unreachable!()
                            };
                            let ptr = self.create_entry_alloca(at, name, elem_ty)?;
                            let _ = self.store_local_value(at, ptr, elem_ty, extracted_v)?;
                            self.env.insert(
                                *id,
                                CgLocal {
                                    hir_ty: None,
                                    ty: elem_ty,
                                    ptr,
                                    mutable: false,
                                    gc_root_slot: None,
                                },
                            );
                        }
                        hir::WhenPat::Tuple { .. } | hir::WhenPat::Variant { .. } => {
                            // 递归绑定：需要一个临时 slot 让子 pattern 能 load/extract。
                            let tmp_name = format!("when_tuple_elem_{idx}");
                            let tmp_ptr = self.create_entry_alloca(at, &tmp_name, elem_ty)?;
                            let _ = self.store_local_value(at, tmp_ptr, elem_ty, extracted_v)?;
                            self.bind_when_pat(at, elem_ty, elem_pat, tmp_ptr)?;
                        }
                        _ => {}
                    }
                }

                Ok(())
            }
        }
    }

    fn when_pat_contains_or(&self, pat: &hir::WhenPat) -> bool {
        match pat {
            hir::WhenPat::Or { .. } => true,
            hir::WhenPat::Tuple { elements, .. } => {
                elements.iter().any(|p| self.when_pat_contains_or(p))
            }
            hir::WhenPat::Variant { args, .. } => args.iter().any(|p| self.when_pat_contains_or(p)),
            _ => false,
        }
    }

    fn codegen_when_pat_cond(
        &mut self,
        at: crate::span::Span,
        subject_ty: CgTy,
        pat: &hir::WhenPat,
        subject_ptr: PointerValue<'ctx>,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        match subject_ty {
            CgTy::Enum(enum_ty) => {
                self.codegen_when_pat_cond_for_enum(at, enum_ty, pat, subject_ptr)
            }
            CgTy::Bool => self.codegen_when_pat_cond_for_bool(at, pat, subject_ptr),
            CgTy::Tuple(tuple_ty) => {
                self.codegen_when_pat_cond_for_tuple(at, tuple_ty, pat, subject_ptr)
            }
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "when subject type",
                at: at.into(),
            }),
        }
    }

    fn codegen_when_pat_cond_for_enum(
        &mut self,
        at: crate::span::Span,
        enum_ty: TypeId,
        pat: &hir::WhenPat,
        subject_ptr: PointerValue<'ctx>,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        // 注意：避免持有 `cg_enum_layout(...)` 的借用跨越后续 builder 调用。
        let (repr, variants) = {
            let cg_layout = self.cg_enum_layout(at, enum_ty)?;
            (cg_layout.repr, cg_layout.variants.clone())
        };
        let llvm_enum_ty = self.llvm_enum_value_type(at, enum_ty)?;
        let loaded = self
            .builder
            .build_load(llvm_enum_ty, subject_ptr, "load_when_subject")?;

        let tag = match repr {
            CgEnumRepr::TaggedUnion => {
                let raw_struct = loaded.into_struct_value();
                self.builder
                    .build_extract_value(raw_struct, 0, "when_tag")?
                    .into_int_value()
            }
            CgEnumRepr::Niche {
                storage,
                none_value,
            } => {
                let is_none = match storage {
                    NicheStorage::Pointer => {
                        let ptr = loaded.into_pointer_value();
                        let word_ty = self.int_type(self.enum_payload_ty());
                        let as_int =
                            self.builder
                                .build_ptr_to_int(ptr, word_ty, "option_ptr_as_int")?;
                        let expected = word_ty.const_int(none_value, false);
                        self.builder.build_int_compare(
                            IntPredicate::EQ,
                            as_int,
                            expected,
                            "option_is_none",
                        )?
                    }
                    NicheStorage::U8 => {
                        let v = loaded.into_int_value();
                        let expected = self.context.i8_type().const_int(none_value, false);
                        self.builder.build_int_compare(
                            IntPredicate::EQ,
                            v,
                            expected,
                            "option_is_none",
                        )?
                    }
                };

                let some_tag = self.context.i32_type().const_int(0, false);
                let none_tag = self.context.i32_type().const_int(1, false);
                self.builder
                    .build_select(is_none, none_tag, some_tag, "option_tag")?
                    .into_int_value()
            }
            CgEnumRepr::ValueOnly { .. } => loaded.into_int_value(),
        };

        self.codegen_when_pat_cond_for_enum_with_tag(at, &variants, tag, pat)
    }

    fn codegen_when_pat_cond_for_enum_with_tag(
        &self,
        at: crate::span::Span,
        variants: &[CgEnumVariant],
        tag: IntValue<'ctx>,
        pat: &hir::WhenPat,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        match pat {
            hir::WhenPat::Else { .. }
            | hir::WhenPat::Wildcard { .. }
            | hir::WhenPat::Bind { .. } => Ok(self.context.bool_type().const_int(1, false)),
            hir::WhenPat::Variant { name, args, .. } => {
                let Some(variant) = variants.iter().find(|v| v.name == *name) else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "when unknown enum variant",
                        at: pat.span().into(),
                    });
                };
                let _ = args;

                let expected = tag.get_type().const_int(variant.tag, false);
                Ok(self.builder.build_int_compare(
                    IntPredicate::EQ,
                    tag,
                    expected,
                    "when_enum_tag_eq",
                )?)
            }
            hir::WhenPat::Or { pats, .. } => {
                let mut cond = self.context.bool_type().const_int(0, false);
                for p in pats {
                    let c = self.codegen_when_pat_cond_for_enum_with_tag(at, variants, tag, p)?;
                    cond = self.builder.build_or(cond, c, "when_or")?;
                }
                Ok(cond)
            }
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "when pattern (enum)",
                at: pat.span().into(),
            }),
        }
    }

    fn codegen_when_pat_cond_for_bool(
        &mut self,
        at: crate::span::Span,
        pat: &hir::WhenPat,
        subject_ptr: PointerValue<'ctx>,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let loaded = self
            .builder
            .build_load(self.context.bool_type(), subject_ptr, "load_when_bool")?
            .into_int_value();
        self.codegen_when_pat_cond_for_bool_with_value(at, loaded, pat)
    }

    fn codegen_when_pat_cond_for_bool_with_value(
        &self,
        _at: crate::span::Span,
        value: IntValue<'ctx>,
        pat: &hir::WhenPat,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        match pat {
            hir::WhenPat::Else { .. }
            | hir::WhenPat::Wildcard { .. }
            | hir::WhenPat::Bind { .. } => Ok(self.context.bool_type().const_int(1, false)),
            hir::WhenPat::BoolLit {
                value: expected, ..
            } => {
                let expected = self.context.bool_type().const_int(*expected as u64, false);
                Ok(self.builder.build_int_compare(
                    IntPredicate::EQ,
                    value,
                    expected,
                    "when_bool_eq",
                )?)
            }
            hir::WhenPat::Or { pats, .. } => {
                let mut cond = self.context.bool_type().const_int(0, false);
                for p in pats {
                    let c = self.codegen_when_pat_cond_for_bool_with_value(_at, value, p)?;
                    cond = self.builder.build_or(cond, c, "when_or")?;
                }
                Ok(cond)
            }
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "when pattern (bool)",
                at: pat.span().into(),
            }),
        }
    }

    fn codegen_when_pat_cond_for_tuple(
        &mut self,
        at: crate::span::Span,
        tuple_ty: TypeId,
        pat: &hir::WhenPat,
        subject_ptr: PointerValue<'ctx>,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        match pat {
            hir::WhenPat::Else { .. }
            | hir::WhenPat::Wildcard { .. }
            | hir::WhenPat::Bind { .. } => Ok(self.context.bool_type().const_int(1, false)),
            hir::WhenPat::Tuple { elements, .. } => {
                self.codegen_when_tuple_pat_cond(at, tuple_ty, elements, subject_ptr)
            }
            hir::WhenPat::Or { pats, .. } => {
                let mut cond = self.context.bool_type().const_int(0, false);
                for p in pats {
                    let c = self.codegen_when_pat_cond_for_tuple(at, tuple_ty, p, subject_ptr)?;
                    cond = self.builder.build_or(cond, c, "when_or")?;
                }
                Ok(cond)
            }
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "when pattern (tuple)",
                at: pat.span().into(),
            }),
        }
    }

    fn when_first_matching_arm_for_enum_variant(
        &self,
        arms: &[hir::WhenArm],
        variant_name: &str,
    ) -> Option<usize> {
        for (idx, arm) in arms.iter().enumerate() {
            match &arm.pat {
                hir::WhenPat::Else { .. }
                | hir::WhenPat::Wildcard { .. }
                | hir::WhenPat::Bind { .. } => return Some(idx),
                hir::WhenPat::Variant { name, .. } if name == variant_name => return Some(idx),
                _ => {}
            }
        }
        None
    }

    fn when_first_matching_arm_for_bool(
        &self,
        arms: &[hir::WhenArm],
        value: bool,
    ) -> Option<usize> {
        for (idx, arm) in arms.iter().enumerate() {
            match &arm.pat {
                hir::WhenPat::Else { .. }
                | hir::WhenPat::Wildcard { .. }
                | hir::WhenPat::Bind { .. } => return Some(idx),
                hir::WhenPat::BoolLit { value: v, .. } if *v == value => return Some(idx),
                _ => {}
            }
        }
        None
    }

    fn codegen_when_tuple_pat_cond(
        &mut self,
        at: crate::span::Span,
        tuple_ty: TypeId,
        elements: &[hir::WhenPat],
        subject_ptr: PointerValue<'ctx>,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let TypeKind::Value(ValueTypeKind::Tuple(tuple_elems)) = self.types.kind(tuple_ty) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "tuple type id",
                at: at.into(),
            });
        };

        let mut rest_idx: Option<usize> = None;
        for (idx, pat) in elements.iter().enumerate() {
            if matches!(pat, hir::WhenPat::Rest { .. }) {
                rest_idx = Some(idx);
                break;
            }
        }

        if let Some(rest) = rest_idx {
            if rest + 1 != elements.len() {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "when tuple pattern rest position",
                    at: elements[rest].span().into(),
                });
            }
        }

        let pat_arity = rest_idx.unwrap_or(elements.len());
        if (rest_idx.is_none() && pat_arity != tuple_elems.len())
            || (rest_idx.is_some() && pat_arity > tuple_elems.len())
        {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "when tuple pattern arity mismatch",
                at: at.into(),
            });
        }

        let llvm_tuple_ty = self.llvm_tuple_type(at, tuple_ty)?;
        let loaded = self
            .builder
            .build_load(llvm_tuple_ty, subject_ptr, "load_when_tuple")?;
        let tuple_v = loaded.into_struct_value();

        let mut cond = self.context.bool_type().const_int(1, false);
        for (idx, elem_pat) in elements.iter().enumerate().take(pat_arity) {
            let elem_ty = self.lookup_tuple_element(tuple_ty, idx as u32, elem_pat.span())?;
            let elem_cond = self.codegen_when_pat_cond_for_tuple_elem(
                at, tuple_ty, idx, elem_ty, tuple_v, elem_pat,
            )?;
            cond = self.builder.build_and(cond, elem_cond, "when_tuple_and")?;
        }
        Ok(cond)
    }

    fn codegen_when_pat_cond_for_tuple_elem(
        &mut self,
        at: crate::span::Span,
        tuple_ty: TypeId,
        elem_idx: usize,
        elem_ty: CgTy,
        tuple_v: inkwell::values::StructValue<'ctx>,
        pat: &hir::WhenPat,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        match pat {
            hir::WhenPat::Wildcard { .. }
            | hir::WhenPat::Bind { .. }
            | hir::WhenPat::Rest { .. } => Ok(self.context.bool_type().const_int(1, false)),
            hir::WhenPat::BoolLit { value, .. } => {
                let CgTy::Bool = elem_ty else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "when tuple elem bool pattern type",
                        at: pat.span().into(),
                    });
                };
                let raw = self
                    .builder
                    .build_extract_value(tuple_v, elem_idx as u32, "when_tuple_elem")?
                    .into_int_value();
                let expected = self.context.bool_type().const_int(*value as u64, false);
                Ok(self.builder.build_int_compare(
                    IntPredicate::EQ,
                    raw,
                    expected,
                    "when_tuple_bool_eq",
                )?)
            }
            hir::WhenPat::IntLit { span } => {
                let CgTy::Int(int_ty) = elem_ty else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "when tuple elem int pattern type",
                        at: pat.span().into(),
                    });
                };
                let raw = self
                    .builder
                    .build_extract_value(tuple_v, elem_idx as u32, "when_tuple_elem")?
                    .into_int_value();
                let text = self.source.slice(*span);
                let value = parse_int_literal_decimal(text);
                let value = mask_to_bits(value, int_ty.bits) as u64;
                let expected = self.int_type(int_ty).const_int(value, false);
                Ok(self.builder.build_int_compare(
                    IntPredicate::EQ,
                    raw,
                    expected,
                    "when_tuple_int_eq",
                )?)
            }
            hir::WhenPat::Tuple { elements, .. } => {
                let CgTy::Tuple(nested_tuple_ty) = elem_ty else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "when tuple elem tuple pattern type",
                        at: pat.span().into(),
                    });
                };

                let TypeKind::Value(ValueTypeKind::Tuple(_)) = self.types.kind(nested_tuple_ty)
                else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "tuple type id",
                        at: pat.span().into(),
                    });
                };

                // 由于 extractvalue 返回的是一个“by-value tuple struct”，我们先把它落到临时 slot，
                // 再复用 `codegen_when_tuple_pat_cond` 的逻辑生成递归比较。
                let nested_raw = self.builder.build_extract_value(
                    tuple_v,
                    elem_idx as u32,
                    "when_tuple_elem",
                )?;
                let nested_value = self.cg_value_from_loaded(pat.span(), elem_ty, nested_raw)?;
                let tmp_name = format!("when_tuple_nested_{}_{}", tuple_ty.as_u32(), elem_idx);
                let tmp_ptr = self.create_entry_alloca(at, &tmp_name, elem_ty)?;
                let _ = self.store_local_value(at, tmp_ptr, elem_ty, nested_value)?;
                self.codegen_when_tuple_pat_cond(at, nested_tuple_ty, elements, tmp_ptr)
            }
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "when tuple pattern",
                at: pat.span().into(),
            }),
        }
    }

    fn llvm_param_ty(
        &mut self,
        span: crate::span::Span,
        ty: TypeId,
    ) -> Result<BasicMetadataTypeEnum<'ctx>, LlvmEmitError> {
        let cg = self
            .cg_ty_of(ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "function param type",
                at: span.into(),
            })?;

        Ok(self.llvm_basic_type_of(span, cg)?.into())
    }

    fn as_llvm_arg_value(
        &self,
        span: crate::span::Span,
        param_ty: CgTy,
        value: CgValue<'ctx>,
    ) -> Result<inkwell::values::BasicMetadataValueEnum<'ctx>, LlvmEmitError> {
        Ok(match param_ty {
            CgTy::Unit => self.context.i8_type().const_int(0, false).into(),
            CgTy::Bool
            | CgTy::Int(_)
            | CgTy::String
            | CgTy::Ref
            | CgTy::Tuple(_)
            | CgTy::Struct(_)
            | CgTy::Enum(_) => value
                .value
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "call arg value",
                    at: span.into(),
                })?
                .into(),
        })
    }

    fn codegen_fun_params(
        &mut self,
        fun: &hir::FunDecl,
        llvm_fun: FunctionValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        for (idx, param) in fun.params.iter().enumerate() {
            let target_ty = self
                .cg_ty_of(param.ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "param type",
                    at: param.span.into(),
                })?;

            let ptr = self.create_entry_alloca(param.span, &param.name, target_ty)?;
            let init = match target_ty {
                CgTy::Unit => CgValue::unit(),
                CgTy::Bool => {
                    let raw = llvm_fun
                        .get_nth_param(idx as u32)
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "missing llvm param",
                            at: param.span.into(),
                        })?
                        .into_int_value();
                    CgValue::bool(raw)
                }
                CgTy::Int(int_ty) => {
                    let raw = llvm_fun
                        .get_nth_param(idx as u32)
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "missing llvm param",
                            at: param.span.into(),
                        })?
                        .into_int_value();
                    CgValue::int(raw, int_ty)
                }
                CgTy::String => {
                    let raw = llvm_fun
                        .get_nth_param(idx as u32)
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "missing llvm param",
                            at: param.span.into(),
                        })?
                        .into_pointer_value();
                    CgValue {
                        ty: CgTy::String,
                        value: Some(raw.into()),
                    }
                }
                CgTy::Ref => {
                    let raw = llvm_fun
                        .get_nth_param(idx as u32)
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "missing llvm param",
                            at: param.span.into(),
                        })?
                        .into_pointer_value();
                    CgValue {
                        ty: CgTy::Ref,
                        value: Some(raw.into()),
                    }
                }
                CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => {
                    let raw = llvm_fun
                        .get_nth_param(idx as u32)
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "missing llvm param",
                            at: param.span.into(),
                        })?;
                    CgValue {
                        ty: target_ty,
                        value: Some(raw),
                    }
                }
            };

            let stored = self.store_local_value(param.span, ptr, target_ty, init)?;
            let gc_root_slot = self.gc_root_slot_for(param.id);
            if let Some(slot_ptr) = gc_root_slot {
                self.store_gc_root_slot_value(param.span, slot_ptr, stored)?;
            }
            self.env.insert(
                param.id,
                CgLocal {
                    hir_ty: Some(param.ty),
                    ty: target_ty,
                    ptr,
                    mutable: false,
                    gc_root_slot,
                },
            );
        }
        Ok(())
    }

    fn codegen_block_as_return_value(
        &mut self,
        block: &hir::Block,
        declared_return_ty: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let mut tail_value: Option<CgValue<'ctx>> = None;

        self.env.push_scope();

        for (idx, stmt) in block.stmts.iter().enumerate() {
            let is_last = idx + 1 == block.stmts.len();
            match &stmt.kind {
                hir::StmtKind::Empty => {}
                hir::StmtKind::Val(decl) => {
                    self.codegen_val_decl(decl)?;
                    tail_value = None;
                }
                hir::StmtKind::Assign { lhs, eq_span, rhs } => {
                    self.codegen_assign_stmt(*eq_span, lhs, rhs)?;
                    tail_value = None;
                }
                hir::StmtKind::Expr(expr) => {
                    let expected = if is_last {
                        Some(declared_return_ty)
                    } else {
                        Some(CgTy::Unit)
                    };
                    let v = self.codegen_expr_in_expected_context(expr, expected)?;
                    tail_value = if is_last { Some(v) } else { None };
                }
                hir::StmtKind::Return { value } => {
                    let out = match value {
                        Some(expr) => {
                            let v =
                                self.codegen_expr_in_expected_context(expr, Some(declared_return_ty))?;
                            if declared_return_ty == CgTy::Unit {
                                CgValue::unit()
                            } else {
                                self.coerce_value(expr.span, v, declared_return_ty)?
                            }
                        }
                        None => self.default_value(declared_return_ty),
                    };

                    self.env.pop_scope();
                    return Ok(out);
                }
                hir::StmtKind::While { cond, body } => {
                    self.codegen_while_stmt(stmt.span, cond, body)?;
                    tail_value = None;
                }
                hir::StmtKind::Break { .. }
                | hir::StmtKind::Continue { .. }
                | hir::StmtKind::Todo(_) => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "statement",
                        at: stmt.span.into(),
                    });
                }
            }
        }

        let out = if let Some(v) = tail_value {
            if declared_return_ty == CgTy::Unit {
                CgValue::unit()
            } else {
                self.coerce_value(block.span, v, declared_return_ty)?
            }
        } else {
            self.default_value(declared_return_ty)
        };

        self.env.pop_scope();
        Ok(out)
    }

    fn default_value(&self, ty: CgTy) -> CgValue<'ctx> {
        match ty {
            CgTy::Unit => CgValue::unit(),
            CgTy::Bool => CgValue::bool(self.context.bool_type().const_int(0, false)),
            CgTy::Int(int_ty) => CgValue::int(self.int_type(int_ty).const_int(0, false), int_ty),
            CgTy::String => CgValue {
                ty: CgTy::String,
                value: Some(self.llvm_scoop_string_ptr_type().const_null().into()),
            },
            CgTy::Ref => CgValue {
                ty: CgTy::Ref,
                value: Some(
                    self.llvm_gc_i8_ptr_type().const_null().into(),
                ),
            },
            // 说明：当前阶段不支持 tuple/struct 作为函数返回类型，因此这里仅提供占位值；
            // 若后续误用，会在 emit/store 阶段触发结构化错误而非 panic。
            CgTy::Tuple(ty) => CgValue {
                ty: CgTy::Tuple(ty),
                value: None,
            },
            CgTy::Struct(ty) => CgValue {
                ty: CgTy::Struct(ty),
                value: None,
            },
            CgTy::Enum(ty) => CgValue {
                ty: CgTy::Enum(ty),
                value: None,
            },
        }
    }

    fn declare_libc_exit(&self) -> FunctionValue<'ctx> {
        if let Some(f) = self.module.get_function("exit") {
            return f;
        }
        let fn_ty = self
            .context
            .void_type()
            .fn_type(&[self.context.i32_type().into()], false);
        self.module.add_function("exit", fn_ty, None)
    }

    fn declare_libc_malloc(&self) -> FunctionValue<'ctx> {
        if let Some(f) = self.module.get_function("malloc") {
            return f;
        }

        // `void* malloc(size_t size)`：这里用 `i64` 作为 size（host 64-bit 场景；32-bit 下会被 truncate）。
        let i8_ptr_ty = self.context.i8_type().ptr_type(AddressSpace::default());
        let size_ty = self.context.i64_type();
        let fn_ty = i8_ptr_ty.fn_type(&[size_ty.into()], false);
        self.module.add_function("malloc", fn_ty, None)
    }

    fn emit_exit_with_code(
        &mut self,
        at: crate::span::Span,
        code: i32,
    ) -> Result<(), LlvmEmitError> {
        let exit = self.declare_libc_exit();
        let code_i32 = self.context.i32_type().const_int(code as u64, false);
        let _ = self.builder.build_call(exit, &[code_i32.into()], "exit")?;
        self.builder.build_unreachable()?;
        let _ = at;
        Ok(())
    }

    fn emit_return(
        &mut self,
        span: crate::span::Span,
        declared_return_ty: CgTy,
        value: CgValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        if let Some(gc) = self.gc_frame.as_ref() {
            let pop = self.declare_runtime_gc_frame_pop();
            let i8_ptr_ty = self.context.i8_type().ptr_type(AddressSpace::default());
            let frame_i8 =
                self.builder
                    .build_pointer_cast(gc.frame_ptr, i8_ptr_ty, "gc_frame_i8")?;
            let _ = self
                .builder
                .build_call(pop, &[frame_i8.into()], "gc_frame_pop")?;
        }

        match declared_return_ty {
            CgTy::Unit => {
                self.builder.build_return(None)?;
                Ok(())
            }
            CgTy::Bool
            | CgTy::Int(_)
            | CgTy::String
            | CgTy::Ref
            | CgTy::Tuple(_)
            | CgTy::Struct(_)
            | CgTy::Enum(_) => {
                let Some(raw) = value.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "return value",
                        at: span.into(),
                    });
                };
                self.builder.build_return(Some(&raw))?;
                Ok(())
            }
        }
    }

    fn codegen_block_value(&mut self, block: &hir::Block) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.codegen_block_value_in_expected_context(block, None)
    }

    fn codegen_block_value_in_expected_context(
        &mut self,
        block: &hir::Block,
        expected: Option<CgTy>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        self.env.push_scope();
        let expected_block_ty = match expected {
            Some(t) => t,
            None => self.cg_ty_of(block.ty).unwrap_or(CgTy::Unit),
        };

        let mut value: CgValue<'ctx> = CgValue::unit();
        for (idx, stmt) in block.stmts.iter().enumerate() {
            let is_last = idx + 1 == block.stmts.len();
            match &stmt.kind {
                hir::StmtKind::Empty => {}
                hir::StmtKind::Val(decl) => {
                    self.codegen_val_decl(decl)?;
                    value = CgValue::unit();
                }
                hir::StmtKind::Assign { lhs, eq_span, rhs } => {
                    self.codegen_assign_stmt(*eq_span, lhs, rhs)?;
                    value = CgValue::unit();
                }
                hir::StmtKind::Expr(expr) => {
                    let expected = if is_last {
                        Some(expected_block_ty)
                    } else {
                        Some(CgTy::Unit)
                    };
                    let v = self.codegen_expr_in_expected_context(expr, expected)?;
                    value = if is_last { v } else { CgValue::unit() };
                }
                hir::StmtKind::While { cond, body } => {
                    self.codegen_while_stmt(stmt.span, cond, body)?;
                    value = CgValue::unit();
                }
                // block 作为表达式时，`return` 语义在当前阶段暂不支持（需要 function-level CFG）。
                hir::StmtKind::Return { .. } => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "`return` inside block expression",
                        at: stmt.span.into(),
                    });
                }
                hir::StmtKind::Break { .. }
                | hir::StmtKind::Continue { .. }
                | hir::StmtKind::Todo(_) => {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "statement inside block expression",
                        at: stmt.span.into(),
                    });
                }
            }
        }

        self.env.pop_scope();
        Ok(value)
    }

    fn codegen_literal(
        &mut self,
        span: crate::span::Span,
        ty: TypeId,
        lit: &hir::LiteralKind,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match lit {
            hir::LiteralKind::Unit => Ok(CgValue::unit()),
            hir::LiteralKind::Bool(v) => Ok(CgValue::bool(
                self.context.bool_type().const_int(*v as u64, false),
            )),
            hir::LiteralKind::Int => {
                let Some(CgTy::Int(int_ty)) = self.cg_ty_of(ty) else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "int literal type",
                        at: span.into(),
                    });
                };
                let text = self.source.slice(span);
                let value = parse_int_literal_decimal(text);
                let value = mask_to_bits(value, int_ty.bits) as u64;
                Ok(CgValue::int(
                    self.int_type(int_ty).const_int(value, false),
                    int_ty,
                ))
            }
            hir::LiteralKind::String => self.codegen_string_literal(span),
        }
    }

    fn codegen_string_literal(
        &mut self,
        span: crate::span::Span,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let text = self.source.slice(span);

        let bytes = match parse_string_literal_bytes(text) {
            Ok(bytes) => bytes,
            Err(StringLiteralParseError::Interpolated) => {
                // 插值字符串（`f"..."`/`f"""..."""`）由后续任务 T0823 lowering 处理；
                // 当前阶段避免“把原始文本当作普通字符串”导致语义错误。
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "interpolated string literal",
                    at: span.into(),
                });
            }
            Err(StringLiteralParseError::Invalid | StringLiteralParseError::InvalidUtf8) => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "invalid string literal",
                    at: span.into(),
                });
            }
        };

        // 1) 分配一个 GC-managed `ScoopString` 对象：
        //    - LLVM 侧类型为 `ScoopString addrspace(1)*`
        //    - 分配通过 `scoop_alloc(sizeof(ScoopString))`
        let scoop_str_ty = self.llvm_scoop_string_type();
        let obj_size = self.target_data.get_store_size(&scoop_str_ty);
        let size_v = self.context.i64_type().const_int(obj_size, false);

        let rt_alloc = self.declare_runtime_alloc();
        let call = self
            .builder
            .build_call(rt_alloc, &[size_v.into()], "rt_alloc_string_lit")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(raw_ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc return type",
                at: span.into(),
            });
        };

        let str_ptr_ty = self.llvm_scoop_string_ptr_type();
        let str_ptr = self
            .builder
            .build_pointer_cast(raw_ptr, str_ptr_ty, "str_obj_ptr")?;

        // 2) 写入 `{ len, data }`（对象头由 runtime 初始化；type_desc 当前仍为 NULL）。
        let len_ptr = self
            .builder
            .build_struct_gep(scoop_str_ty, str_ptr, 1, "str_len_gep")?;
        let data_ptr = self
            .builder
            .build_struct_gep(scoop_str_ty, str_ptr, 2, "str_data_gep")?;

        let len = self.context.i64_type().const_int(bytes.len() as u64, false);
        let _ = self.builder.build_store(len_ptr, len)?;

        // 空串：保持 `data = NULL`（与 runtime 侧空串约定一致）。
        if bytes.is_empty() {
            let i8_ptr_ty = self.context.i8_type().ptr_type(AddressSpace::default());
            let _ = self
                .builder
                .build_store(data_ptr, i8_ptr_ty.const_null())?;
        } else {
            // 把字节序列落到一个只读全局常量：`[N x i8] @__scoop_str_data_*`
            let data_gv = self.get_or_create_global_bytes(span, &bytes);
            let i8_ptr_ty = self.context.i8_type().ptr_type(AddressSpace::default());
            let data_i8_ptr = self.builder.build_pointer_cast(
                data_gv.as_pointer_value(),
                i8_ptr_ty,
                "str_data_ptr",
            )?;
            let _ = self.builder.build_store(data_ptr, data_i8_ptr)?;
        }

        Ok(CgValue {
            ty: CgTy::String,
            value: Some(str_ptr.into()),
        })
    }

    fn codegen_interpolated_string(
        &mut self,
        span: crate::span::Span,
        raw: bool,
        parts: &[hir::InterpolatedStringPart],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        // 当前阶段的落点：把 f-string 分片后“拼接”为一段连续 UTF-8 字节序列，
        // 返回一个 GC-managed `ScoopString` 对象（addrspace(1)），其 `data` 指向 `malloc` 的 bytes buffer。
        //
        // 约束（与 TODO T0823 对齐）：
        // - 仅支持 `{Int}` 与 `{String}`；
        // - 先不支持 format spec / locale；
        // - 当前阶段不接入 type descriptor/release：`data` 的释放留给后续任务补齐（T1507/T1514）。

        #[derive(Clone, Copy)]
        struct Segment<'ctx> {
            ptr: PointerValue<'ctx>,
            len: IntValue<'ctx>,
        }

        let i64_ty = self.context.i64_type();
        let i8_ty = self.context.i8_type();
        let i8_ptr_ty = i8_ty.ptr_type(AddressSpace::default());
        let scoop_str_ty = self.llvm_scoop_string_type();

        // 1) 先做一遍：收集所有片段的 (ptr, len)，并计算总长度（运行期）。
        let mut segments: Vec<Segment<'ctx>> = Vec::new();
        let mut total_len = i64_ty.const_zero();

        for part in parts {
            match part {
                hir::InterpolatedStringPart::Text { span: text_span } => {
                    let text = self.source.slice(*text_span);
                    let bytes = parse_f_string_text_bytes(raw, text).map_err(|_| {
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "invalid interpolated string text",
                            at: (*text_span).into(),
                        }
                    })?;

                    let gv = self.get_or_create_global_bytes(*text_span, &bytes);
                    let ptr = self.builder.build_pointer_cast(
                        gv.as_pointer_value(),
                        i8_ptr_ty,
                        "fstr_text_ptr",
                    )?;
                    let len = i64_ty.const_int(bytes.len() as u64, false);

                    segments.push(Segment { ptr, len });
                    total_len = self
                        .builder
                        .build_int_add(total_len, len, "fstr_total_len")?;
                }
                hir::InterpolatedStringPart::Expr { expr } => {
                    let v = self.codegen_expr(expr)?;

                    match v.ty {
                        CgTy::String => {
                            let coerced = self.coerce_value(expr.span, v, CgTy::String)?;
                            let Some(raw) = coerced.value else {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "string interpolation expr value",
                                    at: expr.span.into(),
                                });
                            };
                            let BasicValueEnum::PointerValue(str_obj_ptr) = raw else {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "string interpolation expr type",
                                    at: expr.span.into(),
                                });
                            };

                            let len_ptr = self.builder.build_struct_gep(
                                scoop_str_ty,
                                str_obj_ptr,
                                1,
                                "fstr_part_len_gep",
                            )?;
                            let data_ptr = self.builder.build_struct_gep(
                                scoop_str_ty,
                                str_obj_ptr,
                                2,
                                "fstr_part_data_gep",
                            )?;

                            let len = self
                                .builder
                                .build_load(i64_ty, len_ptr, "fstr_part_len")?
                                .into_int_value();
                            let data = self
                                .builder
                                .build_load(i8_ptr_ty, data_ptr, "fstr_part_data")?
                                .into_pointer_value();

                            segments.push(Segment { ptr: data, len });
                            total_len =
                                self.builder
                                    .build_int_add(total_len, len, "fstr_total_len")?;
                        }
                        CgTy::Int(from_ty) => {
                            if from_ty.bits > 64 {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "integer width for string interpolation",
                                    at: expr.span.into(),
                                });
                            }

                            let (raw_int, _) =
                                v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                                    kind: "integer interpolation expr value",
                                    at: expr.span.into(),
                                })?;

                            // 先把整数提升/截断到 i64/u64，再调用 runtime 格式化到临时 buffer。
                            let to_ty = IntTy {
                                bits: 64,
                                signed: from_ty.signed,
                            };
                            let int64 = self.cast_int(raw_int, from_ty, to_ty)?;

                            // i64 最长：`-9223372036854775808`（20 字符）；
                            // 这里给更宽松的 cap，避免后续扩展时踩坑。
                            let cap = i64_ty.const_int(64, false);
                            let buf =
                                self.builder
                                    .build_array_alloca(i8_ty, cap, "fstr_int_buf")?;

                            let fmt_name = if from_ty.signed {
                                "scoop_format_i64"
                            } else {
                                "scoop_format_u64"
                            };
                            let fmt_fun = self.declare_runtime_format_int(fmt_name);
                            let call_site = self.builder.build_call(
                                fmt_fun,
                                &[int64.into(), buf.into(), cap.into()],
                                "fstr_fmt_int",
                            )?;
                            let len = call_site
                                .try_as_basic_value()
                                .basic()
                                .ok_or(LlvmEmitError::UnsupportedMainBody {
                                    kind: "string interpolation int length",
                                    at: expr.span.into(),
                                })?
                                .into_int_value();

                            segments.push(Segment { ptr: buf, len });
                            total_len =
                                self.builder
                                    .build_int_add(total_len, len, "fstr_total_len")?;
                        }
                        _ => {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "string interpolation expr type",
                                at: expr.span.into(),
                            });
                        }
                    }
                }
            }
        }

        // 2) 为拼接结果分配 heap buffer（malloc），并按顺序 memcpy 各段。
        let is_zero = self.builder.build_int_compare(
            IntPredicate::EQ,
            total_len,
            i64_ty.const_zero(),
            "fstr_total_is_zero",
        )?;

        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: span.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: span.into(),
            })?;

        let malloc_bb = self.context.append_basic_block(func, "fstr_malloc");
        let done_bb = self.context.append_basic_block(func, "fstr_done");

        self.builder
            .build_conditional_branch(is_zero, done_bb, malloc_bb)?;

        // --- malloc + memcpy ---
        self.builder.position_at_end(malloc_bb);
        let malloc = self.declare_libc_malloc();
        let call = self
            .builder
            .build_call(malloc, &[total_len.into()], "fstr_malloc")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "malloc return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(buf) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "malloc return type",
                at: span.into(),
            });
        };

        let mut cursor = i64_ty.const_zero();
        for (idx, seg) in segments.iter().enumerate() {
            let dst = unsafe {
                self.builder.build_in_bounds_gep(
                    i8_ty,
                    buf,
                    &[cursor],
                    &format!("fstr_dst_{idx}"),
                )?
            };
            let _ = self.builder.build_memcpy(dst, 1, seg.ptr, 1, seg.len)?;
            cursor = self.builder.build_int_add(cursor, seg.len, "fstr_cursor")?;
        }

        self.builder.build_unconditional_branch(done_bb)?;

        // --- done ---
        self.builder.position_at_end(done_bb);
        let buf_phi = self.builder.build_phi(i8_ptr_ty, "fstr_buf")?;
        let buf_null: BasicValueEnum<'ctx> = i8_ptr_ty.const_null().into();
        let buf_value: BasicValueEnum<'ctx> = buf.into();
        buf_phi.add_incoming(&[(&buf_null, insert_block), (&buf_value, malloc_bb)]);
        let buf_ptr = buf_phi.as_basic_value().into_pointer_value();

        // 3) 分配并初始化 `ScoopString` 对象（GC-managed）。
        let obj_size = self.target_data.get_store_size(&scoop_str_ty);
        let size_v = i64_ty.const_int(obj_size, false);

        let rt_alloc = self.declare_runtime_alloc();
        let call = self
            .builder
            .build_call(rt_alloc, &[size_v.into()], "rt_alloc_fstr")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(raw_ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc return type",
                at: span.into(),
            });
        };

        let str_ptr_ty = self.llvm_scoop_string_ptr_type();
        let str_ptr = self
            .builder
            .build_pointer_cast(raw_ptr, str_ptr_ty, "fstr_obj_ptr")?;

        let len_ptr = self
            .builder
            .build_struct_gep(scoop_str_ty, str_ptr, 1, "fstr_len_gep")?;
        let data_ptr = self
            .builder
            .build_struct_gep(scoop_str_ty, str_ptr, 2, "fstr_data_gep")?;

        let _ = self.builder.build_store(len_ptr, total_len)?;
        let _ = self.builder.build_store(data_ptr, buf_ptr)?;

        Ok(CgValue {
            ty: CgTy::String,
            value: Some(str_ptr.into()),
        })
    }

    fn codegen_var_ref(
        &mut self,
        span: crate::span::Span,
        v: &hir::ValueRef,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match v {
            hir::ValueRef::TopLevel { fqn, .. } => {
                // T1311：object/companion object 单例值在表达式位置可用：
                // - 读取单例值应触发一次初始化（init block / 属性 init）；
                // - 运行期用一个 module-local 的唯一地址作为“单例实例指针”（ref type）。
                if self.object_inits.contains_key(fqn) {
                    return self.codegen_object_value_access(span, fqn);
                }

                // T1023：`@ThreadLocal/@Global var` 顶层可变变量。
                let Some(var) = self.top_level_vars.get(fqn) else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "top-level value ref",
                        at: span.into(),
                    });
                };

                let cg_ty = self
                    .cg_ty_of(var.ty)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "top-level var type",
                        at: var.span.into(),
                    })?;

                if cg_ty == CgTy::Unit {
                    return Ok(CgValue::unit());
                }

                let gv = self.declare_top_level_var_global(var)?;
                let llvm_ty = self.llvm_basic_type_of(span, cg_ty)?;
                let loaded = self.builder.build_load(
                    llvm_ty,
                    gv.as_pointer_value(),
                    "load_top_level_var",
                )?;

                Ok(match cg_ty {
                    CgTy::Bool => CgValue::bool(loaded.into_int_value()),
                    CgTy::Int(int_ty) => CgValue::int(loaded.into_int_value(), int_ty),
                    CgTy::String => CgValue {
                        ty: CgTy::String,
                        value: Some(loaded.into_pointer_value().into()),
                    },
                    CgTy::Ref => CgValue {
                        ty: CgTy::Ref,
                        value: Some(loaded.into_pointer_value().into()),
                    },
                    CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) => CgValue {
                        ty: cg_ty,
                        value: Some(loaded),
                    },
                    CgTy::Unit => CgValue::unit(),
                })
            }
            hir::ValueRef::Local { id, .. } => {
                let local = self
                    .env
                    .get(*id)
                    .ok_or_else(|| {
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "unknown local value",
                            at: span.into(),
                        }
                    })?;

                match local.ty {
                    CgTy::Unit => Ok(CgValue::unit()),
                    CgTy::Bool => {
                        let raw = self
                            .builder
                            .build_load(
                                self.llvm_basic_type_of(span, local.ty)?,
                                local.ptr,
                                "load_bool",
                            )?
                            .into_int_value();
                        Ok(CgValue::bool(raw))
                    }
                    CgTy::Int(int_ty) => {
                        let raw = self
                            .builder
                            .build_load(
                                self.llvm_basic_type_of(span, local.ty)?,
                                local.ptr,
                                "load_int",
                            )?
                            .into_int_value();
                        Ok(CgValue::int(raw, int_ty))
                    }
                    CgTy::String => {
                        let raw = self
                            .builder
                            .build_load(
                                self.llvm_basic_type_of(span, local.ty)?,
                                local.ptr,
                                "load_str",
                            )?
                            .into_pointer_value();
                        Ok(CgValue {
                            ty: CgTy::String,
                            value: Some(raw.into()),
                        })
                    }
                    CgTy::Ref => {
                        let raw = self
                            .builder
                            .build_load(
                                self.llvm_basic_type_of(span, local.ty)?,
                                local.ptr,
                                "load_ref",
                            )?
                            .into_pointer_value();
                        Ok(CgValue {
                            ty: CgTy::Ref,
                            value: Some(raw.into()),
                        })
                    }
                    CgTy::Tuple(_) => {
                        let raw = self.builder.build_load(
                            self.llvm_basic_type_of(span, local.ty)?,
                            local.ptr,
                            "load_tuple",
                        )?;
                        Ok(CgValue {
                            ty: local.ty,
                            value: Some(raw),
                        })
                    }
                    CgTy::Struct(_) => {
                        let raw = self.builder.build_load(
                            self.llvm_basic_type_of(span, local.ty)?,
                            local.ptr,
                            "load_struct",
                        )?;
                        Ok(CgValue {
                            ty: local.ty,
                            value: Some(raw),
                        })
                    }
                    CgTy::Enum(_) => {
                        let raw = self.builder.build_load(
                            self.llvm_basic_type_of(span, local.ty)?,
                            local.ptr,
                            "load_enum",
                        )?;
                        Ok(CgValue {
                            ty: local.ty,
                            value: Some(raw),
                        })
                    }
                }
            }
        }
    }

    fn codegen_struct_lit(
        &mut self,
        span: crate::span::Span,
        ty: TypeId,
        fields: &[hir::StructLitField],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let Some(CgTy::Struct(struct_ty)) = self.cg_ty_of(ty) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "struct literal type",
                at: span.into(),
            });
        };

        let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = self.types.kind(struct_ty) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "struct literal type",
                at: span.into(),
            });
        };

        let layout =
            self.struct_layouts
                .get(&nominal.fqn)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "struct literal layout",
                    at: span.into(),
                })?;

        let llvm_struct_ty = self.llvm_struct_type(span, struct_ty)?;
        let mut agg: AggregateValueEnum<'ctx> = llvm_struct_ty.get_undef().into();

        for (idx, field) in layout.fields.iter().enumerate() {
            let Some(init) = fields.iter().find(|f| f.name == field.name) else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "struct literal missing field",
                    at: span.into(),
                });
            };

            let field_cg = self.cg_ty_of_type_fqn(init.span, field.ty_fqn.as_deref())?;

            let init_v = self.codegen_expr(&init.value)?;
            let coerced = self.coerce_value(init.value.span, init_v, field_cg)?;

            let raw = match field_cg {
                CgTy::Unit => self.context.i8_type().const_int(0, false).into(),
                _ => coerced.value.ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "struct field value",
                    at: init.value.span.into(),
                })?,
            };

            let name = format!("insert_{}", field.name);
            agg = self
                .builder
                .build_insert_value(agg, raw, idx as u32, &name)?;
        }

        Ok(CgValue {
            ty: CgTy::Struct(struct_ty),
            value: Some(agg.as_basic_value_enum()),
        })
    }

    fn codegen_tuple_lit(
        &mut self,
        span: crate::span::Span,
        ty: TypeId,
        elements: &[hir::Expr],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let Some(CgTy::Tuple(tuple_ty)) = self.cg_ty_of(ty) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "tuple literal type",
                at: span.into(),
            });
        };

        let TypeKind::Value(ValueTypeKind::Tuple(element_tys)) = self.types.kind(tuple_ty) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "tuple literal type",
                at: span.into(),
            });
        };

        if element_tys.len() != elements.len() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "tuple literal arity mismatch",
                at: span.into(),
            });
        }

        let llvm_tuple_ty = self.llvm_tuple_type(span, tuple_ty)?;
        let mut agg: AggregateValueEnum<'ctx> = llvm_tuple_ty.get_undef().into();

        for (idx, (elem_expr, elem_ty)) in elements.iter().zip(element_tys.iter()).enumerate() {
            let elem_cg = self
                .cg_ty_of(*elem_ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "tuple element type",
                    at: elem_expr.span.into(),
                })?;

            let elem_v = self.codegen_expr(elem_expr)?;
            let coerced = self.coerce_value(elem_expr.span, elem_v, elem_cg)?;

            let raw: BasicValueEnum<'ctx> = match elem_cg {
                CgTy::Unit => self.context.i8_type().const_int(0, false).into(),
                _ => coerced.value.ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "tuple element value",
                    at: elem_expr.span.into(),
                })?,
            };

            let name = format!("insert_elem_{idx}");
            agg = self
                .builder
                .build_insert_value(agg, raw, idx as u32, &name)?;
        }

        Ok(CgValue {
            ty: CgTy::Tuple(tuple_ty),
            value: Some(agg.as_basic_value_enum()),
        })
    }

    fn codegen_member_access(
        &mut self,
        _span: crate::span::Span,
        receiver: &hir::Expr,
        member: &hir::MemberAccess,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match member.resolved.as_ref() {
            Some(hir::MemberRef::Value { fqn, .. }) => {
                // T1311：`TypeName.NestedObject` / `Obj.NestedObject` 的“object 值”访问。
                if self.object_inits.contains_key(fqn) {
                    return self.codegen_object_value_access(member.span, fqn);
                }

                // T0828：`object` / `companion object` 静态成员访问（backing field 读取）。
                if self.lookup_object_property_by_fqn(fqn).is_some() {
                    return self.codegen_object_property_access(member.span, fqn);
                }

                // `EnumName.Variant`（unit variant）：`RuntimeError.NullAssertionFailed` 等。
                if let Some(v) =
                    self.try_codegen_qualified_enum_unit_variant_value(member.span, fqn)?
                {
                    return Ok(v);
                }

                // T1312：class 实例字段访问（`this.x` / `obj.x`）。
                if let Some((class, field_idx, field_cg)) =
                    self.lookup_class_field_by_fqn(fqn, member.span)?
                {
                    if field_cg == CgTy::Unit {
                        return Ok(CgValue::unit());
                    }

                    let recv = self.codegen_expr_in_expected_context(receiver, Some(CgTy::Ref))?;
                    let recv = self.coerce_value(receiver.span, recv, CgTy::Ref)?;
                    let Some(raw) = recv.value else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "class field receiver value",
                            at: receiver.span.into(),
                        });
                    };
                    let BasicValueEnum::PointerValue(obj_ptr) = raw else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "class field receiver type",
                            at: receiver.span.into(),
                        });
                    };

                    let field_ptr =
                        self.codegen_class_field_ptr(member.span, &class, obj_ptr, field_idx)?;
                    let llvm_ty = self.llvm_basic_type_of(member.span, field_cg)?;
                    let loaded = self
                        .builder
                        .build_load(llvm_ty, field_ptr, "load_class_field")?;
                    return self.cg_value_from_loaded(member.span, field_cg, loaded);
                }

                // 优先路径：`localStruct.field` —— 用 GEP 从 alloca slot 取字段（更贴近后续可变字段语义）。
                if let hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) = &receiver.kind {
                    if let Some(local) = self.env.get(*id) {
                        if let CgTy::Struct(struct_ty) = local.ty {
                            let (field_idx, field_ty) =
                                self.lookup_struct_field(struct_ty, fqn, member.span)?;
                            if field_ty == CgTy::Unit {
                                return Ok(CgValue::unit());
                            }

                            let llvm_struct_ty = self.llvm_struct_type(member.span, struct_ty)?;
                            let field_ptr = self.builder.build_struct_gep(
                                llvm_struct_ty,
                                local.ptr,
                                field_idx,
                                "field_gep",
                            )?;
                            let llvm_field_ty = self.llvm_basic_type_of(member.span, field_ty)?;
                            let loaded =
                                self.builder
                                    .build_load(llvm_field_ty, field_ptr, "load_field")?;
                            // `@CLayout(packed = 1)`：字段地址可能是未对齐的，因此必须把 load 的 alignment
                            // 降到 1，避免 LLVM 以 ABI 对齐假设做错误优化（UB）。
                            if self
                                .struct_clayout(struct_ty)
                                .and_then(|c| c.packed)
                                .is_some()
                            {
                                if let Some(inst) = loaded.as_instruction_value() {
                                    inst.set_alignment(1)?;
                                }
                            }
                            return self.cg_value_from_loaded(member.span, field_ty, loaded);
                        }
                    }
                }

                // fallback：先把 receiver 降到值，再用 extractvalue 取字段。
                let recv = self.codegen_expr(receiver)?;
                let CgTy::Struct(struct_ty) = recv.ty else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "member access receiver type",
                        at: receiver.span.into(),
                    });
                };
                let (field_idx, field_ty) =
                    self.lookup_struct_field(struct_ty, fqn, member.span)?;
                if field_ty == CgTy::Unit {
                    return Ok(CgValue::unit());
                }

                let raw = recv.value.ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "member access receiver value",
                    at: receiver.span.into(),
                })?;
                let struct_v = raw.into_struct_value();
                let extracted =
                    self.builder
                        .build_extract_value(struct_v, field_idx, "extract_field")?;
                return self.cg_value_from_loaded(member.span, field_ty, extracted);
            }
            Some(_) => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "member access target",
                    at: member.span.into(),
                });
            }
            None => {}
        }

        // tuple 元素访问（spec §2.3.3）：`t._0` / `t._1` / ...
        let Some(elem_idx) = parse_tuple_member_index(&member.name) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "member access target",
                at: member.span.into(),
            });
        };

        // 优先路径：`localTuple._0` —— 用 GEP 从 alloca slot 取元素。
        if let hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) = &receiver.kind {
            if let Some(local) = self.env.get(*id) {
                if let CgTy::Tuple(tuple_ty) = local.ty {
                    let elem_ty = self.lookup_tuple_element(tuple_ty, elem_idx, member.span)?;
                    if elem_ty == CgTy::Unit {
                        return Ok(CgValue::unit());
                    }

                    let llvm_tuple_ty = self.llvm_tuple_type(member.span, tuple_ty)?;
                    let elem_ptr = self.builder.build_struct_gep(
                        llvm_tuple_ty,
                        local.ptr,
                        elem_idx,
                        "tuple_elem_gep",
                    )?;
                    let llvm_elem_ty = self.llvm_basic_type_of(member.span, elem_ty)?;
                    let loaded =
                        self.builder
                            .build_load(llvm_elem_ty, elem_ptr, "load_tuple_elem")?;
                    return self.cg_value_from_loaded(member.span, elem_ty, loaded);
                }
            }
        }

        // fallback：先把 receiver 降到值，再用 extractvalue 取元素。
        let recv = self.codegen_expr(receiver)?;
        let CgTy::Tuple(tuple_ty) = recv.ty else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "member access receiver type",
                at: receiver.span.into(),
            });
        };

        let elem_ty = self.lookup_tuple_element(tuple_ty, elem_idx, member.span)?;
        if elem_ty == CgTy::Unit {
            return Ok(CgValue::unit());
        }

        let raw = recv.value.ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "member access receiver value",
            at: receiver.span.into(),
        })?;
        let tuple_v = raw.into_struct_value();
        let extracted =
            self.builder
                .build_extract_value(tuple_v, elem_idx, "extract_tuple_elem")?;
        self.cg_value_from_loaded(member.span, elem_ty, extracted)
    }

    /// 将一个“限定名 enum unit variant 值”（例如 `RuntimeError.NullAssertionFailed`）降低为 enum 常量。
    ///
    /// 说明：
    /// - parser 会把 `EnumName.Variant` 表示为 member access；
    /// - resolver 会将 `Variant` 解析为一个 value FQN（`EnumFqn.Variant`）；
    /// - 对于 0-arity（unit）variant，我们在 codegen 侧直接构造 `{ tag, payload }` 值。
    fn try_codegen_qualified_enum_unit_variant_value(
        &mut self,
        at: crate::span::Span,
        value_fqn: &str,
    ) -> Result<Option<CgValue<'ctx>>, LlvmEmitError> {
        let Some((owner_fqn, variant_name)) = value_fqn.rsplit_once('.') else {
            return Ok(None);
        };
        let Some(enum_layout) = self.enum_layouts.get(owner_fqn) else {
            return Ok(None);
        };
        let Some(variant) = enum_layout.variants.iter().find(|v| v.name == variant_name) else {
            return Ok(None);
        };

        let tag = variant.tag;
        let field_count = variant.fields.len();
        if field_count != 0 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "enum variant with payload used as value",
                at: at.into(),
            });
        }

        let enum_ty = self
            .types
            .iter_ids()
            .find(|id| {
                matches!(
                    self.types.kind(*id),
                    TypeKind::Value(ValueTypeKind::Nominal(nominal))
                        if nominal.fqn == owner_fqn && nominal.args.is_empty() && nominal.eff.is_none()
                )
            })
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "enum type id for qualified variant value",
                at: at.into(),
            })?;

        let v = self.build_enum_value(at, enum_ty, tag, None)?;
        Ok(Some(v))
    }

    fn lookup_object_property_by_fqn(
        &self,
        prop_fqn: &str,
    ) -> Option<(&hir::ObjectInit, &hir::ObjectProperty)> {
        let (owner, name) = prop_fqn.rsplit_once('.')?;
        let obj = self.object_inits.get(owner)?;
        let prop = obj.properties.get(name)?;
        Some((obj, prop))
    }

    /// 若 `field_fqn` 指向一个 class 的实例字段，则返回该字段的布局/类型信息。
    ///
    /// 返回值：
    /// - `class`：对应 class 的初始化信息（字段列表/初始化步骤）
    /// - `field_idx`：字段在 payload struct 中的稳定索引
    /// - `field_cg`：字段的 codegen 类型（用于 load/store）
    fn lookup_class_field_by_fqn(
        &mut self,
        field_fqn: &str,
        at: crate::span::Span,
    ) -> Result<Option<(hir::ClassInit, u32, CgTy)>, LlvmEmitError> {
        let Some((owner_fqn, _name)) = field_fqn.rsplit_once('.') else {
            return Ok(None);
        };
        if !self.class_inits.contains_key(owner_fqn) {
            return Ok(None);
        }
        let class = self.class_init_layout(at, owner_fqn)?;
        let Some(field_idx) = class.field_indices.get(field_fqn).copied() else {
            return Ok(None);
        };
        let field =
            class
                .fields
                .get(field_idx as usize)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "class field index",
                    at: at.into(),
                })?;
        let field_cg = self
            .cg_ty_of(field.ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "class field type",
                at: at.into(),
            })?;
        Ok(Some((class, field_idx, field_cg)))
    }

    fn codegen_object_property_access(
        &mut self,
        at: crate::span::Span,
        prop_fqn: &str,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let (object_fqn, prop) = match self.lookup_object_property_by_fqn(prop_fqn) {
            Some((obj, prop)) => (obj.fqn.clone(), prop.clone()),
            None => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "object property access (missing metadata)",
                    at: at.into(),
                });
            }
        };

        if !prop.has_init {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "object property without initializer",
                at: at.into(),
            });
        }

        let prop_cg = self
            .cg_ty_of(prop.ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "object property type",
                at: at.into(),
            })?;

        let init_fn = self.ensure_object_init_function_defined(&object_fqn)?;
        let _ = self.builder.build_call(init_fn, &[], "obj_init")?;

        if prop_cg == CgTy::Unit {
            return Ok(CgValue::unit());
        }

        let Some(global) = self.declare_object_property_global(at, prop_fqn, prop_cg)? else {
            return Ok(CgValue::unit());
        };
        let llvm_ty = self.llvm_basic_type_of(at, prop_cg)?;
        let loaded =
            self.builder
                .build_load(llvm_ty, global.as_pointer_value(), "load_obj_prop")?;
        self.cg_value_from_loaded(at, prop_cg, loaded)
    }

    fn ensure_object_init_function_defined(
        &mut self,
        object_fqn: &str,
    ) -> Result<FunctionValue<'ctx>, LlvmEmitError> {
        let Some(obj) = self.object_inits.get(object_fqn) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "object init (missing metadata)",
                at: crate::span::Span::new(0, 0).into(),
            });
        };

        let name = object_init_fn_name(object_fqn);
        let fn_ty = self.context.void_type().fn_type(&[], false);

        let llvm_fun = self
            .module
            .get_function(&name)
            .unwrap_or_else(|| self.module.add_function(&name, fn_ty, None));

        // 已有 body：无需重复生成。
        if llvm_fun.get_first_basic_block().is_some() {
            return Ok(llvm_fun);
        }

        // 在生成 init function body 时，临时切换 builder 的插入点；结束后恢复到调用方位置。
        let saved_block = self.builder.get_insert_block();

        let mut init_codegen = MainCodegen::new(
            self.context,
            self.module,
            self.builder,
            self.target_data,
            self.host,
            self.source,
            self.types,
            self.struct_layouts,
            self.enum_layouts,
            self.top_level_vars,
            self.object_inits,
            self.class_inits,
            self.ctor_call_sites,
            self.extern_funs,
            self.fun_index,
        );
        init_codegen.codegen_object_init_fun_body(obj, llvm_fun)?;

        if let Some(bb) = saved_block {
            self.builder.position_at_end(bb);
        }

        Ok(llvm_fun)
    }

    fn codegen_object_init_fun_body(
        &mut self,
        obj: &hir::ObjectInit,
        llvm_fun: FunctionValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        let err_span = obj
            .steps
            .first()
            .map(|step| match step {
                hir::ObjectInitStep::PropertyInit { init, .. } => init.span,
                hir::ObjectInitStep::InitBlock { block } => block.span,
            })
            .unwrap_or(crate::span::Span::new(0, 0));

        let entry = self.context.append_basic_block(llvm_fun, "entry");
        let init_bb = self.context.append_basic_block(llvm_fun, "init");
        let done_bb = self.context.append_basic_block(llvm_fun, "done");

        self.builder.position_at_end(entry);
        // object init 是一个内部 `void` 函数：为 flag-based unwinding（Raise.raise）提供返回类型上下文。
        self.current_fun_return_ty = Some(CgTy::Unit);

        let guard = self.declare_object_init_guard(&obj.fqn);
        let once_begin = self.declare_runtime_once_begin();
        let call = self.builder.build_call(
            once_begin,
            &[guard.as_pointer_value().into()],
            "once_begin",
        )?;
        let ret = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "object init once begin return value",
                at: err_span.into(),
            })?;
        let BasicValueEnum::IntValue(should_init) = ret else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "object init once begin return type",
                at: err_span.into(),
            });
        };
        let i32_ty = self.context.i32_type();
        let cond = self.builder.build_int_compare(
            IntPredicate::NE,
            should_init,
            i32_ty.const_int(0, false),
            "should_init",
        )?;
        self.builder
            .build_conditional_branch(cond, init_bb, done_bb)?;

        self.builder.position_at_end(init_bb);

        self.env.push_scope();
        for step in &obj.steps {
            match step {
                hir::ObjectInitStep::PropertyInit { name, init } => {
                    let Some(prop) = obj.properties.get(name) else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "object property init (missing property)",
                            at: init.span.into(),
                        });
                    };

                    let prop_cg =
                        self.cg_ty_of(prop.ty)
                            .ok_or(LlvmEmitError::UnsupportedMainBody {
                                kind: "object property init type",
                                at: init.span.into(),
                            })?;

                    let v = self.codegen_expr_in_expected_context(init, Some(prop_cg))?;

                    // Unit：只执行副作用即可，无需 backing storage。
                    if prop_cg != CgTy::Unit {
                        let prop_fqn = format!("{}.{}", obj.fqn, name);
                        let Some(global) =
                            self.declare_object_property_global(init.span, &prop_fqn, prop_cg)?
                        else {
                            continue;
                        };
                        let _ = self.store_local_value(
                            init.span,
                            global.as_pointer_value(),
                            prop_cg,
                            v,
                        )?;
                    }
                }
                hir::ObjectInitStep::InitBlock { block } => {
                    let _ = self.codegen_block_value(block)?;
                }
            }
        }
        self.env.pop_scope();

        let once_end = self.declare_runtime_once_end();
        let _ =
            self.builder
                .build_call(once_end, &[guard.as_pointer_value().into()], "once_end")?;
        self.builder.build_unconditional_branch(done_bb)?;

        self.builder.position_at_end(done_bb);
        self.builder.build_return(None)?;
        Ok(())
    }

    fn declare_object_init_guard(&self, object_fqn: &str) -> GlobalValue<'ctx> {
        let name = object_guard_global_name(object_fqn);
        if let Some(existing) = self.module.get_global(&name) {
            return existing;
        }

        // 说明：
        // - 该 guard 由 runtime 的 `scoop_once_begin/end` 维护（TODO T0918）；
        // - 布局约定：单个 `uint64_t` word（低 2 bit 状态 + 其余 bit 为 owner thread id）。
        let gv = self.module.add_global(self.context.i64_type(), None, &name);
        gv.set_linkage(Linkage::Internal);
        gv.set_initializer(&self.context.i64_type().const_int(0, false));
        gv
    }

    fn declare_object_instance_global(&self, object_fqn: &str) -> GlobalValue<'ctx> {
        let name = object_instance_global_name(object_fqn);
        if let Some(existing) = self.module.get_global(&name) {
            return existing;
        }

        // 说明：
        // - 早期阶段我们用一个 module-local 的唯一地址充当 object 单例实例的“身份”（指针值）；
        // - 该地址不参与 GC，也不承载字段布局；静态属性仍单独走 `__scoop_object_prop__*` 全局存储。
        let gv = self.module.add_global(self.context.i8_type(), None, &name);
        gv.set_linkage(Linkage::Internal);
        gv.set_initializer(&self.context.i8_type().const_int(0, false));
        gv
    }

    fn codegen_object_value_access(
        &mut self,
        _at: crate::span::Span,
        object_fqn: &str,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let init_fn = self.ensure_object_init_function_defined(object_fqn)?;
        let _ = self.builder.build_call(init_fn, &[], "obj_init")?;

        let instance = self.declare_object_instance_global(object_fqn);
        Ok(CgValue {
            ty: CgTy::Ref,
            value: Some(instance.as_pointer_value().into()),
        })
    }

    fn declare_object_property_global(
        &mut self,
        at: crate::span::Span,
        prop_fqn: &str,
        prop_cg: CgTy,
    ) -> Result<Option<GlobalValue<'ctx>>, LlvmEmitError> {
        if prop_cg == CgTy::Unit {
            return Ok(None);
        }

        let name = object_prop_global_name(prop_fqn);
        if let Some(existing) = self.module.get_global(&name) {
            return Ok(Some(existing));
        }

        let llvm_ty = self.llvm_basic_type_of(at, prop_cg)?;
        let gv = self.module.add_global(llvm_ty, None, &name);
        gv.set_linkage(Linkage::Internal);

        let init: BasicValueEnum<'ctx> = match llvm_ty {
            BasicTypeEnum::IntType(ty) => BasicValueEnum::IntValue(ty.const_int(0, false)),
            BasicTypeEnum::PointerType(ty) => BasicValueEnum::PointerValue(ty.const_null()),
            BasicTypeEnum::StructType(ty) => BasicValueEnum::StructValue(ty.const_zero()),
            BasicTypeEnum::ArrayType(ty) => BasicValueEnum::ArrayValue(ty.const_zero()),
            BasicTypeEnum::FloatType(ty) => BasicValueEnum::FloatValue(ty.const_float(0.0)),
            BasicTypeEnum::VectorType(ty) => BasicValueEnum::VectorValue(ty.const_zero()),
            BasicTypeEnum::ScalableVectorType(ty) => {
                BasicValueEnum::ScalableVectorValue(ty.const_zero())
            }
        };
        gv.set_initializer(&init);
        Ok(Some(gv))
    }

    fn lookup_struct_field(
        &self,
        struct_ty: TypeId,
        field_fqn: &str,
        at: crate::span::Span,
    ) -> Result<(u32, CgTy), LlvmEmitError> {
        let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = self.types.kind(struct_ty) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "struct type id",
                at: at.into(),
            });
        };

        let layout =
            self.struct_layouts
                .get(&nominal.fqn)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "struct layout",
                    at: at.into(),
                })?;

        let idx = layout
            .fields
            .iter()
            .position(|f| f.fqn == field_fqn)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "unknown struct field",
                at: at.into(),
            })?;

        let field = &layout.fields[idx];
        let field_ty = self.cg_ty_of_type_fqn(field.span, field.ty_fqn.as_deref())?;
        Ok((idx as u32, field_ty))
    }

    fn struct_clayout(&self, struct_ty: TypeId) -> Option<hir::StructCLayout> {
        let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = self.types.kind(struct_ty) else {
            return None;
        };
        self.struct_layouts
            .get(&nominal.fqn)
            .and_then(|layout| layout.c_layout)
    }

    /// 计算 class 对象中某个字段的地址。
    ///
    /// 约定：
    /// - `obj_ptr` 指向对象头（即 runtime `scoop_alloc` 的返回值，`ScoopGcObjectHeader*` 起始地址）；
    /// - 对象布局在 LLVM 侧表示为 `{ ScoopGcObjectHeader, ClassPayload }`；
    /// - 字段位于 `ClassPayload` 内部，索引由 `hir::ClassInit.fields` 的稳定顺序决定。
    fn codegen_class_field_ptr(
        &mut self,
        at: crate::span::Span,
        class: &hir::ClassInit,
        obj_ptr: PointerValue<'ctx>,
        field_idx: u32,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        if field_idx as usize >= class.fields.len() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "class field index out of bounds",
                at: at.into(),
            });
        }

        let obj_ty = self.llvm_class_object_type(at, class)?;
        let obj_ptr_ty = obj_ty.ptr_type(self.gc_address_space());
        let typed_obj = self
            .builder
            .build_pointer_cast(obj_ptr, obj_ptr_ty, "class_obj_ptr")?;

        let payload_ptr =
            self.builder
                .build_struct_gep(obj_ty, typed_obj, 1, "class_payload_gep")?;

        let payload_ty = self.llvm_class_payload_type(at, class)?;
        let field_ptr =
            self.builder
                .build_struct_gep(payload_ty, payload_ptr, field_idx, "class_field_gep")?;
        Ok(field_ptr)
    }

    fn lookup_tuple_element(
        &self,
        tuple_ty: TypeId,
        elem_idx: u32,
        at: crate::span::Span,
    ) -> Result<CgTy, LlvmEmitError> {
        let TypeKind::Value(ValueTypeKind::Tuple(elements)) = self.types.kind(tuple_ty) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "tuple type id",
                at: at.into(),
            });
        };

        let elem_ty =
            elements
                .get(elem_idx as usize)
                .copied()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "tuple element out of bounds",
                    at: at.into(),
                })?;

        self.cg_ty_of(elem_ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "tuple element type",
                at: at.into(),
            })
    }

    fn cg_value_from_loaded(
        &self,
        _at: crate::span::Span,
        ty: CgTy,
        raw: BasicValueEnum<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        Ok(match ty {
            CgTy::Unit => CgValue::unit(),
            CgTy::Bool => CgValue::bool(raw.into_int_value()),
            CgTy::Int(int_ty) => CgValue::int(raw.into_int_value(), int_ty),
            CgTy::String => CgValue {
                ty: CgTy::String,
                value: Some(raw.into_pointer_value().into()),
            },
            CgTy::Ref => CgValue {
                ty: CgTy::Ref,
                value: Some(raw.into_pointer_value().into()),
            },
            CgTy::Tuple(tuple_ty) => CgValue {
                ty: CgTy::Tuple(tuple_ty),
                value: Some(raw),
            },
            CgTy::Struct(struct_ty) => CgValue {
                ty: CgTy::Struct(struct_ty),
                value: Some(raw),
            },
            CgTy::Enum(enum_ty) => CgValue {
                ty: CgTy::Enum(enum_ty),
                value: Some(raw),
            },
        })
    }

    fn codegen_unary(
        &mut self,
        span: crate::span::Span,
        op: ast::UnaryOp,
        expr: &hir::Expr,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match op {
            ast::UnaryOp::Not => {
                let v = self.codegen_expr(expr)?.as_bool().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "unary ! operand",
                        at: span.into(),
                    },
                )?;
                let out = self.builder.build_not(v, "not")?;
                Ok(CgValue::bool(out))
            }
            ast::UnaryOp::Neg => {
                let (v, ty) = self.codegen_expr(expr)?.as_int().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "unary - operand",
                        at: span.into(),
                    },
                )?;
                let out = self.builder.build_int_neg(v, "neg")?;
                Ok(CgValue::int(out, ty))
            }
            ast::UnaryOp::BitNot => {
                let (v, ty) = self.codegen_expr(expr)?.as_int().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "unary ~ operand",
                        at: span.into(),
                    },
                )?;
                let out = self.builder.build_not(v, "bitnot")?;
                Ok(CgValue::int(out, ty))
            }
        }
    }

    fn codegen_binary(
        &mut self,
        span: crate::span::Span,
        op: ast::BinaryOp,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match op {
            ast::BinaryOp::Add
            | ast::BinaryOp::Sub
            | ast::BinaryOp::Mul
            | ast::BinaryOp::Div
            | ast::BinaryOp::Rem
            | ast::BinaryOp::BitAnd
            | ast::BinaryOp::BitXor
            | ast::BinaryOp::BitOr => self.codegen_int_binary_same_type(span, op, lhs, rhs),

            ast::BinaryOp::Shl | ast::BinaryOp::Shr => self.codegen_shift(span, op, lhs, rhs),

            ast::BinaryOp::Lt | ast::BinaryOp::Le | ast::BinaryOp::Gt | ast::BinaryOp::Ge => {
                self.codegen_int_compare(span, op, lhs, rhs)
            }

            ast::BinaryOp::Eq | ast::BinaryOp::Ne => self.codegen_equality(span, op, lhs, rhs),

            ast::BinaryOp::LogAnd | ast::BinaryOp::LogOr => {
                self.codegen_bool_logic(span, op, lhs, rhs)
            }

            ast::BinaryOp::RangeInclusive => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "range operator",
                at: span.into(),
            }),

            ast::BinaryOp::Elvis => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "elvis operator",
                at: span.into(),
            }),
        }
    }

    fn codegen_int_binary_same_type(
        &mut self,
        span: crate::span::Span,
        op: ast::BinaryOp,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let lhs_is_lit = matches!(lhs.kind, hir::ExprKind::Literal(hir::LiteralKind::Int));
        let rhs_is_lit = matches!(rhs.kind, hir::ExprKind::Literal(hir::LiteralKind::Int));

        let (l_raw, l_ty) =
            self.codegen_expr(lhs)?
                .as_int()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "integer binary op lhs",
                    at: span.into(),
                })?;
        let (r_raw, r_ty) =
            self.codegen_expr(rhs)?
                .as_int()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "integer binary op rhs",
                    at: span.into(),
                })?;

        let out_ty = unify_int_types(lhs_is_lit, l_ty, rhs_is_lit, r_ty).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "integer binary op type",
                at: span.into(),
            },
        )?;

        let l = self.cast_int(l_raw, l_ty, out_ty)?;
        let r = self.cast_int(r_raw, r_ty, out_ty)?;

        let out = match op {
            ast::BinaryOp::Add => self.builder.build_int_add(l, r, "add")?,
            ast::BinaryOp::Sub => self.builder.build_int_sub(l, r, "sub")?,
            ast::BinaryOp::Mul => self.builder.build_int_mul(l, r, "mul")?,
            ast::BinaryOp::Div => {
                if out_ty.signed {
                    self.builder.build_int_signed_div(l, r, "sdiv")?
                } else {
                    self.builder.build_int_unsigned_div(l, r, "udiv")?
                }
            }
            ast::BinaryOp::Rem => {
                if out_ty.signed {
                    self.builder.build_int_signed_rem(l, r, "srem")?
                } else {
                    self.builder.build_int_unsigned_rem(l, r, "urem")?
                }
            }
            ast::BinaryOp::BitAnd => self.builder.build_and(l, r, "and")?,
            ast::BinaryOp::BitXor => self.builder.build_xor(l, r, "xor")?,
            ast::BinaryOp::BitOr => self.builder.build_or(l, r, "or")?,
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "integer binary op",
                    at: span.into(),
                });
            }
        };

        Ok(CgValue::int(out, out_ty))
    }

    fn codegen_shift(
        &mut self,
        span: crate::span::Span,
        op: ast::BinaryOp,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let (lhs_value, lhs_ty) =
            self.codegen_expr(lhs)?
                .as_int()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "shift lhs type",
                    at: span.into(),
                })?;

        let rhs_value =
            self.codegen_expr(rhs)?
                .as_int()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "shift rhs type",
                    at: span.into(),
                })?;

        let shift_count = self.mask_shift_count(lhs_ty, rhs_value.0)?;

        let out = match op {
            ast::BinaryOp::Shl => self
                .builder
                .build_left_shift(lhs_value, shift_count, "shl")?,
            ast::BinaryOp::Shr => {
                self.builder
                    .build_right_shift(lhs_value, shift_count, lhs_ty.signed, "shr")?
            }
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "shift operator",
                    at: span.into(),
                });
            }
        };

        Ok(CgValue::int(out, lhs_ty))
    }

    fn mask_shift_count(
        &mut self,
        lhs_ty: IntTy,
        rhs: IntValue<'ctx>,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let lhs_bits = lhs_ty.bits;
        let lhs_int = self.int_type(lhs_ty);

        // 1) 截断为 lhs 的位宽（只取低位，后续再 mask）。
        let rhs_trunc = self
            .builder
            .build_int_truncate(rhs, lhs_int, "shift_rhs_trunc")?;

        // 2) mask：shiftCount & (bitWidth - 1)，避免 LLVM 对“超范围 shift”的 UB。
        let mask = lhs_int.const_int((lhs_bits.saturating_sub(1)) as u64, false);
        Ok(self.builder.build_and(rhs_trunc, mask, "shift_masked")?)
    }

    fn codegen_int_compare(
        &mut self,
        span: crate::span::Span,
        op: ast::BinaryOp,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let lhs_is_lit = matches!(lhs.kind, hir::ExprKind::Literal(hir::LiteralKind::Int));
        let rhs_is_lit = matches!(rhs.kind, hir::ExprKind::Literal(hir::LiteralKind::Int));

        let (l_raw, l_ty) =
            self.codegen_expr(lhs)?
                .as_int()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "comparison lhs",
                    at: span.into(),
                })?;
        let (r_raw, r_ty) =
            self.codegen_expr(rhs)?
                .as_int()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "comparison rhs",
                    at: span.into(),
                })?;

        let int_ty = unify_int_types(lhs_is_lit, l_ty, rhs_is_lit, r_ty).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "comparison operand type",
                at: span.into(),
            },
        )?;

        let l = self.cast_int(l_raw, l_ty, int_ty)?;
        let r = self.cast_int(r_raw, r_ty, int_ty)?;

        let pred = match (op, int_ty.signed) {
            (ast::BinaryOp::Lt, true) => IntPredicate::SLT,
            (ast::BinaryOp::Lt, false) => IntPredicate::ULT,
            (ast::BinaryOp::Le, true) => IntPredicate::SLE,
            (ast::BinaryOp::Le, false) => IntPredicate::ULE,
            (ast::BinaryOp::Gt, true) => IntPredicate::SGT,
            (ast::BinaryOp::Gt, false) => IntPredicate::UGT,
            (ast::BinaryOp::Ge, true) => IntPredicate::SGE,
            (ast::BinaryOp::Ge, false) => IntPredicate::UGE,
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "comparison operator",
                    at: span.into(),
                });
            }
        };

        Ok(CgValue::bool(
            self.builder.build_int_compare(pred, l, r, "icmp")?,
        ))
    }

    fn codegen_equality(
        &mut self,
        span: crate::span::Span,
        op: ast::BinaryOp,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let lhs_is_lit = matches!(lhs.kind, hir::ExprKind::Literal(hir::LiteralKind::Int));
        let rhs_is_lit = matches!(rhs.kind, hir::ExprKind::Literal(hir::LiteralKind::Int));

        let lhs_v = self.codegen_expr(lhs)?;
        let rhs_v = self.codegen_expr(rhs)?;

        // Bool == Bool
        if matches!((lhs_v.ty, rhs_v.ty), (CgTy::Bool, CgTy::Bool)) {
            let l = lhs_v.as_bool().unwrap();
            let r = rhs_v.as_bool().unwrap();
            let pred = match op {
                ast::BinaryOp::Eq => IntPredicate::EQ,
                ast::BinaryOp::Ne => IntPredicate::NE,
                _ => unreachable!("filtered by caller"),
            };
            return Ok(CgValue::bool(self.builder.build_int_compare(
                pred,
                l,
                r,
                "icmp_bool",
            )?));
        }

        // Int == Int（含 int literal 吸收）
        let Some((l_raw, l_ty)) = lhs_v.as_int() else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "equality lhs",
                at: span.into(),
            });
        };
        let Some((r_raw, r_ty)) = rhs_v.as_int() else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "equality rhs",
                at: span.into(),
            });
        };

        let int_ty = unify_int_types(lhs_is_lit, l_ty, rhs_is_lit, r_ty).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "equality operand type",
                at: span.into(),
            },
        )?;

        let l = self.cast_int(l_raw, l_ty, int_ty)?;
        let r = self.cast_int(r_raw, r_ty, int_ty)?;

        let pred = match op {
            ast::BinaryOp::Eq => IntPredicate::EQ,
            ast::BinaryOp::Ne => IntPredicate::NE,
            _ => unreachable!("filtered by caller"),
        };
        Ok(CgValue::bool(
            self.builder.build_int_compare(pred, l, r, "icmp_eq")?,
        ))
    }

    fn codegen_bool_logic(
        &mut self,
        span: crate::span::Span,
        op: ast::BinaryOp,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let l = self
            .codegen_expr(lhs)?
            .as_bool()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "bool operator lhs",
                at: span.into(),
            })?;
        let r = self
            .codegen_expr(rhs)?
            .as_bool()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "bool operator rhs",
                at: span.into(),
            })?;

        let out = match op {
            ast::BinaryOp::LogAnd => self.builder.build_and(l, r, "and")?,
            ast::BinaryOp::LogOr => self.builder.build_or(l, r, "or")?,
            _ => unreachable!("filtered by caller"),
        };
        Ok(CgValue::bool(out))
    }

    fn coerce_value(
        &mut self,
        at: crate::span::Span,
        value: CgValue<'ctx>,
        target: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match (value.ty, target) {
            (CgTy::Unit, CgTy::Unit) => Ok(CgValue::unit()),
            (CgTy::Unit, CgTy::Ref) => {
                // early stage：允许把 `Unit` 装箱到 `Any`。
                //
                // 说明：
                // - 当前阶段有一部分“语句位置”的表达式仍会被类型系统视为 `Any`（例如某些 `block/when`），
                //   因此后端需要支持 `Unit -> Any` 的值提升；
                // - v0 阶段 runtime type descriptor 仍是占位（NULL），这里只保证“可执行/可回归”。
                let boxed = self.codegen_box_unit_to_ref(at)?;
                Ok(CgValue {
                    ty: CgTy::Ref,
                    value: Some(boxed.into()),
                })
            }
            (CgTy::Bool, CgTy::Bool) => Ok(value),
            (CgTy::Bool, CgTy::Int(int_ty)) => {
                let v = value.as_bool().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "bool value",
                    at: at.into(),
                })?;
                let out =
                    self.builder
                        .build_int_z_extend(v, self.int_type(int_ty), "bool_to_int")?;
                Ok(CgValue::int(out, int_ty))
            }
            (CgTy::Bool, CgTy::Ref) => {
                // early stage：允许把 `Bool` 装箱到 `Any`（与 `Int -> Any` 一致）。
                //
                // 注意：
                // - 当前阶段 runtime type descriptor 仍是占位（NULL），因此这里只保证“可执行/可回归”，
                //   不承诺后续 runtime type casts 的可观察语义；
                // - 为复用现有 box 形态，这里把 `Bool` 扩展为 word-sized 无符号整数后按 int box 存储。
                let v = value.as_bool().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "bool value",
                    at: at.into(),
                })?;
                let word = IntTy {
                    bits: self.host.word_bit_width(),
                    signed: false,
                };
                let widened = self
                    .builder
                    .build_int_z_extend(v, self.int_type(word), "box_bool_to_word")?;
                let boxed = self.codegen_box_int_to_ref(at, widened, word)?;
                Ok(CgValue {
                    ty: CgTy::Ref,
                    value: Some(boxed.into()),
                })
            }
            (CgTy::Int(from), CgTy::Int(to)) => {
                let (v, _) = value.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "int value",
                    at: at.into(),
                })?;
                let out = self.cast_int(v, from, to)?;
                Ok(CgValue::int(out, to))
            }
            (CgTy::String, CgTy::String) => Ok(value),
            (CgTy::String, CgTy::Ref) => {
                let Some(raw) = value.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "string -> ref coercion value",
                        at: at.into(),
                    });
                };
                let BasicValueEnum::PointerValue(ptr) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "string -> ref coercion type",
                        at: at.into(),
                    });
                };

                let casted =
                    self.builder
                        .build_pointer_cast(ptr, self.llvm_gc_i8_ptr_type(), "str_to_ref")?;
                Ok(CgValue {
                    ty: CgTy::Ref,
                    value: Some(casted.into()),
                })
            }
            (CgTy::Ref, CgTy::Ref) => Ok(value),
            (CgTy::Int(_), CgTy::Ref) => {
                // T0817：值类型装箱到 `Any`（当前阶段先只支持整数族）。
                let (raw_int, from_ty) =
                    value.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "int value",
                        at: at.into(),
                    })?;
                let boxed = self.codegen_box_int_to_ref(at, raw_int, from_ty)?;
                Ok(CgValue {
                    ty: CgTy::Ref,
                    value: Some(boxed.into()),
                })
            }
            (CgTy::Tuple(from), CgTy::Tuple(to)) if from == to => Ok(value),
            (CgTy::Struct(from), CgTy::Struct(to)) if from == to => Ok(value),
            (CgTy::Enum(from), CgTy::Enum(to)) if from == to => Ok(value),
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "value coercion",
                at: at.into(),
            }),
        }
    }

    fn codegen_box_unit_to_ref(
        &mut self,
        at: crate::span::Span,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        // 约定（early stage）：
        // - box 对象布局：`{ header: ScoopGcObjectHeader }`（无 payload）
        // - 对象头字段由 runtime 的 `scoop_alloc` 初始化（与 `codegen_box_int_to_ref` 一致）。
        let target = self.target_layout();

        // 对象头布局与 C runtime 对齐（见 `runtime/c/scoop_gc.h` 的 static asserts）。
        let header_size = 2 * target.pointer_size + 16;
        let header_align = target.pointer_align.max(8).max(1);
        let total_size = align_to(header_size, header_align);

        let rt_alloc = self.declare_runtime_alloc();
        let size_v = self.context.i64_type().const_int(total_size as u64, false);
        let call = self
            .builder
            .build_call(rt_alloc, &[size_v.into()], "rt_alloc_box_unit")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc return value",
                at: at.into(),
            })?;

        let BasicValueEnum::PointerValue(raw_ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc return type",
                at: at.into(),
            });
        };

        Ok(raw_ptr)
    }

    fn codegen_box_int_to_ref(
        &mut self,
        at: crate::span::Span,
        value: IntValue<'ctx>,
        value_ty: IntTy,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        // 约定（early stage）：
        // - box 对象布局：`{ header: ScoopGcObjectHeader, payload: <int> }`（TODO T0908）
        // - 当前阶段由 runtime 的 `scoop_alloc` 初始化对象头字段：
        //   - `next = NULL`
        //   - `type_desc = NULL`（后续由 typed alloc 或 codegen 写入；TODO T0907+）
        //   - `size_bytes = alloc_size`
        //   - `flags/mark = 0`
        //
        // 注意：这里不尝试做“复用 box 类型”或 cache；LLVM named struct 会在 module 内复用。
        let target = self.target_layout();
        let payload_size = (u64::from(value_ty.bits) + 7) / 8;
        let payload_align = payload_size.clamp(1, target.pointer_align.max(1));

        // 对象头布局与 C runtime 对齐（见 `runtime/c/scoop_gc.h` 的 static asserts）。
        //
        // `ScoopGcObjectHeader` 字段：
        // - next: void*
        // - type_desc: void*
        // - size_bytes: u64
        // - flags: u32
        // - mark: u32
        let header_size = 2 * target.pointer_size + 16;
        let header_align = target.pointer_align.max(8).max(1);
        let payload_offset = align_to(header_size, payload_align);
        let obj_align = header_align.max(payload_align);
        let total_size = align_to(payload_offset.saturating_add(payload_size), obj_align);

        let rt_alloc = self.declare_runtime_alloc();
        let size_v = self.context.i64_type().const_int(total_size as u64, false);
        let call = self
            .builder
            .build_call(rt_alloc, &[size_v.into()], "rt_alloc_box")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc return value",
                at: at.into(),
            })?;

        let BasicValueEnum::PointerValue(raw_ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc return type",
                at: at.into(),
            });
        };

        // 写入 payload（对象头由 runtime 初始化）。
        let boxed_ty = self.llvm_boxed_int_type(value_ty);
        let boxed_ptr_ty = boxed_ty.ptr_type(self.gc_address_space());
        let boxed_ptr = self
            .builder
            .build_pointer_cast(raw_ptr, boxed_ptr_ty, "boxed_int_ptr")?;

        let payload_ptr =
            self.builder
                .build_struct_gep(boxed_ty, boxed_ptr, 1, "boxed_payload_gep")?;
        let _ = self.builder.build_store(payload_ptr, value)?;

        Ok(raw_ptr)
    }

    fn cast_int(
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

    fn coerce_exit_code(
        &mut self,
        at: crate::span::Span,
        value: CgValue<'ctx>,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let i32_ty = self.context.i32_type();

        match value.ty {
            CgTy::Unit => Ok(i32_ty.const_int(0, false)),
            CgTy::Bool => {
                let b = value.as_bool().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "exit bool",
                    at: at.into(),
                })?;
                Ok(self.builder.build_int_z_extend(b, i32_ty, "exit_bool")?)
            }
            CgTy::Int(int_ty) => {
                let (v, from) = value.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "exit int",
                    at: at.into(),
                })?;
                let to = IntTy {
                    bits: 32,
                    signed: int_ty.signed,
                };
                let casted = self.cast_int(v, from, to)?;
                Ok(casted)
            }
            CgTy::String => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "string exit code",
                at: at.into(),
            }),
            CgTy::Ref => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "ref exit code",
                at: at.into(),
            }),
            CgTy::Tuple(_) => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "tuple exit code",
                at: at.into(),
            }),
            CgTy::Struct(_) | CgTy::Enum(_) => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "composite exit code",
                at: at.into(),
            }),
        }
    }

    fn cg_ty_of(&self, ty: TypeId) -> Option<CgTy> {
        match self.types.kind(ty) {
            TypeKind::Ref(RefTypeKind::String) => Some(CgTy::String),
            // T0622：`Task<T>` 在早期阶段先落到 “word-sized handle”（runtime 用 `uint64_t` 承载）。
            // 为保持 run-pass/codegen 可回归，这里把它视为 `UInt` 风格的整数句柄类型。
            TypeKind::Ref(RefTypeKind::Nominal(nominal)) if nominal.fqn == "scoop.core.Task" => {
                Some(CgTy::Int(IntTy {
                    bits: self.host.word_bit_width(),
                    signed: false,
                }))
            }
            // T1319e：std v3 executor 句柄在 early stage 与 `Task<T>` 一致：落到 word-sized handle（u64）。
            TypeKind::Ref(RefTypeKind::Nominal(nominal)) if nominal.fqn == "scoop.task.Executor" => {
                Some(CgTy::Int(IntTy {
                    bits: self.host.word_bit_width(),
                    signed: false,
                }))
            }
            TypeKind::Ref(_) => Some(CgTy::Ref),
            TypeKind::Value(ValueTypeKind::Unit) => Some(CgTy::Unit),
            TypeKind::Value(ValueTypeKind::Bool) => Some(CgTy::Bool),
            TypeKind::Value(ValueTypeKind::Int) => Some(CgTy::Int(IntTy {
                bits: self.host.word_bit_width(),
                signed: true,
            })),
            TypeKind::Value(ValueTypeKind::UInt) => Some(CgTy::Int(IntTy {
                bits: self.host.word_bit_width(),
                signed: false,
            })),
            TypeKind::Value(ValueTypeKind::IntN(bits)) => Some(CgTy::Int(IntTy {
                bits: u32::from(*bits),
                signed: true,
            })),
            TypeKind::Value(ValueTypeKind::UIntN(bits)) => Some(CgTy::Int(IntTy {
                bits: u32::from(*bits),
                signed: false,
            })),
            TypeKind::Value(ValueTypeKind::Option(_)) => Some(CgTy::Enum(ty)),
            TypeKind::Value(ValueTypeKind::Tuple(_)) => Some(CgTy::Tuple(ty)),
            TypeKind::Value(ValueTypeKind::Nominal(nominal)) => {
                // T1027：internal atomics（`__AtomicInt`）——值类型、与底层整数相同布局。
                //
                // 说明：
                // - typecheck 内部会为 typealias 保留一个名义 `TypeId`（便于诊断/审计），
                //   但后端必须把它映射到与 `Int` 完全一致的 ABI（word-sized, signed）。
                if nominal.fqn == "scoop.unsafe.__AtomicInt" {
                    return Some(CgTy::Int(IntTy {
                        bits: self.host.word_bit_width(),
                        signed: true,
                    }));
                }
                // `UIntPtr`（typealias）：在 early stage 直接落到 word-sized unsigned int。
                if nominal.fqn == "scoop.core.UIntPtr" {
                    return Some(CgTy::Int(IntTy {
                        bits: self.host.word_bit_width(),
                        signed: false,
                    }));
                }
                // T1026：`FunPtr<F>` —— 运行期表示为 word-sized address（unsigned），并作为 opaque handle 传递。
                if nominal.fqn == "scoop.unsafe.FunPtr" {
                    return Some(CgTy::Int(IntTy {
                        bits: self.host.word_bit_width(),
                        signed: false,
                    }));
                }
                if self.struct_layouts.contains_key(&nominal.fqn) {
                    return Some(CgTy::Struct(ty));
                }
                if self.enum_layouts.contains_key(&nominal.fqn) {
                    return Some(CgTy::Enum(ty));
                }
                None
            }
            _ => None,
        }
    }

    /// 在当前 compilation unit 的 `TypeStore` 中查找 `() -> Unit / Pure` 的函数类型。
    ///
    /// 用途：
    /// - 一些 sysroot API（例如 `scoop.sync.Once.run`）在 early stage 是“只有声明没有 body 的外部落点”，
    ///   因此不在 `fun_index` 中；但 closure codegen 仍需要一个 expected function type 来确定参数绑定。
    fn lookup_pure_unit_closure_type(&self) -> Option<TypeId> {
        let unit = self.types.iter_ids().find(|id| {
            matches!(self.types.kind(*id), TypeKind::Value(ValueTypeKind::Unit))
        })?;

        self.types.iter_ids().find(|id| match self.types.kind(*id) {
            TypeKind::Ref(RefTypeKind::Function(fun_ty)) => {
                fun_ty.receiver.is_none()
                    && fun_ty.params.is_empty()
                    && fun_ty.return_ty == unit
                    && fun_ty.effects.is_pure()
            }
            _ => false,
        })
    }

    /// 在当前 compilation unit 的 `TypeStore` 中查找 `() -> Int` 形状的函数类型。
    ///
    /// 用途：
    /// - `scoop.task.taskCreate { ... }` 这类 sysroot API 只有声明、无 body，因此不在 `fun_index` 中；
    ///   但 closure codegen 仍需要一个 expected function type 来确定参数绑定与返回类型。
    fn lookup_pure_int_closure_type(&self) -> Option<TypeId> {
        let expected_bits = self.host.word_bit_width();

        self.types.iter_ids().find(|id| match self.types.kind(*id) {
            TypeKind::Ref(RefTypeKind::Function(fun_ty)) => {
                if fun_ty.receiver.is_some() || !fun_ty.params.is_empty() {
                    return false;
                }
                let Some(CgTy::Int(int_ty)) = self.cg_ty_of(fun_ty.return_ty) else {
                    return false;
                };
                int_ty.bits == expected_bits
            }
            _ => false,
        })
    }

    fn cg_ty_of_type_fqn(
        &self,
        at: crate::span::Span,
        ty_fqn: Option<&str>,
    ) -> Result<CgTy, LlvmEmitError> {
        let Some(ty_fqn) = ty_fqn else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "struct field type",
                at: at.into(),
            });
        };

        match ty_fqn {
            "scoop.core.Unit" => Ok(CgTy::Unit),
            "scoop.core.Bool" => Ok(CgTy::Bool),
            "scoop.core.Any" => Ok(CgTy::Ref),
            "scoop.core.String" => Ok(CgTy::String),
            "scoop.core.Int" => Ok(CgTy::Int(IntTy {
                bits: self.host.word_bit_width(),
                signed: true,
            })),
            "scoop.core.UInt" => Ok(CgTy::Int(IntTy {
                bits: self.host.word_bit_width(),
                signed: false,
            })),
            other => {
                // 固定位宽整数族（与 HIR lowering 的 special-case 规则对齐）。
                if let Some(bits) = other
                    .strip_prefix("scoop.core.Int")
                    .and_then(|s| s.parse::<u32>().ok())
                {
                    return Ok(CgTy::Int(IntTy { bits, signed: true }));
                }
                if let Some(bits) = other
                    .strip_prefix("scoop.core.UInt")
                    .and_then(|s| s.parse::<u32>().ok())
                {
                    return Ok(CgTy::Int(IntTy {
                        bits,
                        signed: false,
                    }));
                }

                Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "struct field type",
                    at: at.into(),
                })
            }
        }
    }

    fn int_type(&self, ty: IntTy) -> IntType<'ctx> {
        self.context.custom_width_int_type(ty.bits)
    }

    fn gc_address_space(&self) -> AddressSpace {
        AddressSpace::from(GC_ADDRSPACE)
    }

    /// LLVM addrspace(0)：native/unsafe 指针（C ABI / malloc buffer 等）。
    fn llvm_i8_ptr_type(&self) -> PointerType<'ctx> {
        self.context.i8_type().ptr_type(AddressSpace::default())
    }

    /// LLVM addrspace(1)：GC-managed 引用指针（Any/class/interface/closure/...）。
    fn llvm_gc_i8_ptr_type(&self) -> PointerType<'ctx> {
        self.context.i8_type().ptr_type(self.gc_address_space())
    }

    fn llvm_scoop_string_type(&self) -> StructType<'ctx> {
        // 说明：该类型名用于 LLVM module 内部复用，不应与用户类型冲突（使用 runtime 命名空间前缀）。
        const TY_NAME: &str = "scoop.runtime.ScoopString";

        if let Some(existing) = self.context.get_struct_type(TY_NAME) {
            return existing;
        }

        // `typedef struct { ScoopGcObjectHeader hdr; uint64_t len; const uint8_t *data; } ScoopString;`
        let ty = self.context.opaque_struct_type(TY_NAME);
        let header_ty = self.llvm_gc_object_header_type();
        let len_ty = self.context.i64_type();
        let data_ty = self.context.i8_type().ptr_type(AddressSpace::default());
        ty.set_body(&[header_ty.into(), len_ty.into(), data_ty.into()], false);
        ty
    }

    fn llvm_scoop_string_ptr_type(&self) -> inkwell::types::PointerType<'ctx> {
        self.llvm_scoop_string_type().ptr_type(self.gc_address_space())
    }

    fn llvm_gc_object_header_type(&self) -> StructType<'ctx> {
        // 说明：
        // - 该类型对应 `runtime/c/scoop_gc.h` 的 `ScoopGcObjectHeader`（TODO T0908）；
        // - 当前阶段用 `i8*` 作为 `next` 与 `type_desc` 的承载类型（不暴露具体指针类型）；
        // - 布局必须与 C runtime 一致，否则 `scoop_alloc` 初始化的对象头会被错误解释。
        const TY_NAME: &str = "scoop.runtime.ScoopGcObjectHeader";

        if let Some(existing) = self.context.get_struct_type(TY_NAME) {
            return existing;
        }

        // `typedef struct { void* next; void* type_desc; uint64_t size_bytes; uint32_t flags; uint32_t mark; } ScoopGcObjectHeader;`
        let ty = self.context.opaque_struct_type(TY_NAME);
        let i8_ptr_ty = self.context.i8_type().ptr_type(AddressSpace::default());
        let i64_ty = self.context.i64_type();
        let i32_ty = self.context.i32_type();
        ty.set_body(
            &[
                i8_ptr_ty.into(),
                i8_ptr_ty.into(),
                i64_ty.into(),
                i32_ty.into(),
                i32_ty.into(),
            ],
            false,
        );
        ty
    }

    fn llvm_boxed_int_type(&self, payload: IntTy) -> StructType<'ctx> {
        // 说明：box 类型目前只用于 `Int/UInt/... -> Any` 的最小装箱（T0817）。
        // 未来会扩展为统一的对象头 + type descriptor（T0907+）；当前已接入最小对象头（T0908）。
        let name = format!(
            "scoop.runtime.BoxedInt{}_{}",
            payload.bits,
            if payload.signed { "i" } else { "u" }
        );
        if let Some(existing) = self.context.get_struct_type(&name) {
            return existing;
        }

        // `{ ScoopGcObjectHeader header, <int> payload }`
        let ty = self.context.opaque_struct_type(&name);
        let header_ty = self.llvm_gc_object_header_type();
        ty.set_body(&[header_ty.into(), self.int_type(payload).into()], false);
        ty
    }

    fn llvm_closure_object_type(&self) -> StructType<'ctx> {
        // 说明：
        // - 该类型是 early stage 的函数值/闭包运行期表示（T0710/T1307b）。
        // - 当前只支持“非捕获 lambda”，因此 env 指针固定为 NULL；但 ABI 仍预留 env 字段。
        //
        // 布局（与 GC 对象头兼容）：
        // `{ header: ScoopGcObjectHeader, env_ptr: i8*, fn_ptr: i8* }`
        const TY_NAME: &str = "scoop.runtime.ScoopClosure";

        if let Some(existing) = self.context.get_struct_type(TY_NAME) {
            return existing;
        }

        let ty = self.context.opaque_struct_type(TY_NAME);
        let header_ty = self.llvm_gc_object_header_type();
        let i8_ptr_ty = self.context.i8_type().ptr_type(AddressSpace::default());
        ty.set_body(
            &[header_ty.into(), i8_ptr_ty.into(), i8_ptr_ty.into()],
            false,
        );
        ty
    }

    fn llvm_gc_frame_type(&self, root_count: u32) -> StructType<'ctx> {
        // 说明：
        // - `runtime/c/scoop_gc.h` 中的 `ScoopGcFrame` 使用 flexible array `roots[]`；
        // - 在 LLVM IR 中我们为每个不同的 `root_count` 生成一个具名 struct：
        //   `{ prev: i8*, root_count: i32, reserved: i32, roots: [N x i8 addrspace(1)*] }`。
        //
        // 只要前 3 个字段布局与 runtime 匹配，就能与 C 侧按前缀访问兼容（push/pop/scan）。
        let name = format!("scoop.runtime.ScoopGcFrame_{root_count}");
        if let Some(existing) = self.context.get_struct_type(&name) {
            return existing;
        }

        let ty = self.context.opaque_struct_type(&name);
        let frame_prev_ptr_ty = self.llvm_i8_ptr_type();
        let gc_root_ptr_ty = self.llvm_gc_i8_ptr_type();
        let i32_ty = self.context.i32_type();
        let roots_ty = gc_root_ptr_ty.array_type(root_count);
        ty.set_body(
            &[
                frame_prev_ptr_ty.into(),
                i32_ty.into(),
                i32_ty.into(),
                roots_ty.into(),
            ],
            false,
        );
        ty
    }

    fn llvm_effect_handler_frame_type(&self) -> StructType<'ctx> {
        // 说明：
        // - 该类型对应 `runtime/c/scoop_runtime.c` 的 `ScoopEffectHandlerFrame`（TODO T0913）；
        // - v0 只要求 `{ prev: i8*, op_tag: i32, active: i32 }` 的稳定布局；
        // - codegen 不直接访问字段，只负责在栈上分配并把指针传给 runtime push/pop/active API。
        const TY_NAME: &str = "scoop.runtime.ScoopEffectHandlerFrame";

        if let Some(existing) = self.context.get_struct_type(TY_NAME) {
            return existing;
        }

        let ty = self.context.opaque_struct_type(TY_NAME);
        let i8_ptr_ty = self.context.i8_type().ptr_type(AddressSpace::default());
        let i32_ty = self.context.i32_type();
        ty.set_body(&[i8_ptr_ty.into(), i32_ty.into(), i32_ty.into()], false);
        ty
    }

    fn declare_runtime_print_like(&self, name: &str) -> FunctionValue<'ctx> {
        if let Some(existing) = self.module.get_function(name) {
            return existing;
        }

        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] =
            [self.llvm_scoop_string_ptr_type().into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(name, fn_ty, None)
    }

    fn declare_runtime_format_int(&self, name: &str) -> FunctionValue<'ctx> {
        if let Some(existing) = self.module.get_function(name) {
            return existing;
        }

        // `uint64_t scoop_format_{i64,u64}(int64_t value, uint8_t* out, uint64_t cap)`
        //
        // 说明：
        // - 该函数用于 f-string 插值 `{Int}` 的最小 formatting（TODO T0823）；
        // - 由 runtime 实现，避免在 LLVM IR 中直接引入 varargs `snprintf` 调用。
        let i64_ty = self.context.i64_type();
        let i8_ptr_ty = self.context.i8_type().ptr_type(AddressSpace::default());
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 3] =
            [i64_ty.into(), i8_ptr_ty.into(), i64_ty.into()];
        let fn_ty = i64_ty.fn_type(&param_tys, false);
        self.module.add_function(name, fn_ty, None)
    }

    fn declare_runtime_trim_indent(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_string_trim_indent";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `const ScoopString* scoop_string_trim_indent(const ScoopString* value)`
        let str_ptr_ty = self.llvm_scoop_string_ptr_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [str_ptr_ty.into()];
        let fn_ty = str_ptr_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_env_get(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_env_get";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `const ScoopString* scoop_env_get(const ScoopString* key)`
        let str_ptr_ty = self.llvm_scoop_string_ptr_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [str_ptr_ty.into()];
        let fn_ty = str_ptr_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_time_now_unix_millis(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_time_now_unix_millis";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `int64_t scoop_time_now_unix_millis(void)`
        let i64_ty = self.context.i64_type();
        let fn_ty = i64_ty.fn_type(&[], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_fs_read_all_text_utf8(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_fs_read_all_text_utf8";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `const ScoopString* scoop_fs_read_all_text_utf8(const ScoopString* path)`
        let str_ptr_ty = self.llvm_scoop_string_ptr_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [str_ptr_ty.into()];
        let fn_ty = str_ptr_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_fs_write_all_text_utf8(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_fs_write_all_text_utf8";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `int64_t scoop_fs_write_all_text_utf8(const ScoopString* path, const ScoopString* content)`
        let i64_ty = self.context.i64_type();
        let str_ptr_ty = self.llvm_scoop_string_ptr_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 2] = [str_ptr_ty.into(), str_ptr_ty.into()];
        let fn_ty = i64_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_process_exit(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_process_exit";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_process_exit(int64_t code)`
        let i64_ty = self.context.i64_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [i64_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_process_args_array(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_process_args_array";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void* scoop_process_args_array(void)`
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let fn_ty = gc_i8_ptr_ty.fn_type(&[], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_io_stdin_read_line_utf8(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_io_stdin_read_line_utf8";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `const ScoopString* scoop_io_stdin_read_line_utf8(void)`
        let str_ptr_ty = self.llvm_scoop_string_ptr_type();
        let fn_ty = str_ptr_ty.fn_type(&[], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_path_normalize(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_path_normalize";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `const ScoopString* scoop_path_normalize(const ScoopString* path)`
        let str_ptr_ty = self.llvm_scoop_string_ptr_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [str_ptr_ty.into()];
        let fn_ty = str_ptr_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_path_join(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_path_join";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `const ScoopString* scoop_path_join(const ScoopString* base, const ScoopString* child)`
        let str_ptr_ty = self.llvm_scoop_string_ptr_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 2] = [str_ptr_ty.into(), str_ptr_ty.into()];
        let fn_ty = str_ptr_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_path_basename(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_path_basename";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `const ScoopString* scoop_path_basename(const ScoopString* path)`
        let str_ptr_ty = self.llvm_scoop_string_ptr_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [str_ptr_ty.into()];
        let fn_ty = str_ptr_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_path_dirname(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_path_dirname";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `const ScoopString* scoop_path_dirname(const ScoopString* path)`
        let str_ptr_ty = self.llvm_scoop_string_ptr_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [str_ptr_ty.into()];
        let fn_ty = str_ptr_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    // --- std v3：sync（T1319b） ---

    fn declare_runtime_sync_mutex_create(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_sync_mutex_create";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void* scoop_sync_mutex_create(void)`
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let fn_ty = gc_i8_ptr_ty.fn_type(&[], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_sync_mutex_lock(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_sync_mutex_lock";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_sync_mutex_lock(void* mutex_obj)`
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [gc_i8_ptr_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_sync_mutex_unlock(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_sync_mutex_unlock";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_sync_mutex_unlock(void* mutex_obj)`
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [gc_i8_ptr_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_sync_mutex_destroy(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_sync_mutex_destroy";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_sync_mutex_destroy(void* mutex_obj)`
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [gc_i8_ptr_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_sync_condvar_create(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_sync_condvar_create";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void* scoop_sync_condvar_create(void)`
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let fn_ty = gc_i8_ptr_ty.fn_type(&[], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_sync_condvar_wait(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_sync_condvar_wait";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_sync_condvar_wait(void* condvar_obj, void* mutex_obj)`
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 2] =
            [gc_i8_ptr_ty.into(), gc_i8_ptr_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_sync_condvar_notify_one(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_sync_condvar_notify_one";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_sync_condvar_notify_one(void* condvar_obj)`
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [gc_i8_ptr_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_sync_condvar_notify_all(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_sync_condvar_notify_all";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_sync_condvar_notify_all(void* condvar_obj)`
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [gc_i8_ptr_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_sync_condvar_destroy(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_sync_condvar_destroy";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_sync_condvar_destroy(void* condvar_obj)`
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [gc_i8_ptr_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_sync_once_create(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_sync_once_create";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void* scoop_sync_once_create(void)`
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let fn_ty = gc_i8_ptr_ty.fn_type(&[], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_sync_once_is_done(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_sync_once_is_done";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `bool scoop_sync_once_is_done(void* once_obj)`
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let i1_ty = self.context.bool_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [gc_i8_ptr_ty.into()];
        let fn_ty = i1_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_sync_once_run(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_sync_once_run";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_sync_once_run(void* once_obj, void* env_ptr, void (*fn)(void* env_ptr))`
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let env_ptr_ty = self.llvm_i8_ptr_type();
        let init_fn_ty = self
            .context
            .void_type()
            .fn_type(&[env_ptr_ty.into()], false);
        let init_fn_ptr_ty = init_fn_ty.ptr_type(AddressSpace::default());
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 3] =
            [gc_i8_ptr_ty.into(), env_ptr_ty.into(), init_fn_ptr_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    // --- std v3：thread（T1319c） ---

    fn declare_runtime_thread_spawn(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_thread_spawn";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void* scoop_thread_spawn(void* env_ptr, void (*fn)(void* env_ptr))`
        let i8_ptr_ty = self.llvm_i8_ptr_type();
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let start_fn_ty = self
            .context
            .void_type()
            .fn_type(&[i8_ptr_ty.into()], false);
        let start_fn_ptr_ty = start_fn_ty.ptr_type(AddressSpace::default());
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 2] =
            [i8_ptr_ty.into(), start_fn_ptr_ty.into()];
        let fn_ty = gc_i8_ptr_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_thread_join(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_thread_join";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_thread_join(void* thread_obj)`
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [gc_i8_ptr_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_thread_yield(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_thread_yield";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_thread_yield(void)`
        let fn_ty = self.context.void_type().fn_type(&[], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_thread_sleep_millis(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_thread_sleep_millis";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_thread_sleep_millis(int64_t ms)`
        let i64_ty = self.context.i64_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [i64_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_thread_current_id(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_thread_current_id";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `int64_t scoop_thread_current_id(void)`
        let i64_ty = self.context.i64_type();
        let fn_ty = i64_ty.fn_type(&[], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    // --- std v3：channels（T1319d） ---

    fn declare_runtime_channels_channel_create(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_channels_channel_create";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void* scoop_channels_channel_create(void)`
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let fn_ty = gc_i8_ptr_ty.fn_type(&[], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_channels_send_u64(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_channels_send_u64";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `uint32_t scoop_channels_send_u64(void* channel, uint64_t value)`
        let i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let i64_ty = self.context.i64_type();
        let i32_ty = self.context.i32_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 2] = [i8_ptr_ty.into(), i64_ty.into()];
        let fn_ty = i32_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_channels_recv_u64(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_channels_recv_u64";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `uint32_t scoop_channels_recv_u64(void* channel, uint64_t* out_value)`
        let i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let i64_ty = self.context.i64_type();
        let i64_ptr_ty = i64_ty.ptr_type(AddressSpace::default());
        let i32_ty = self.context.i32_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 2] = [i8_ptr_ty.into(), i64_ptr_ty.into()];
        let fn_ty = i32_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_channels_close(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_channels_close";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_channels_close(void* channel)`
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [gc_i8_ptr_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    // --- std v3：task/executor（T1319e） ---

    fn declare_runtime_executor_create(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_executor_create";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `uint64_t scoop_executor_create(void)`
        let fn_ty = self.context.i64_type().fn_type(&[], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_executor_destroy(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_executor_destroy";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_executor_destroy(uint64_t executor_handle)`
        let i64_ty = self.context.i64_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [i64_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_executor_debug_pending_count(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_executor_debug_pending_count";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `uint64_t scoop_executor_debug_pending_count(uint64_t executor_handle)`
        let i64_ty = self.context.i64_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [i64_ty.into()];
        let fn_ty = i64_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_executor_run_next(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_executor_run_next";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `uint64_t scoop_executor_run_next(uint64_t executor_handle)`
        let i64_ty = self.context.i64_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [i64_ty.into()];
        let fn_ty = i64_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_executor_run_until_idle(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_executor_run_until_idle";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `uint64_t scoop_executor_run_until_idle(uint64_t executor_handle, uint64_t max_steps)`
        let i64_ty = self.context.i64_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 2] = [i64_ty.into(), i64_ty.into()];
        let fn_ty = i64_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_task_u64_create(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_task_u64_create";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `uint64_t scoop_task_u64_create(uint64_t (*body_fn)(void*), void* body_ctx)`
        let i8_ptr_ty = self.context.i8_type().ptr_type(AddressSpace::default());
        let i64_ty = self.context.i64_type();
        let body_fn_ty = i64_ty.fn_type(&[i8_ptr_ty.into()], false);
        let body_fn_ptr_ty = body_fn_ty.ptr_type(AddressSpace::default());
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 2] =
            [body_fn_ptr_ty.into(), i8_ptr_ty.into()];
        let fn_ty = i64_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_task_u64_state(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_task_u64_state";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `uint32_t scoop_task_u64_state(uint64_t task_handle)`
        let i64_ty = self.context.i64_type();
        let i32_ty = self.context.i32_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [i64_ty.into()];
        let fn_ty = i32_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_task_u64_result(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_task_u64_result";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `uint64_t scoop_task_u64_result(uint64_t task_handle)`
        let i64_ty = self.context.i64_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [i64_ty.into()];
        let fn_ty = i64_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_task_u64_try_start(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_task_u64_try_start";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `uint32_t scoop_task_u64_try_start(uint64_t task_handle, uint64_t executor_handle)`
        let i64_ty = self.context.i64_type();
        let i32_ty = self.context.i32_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 2] = [i64_ty.into(), i64_ty.into()];
        let fn_ty = i32_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_task_u64_complete(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_task_u64_complete";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `uint32_t scoop_task_u64_complete(uint64_t task_handle, uint64_t value)`
        let i64_ty = self.context.i64_type();
        let i32_ty = self.context.i32_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 2] = [i64_ty.into(), i64_ty.into()];
        let fn_ty = i32_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_task_u64_on_complete_resume_u64(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_task_u64_on_complete_resume_u64";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `uint32_t scoop_task_u64_on_complete_resume_u64(uint64_t task_handle, uint64_t executor_handle, void* continuation)`
        let i64_ty = self.context.i64_type();
        let i32_ty = self.context.i32_type();
        let i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 3] =
            [i64_ty.into(), i64_ty.into(), i8_ptr_ty.into()];
        let fn_ty = i32_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_array_builder_new(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_array_builder_new";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void* scoop_array_builder_new(void)`
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let fn_ty = gc_i8_ptr_ty.fn_type(&[], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_array_builder_push_u64(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_array_builder_push_u64";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_array_builder_push_u64(void* builder, uint64_t value)`
        let i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let i64_ty = self.context.i64_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 2] = [i8_ptr_ty.into(), i64_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_array_builder_push_ref(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_array_builder_push_ref";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_array_builder_push_ref(void* builder, void* value)`
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 2] =
            [gc_i8_ptr_ty.into(), gc_i8_ptr_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_array_builder_build_array(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_array_builder_build_array";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void* scoop_array_builder_build_array(void* builder)`
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [gc_i8_ptr_ty.into()];
        let fn_ty = gc_i8_ptr_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_array_builder_build_mutable_array(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_array_builder_build_mutable_array";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void* scoop_array_builder_build_mutable_array(void* builder)`
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [gc_i8_ptr_ty.into()];
        let fn_ty = gc_i8_ptr_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_array_len(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_array_len";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `uint64_t scoop_array_len(void* array_obj)`
        let i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let i64_ty = self.context.i64_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [i8_ptr_ty.into()];
        let fn_ty = i64_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_array_get_u64(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_array_get_u64";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `uint64_t scoop_array_get_u64(void* array_obj, int64_t index)`
        let i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let i64_ty = self.context.i64_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 2] = [i8_ptr_ty.into(), i64_ty.into()];
        let fn_ty = i64_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_array_get_ref(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_array_get_ref";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void* scoop_array_get_ref(void* array_obj, int64_t index)`
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let i64_ty = self.context.i64_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 2] =
            [gc_i8_ptr_ty.into(), i64_ty.into()];
        let fn_ty = gc_i8_ptr_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_array_set_u64(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_array_set_u64";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_array_set_u64(void* array_obj, int64_t index, uint64_t value)`
        let i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let i64_ty = self.context.i64_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 3] =
            [i8_ptr_ty.into(), i64_ty.into(), i64_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_array_set_ref(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_array_set_ref";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_array_set_ref(void* array_obj, int64_t index, void* value)`
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let i64_ty = self.context.i64_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 3] =
            [gc_i8_ptr_ty.into(), i64_ty.into(), gc_i8_ptr_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_alloc(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_alloc";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void *scoop_alloc(uint64_t size)`
        let i64_ty = self.context.i64_type();
        let i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [i64_ty.into()];
        let fn_ty = i8_ptr_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_once_begin(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_once_begin";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `uint32_t scoop_once_begin(uint64_t* guard_word)`（TODO T0918）
        let i32_ty = self.context.i32_type();
        let i64_ptr_ty = self.context.i64_type().ptr_type(AddressSpace::default());
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [i64_ptr_ty.into()];
        let fn_ty = i32_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_once_end(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_once_end";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_once_end(uint64_t* guard_word)`（TODO T0918）
        let i64_ptr_ty = self.context.i64_type().ptr_type(AddressSpace::default());
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [i64_ptr_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_gc_frame_push(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_gc_frame_push";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_gc_frame_push(ScoopGcFrame* frame)`
        let i8_ptr_ty = self.context.i8_type().ptr_type(AddressSpace::default());
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [i8_ptr_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_gc_frame_pop(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_gc_frame_pop";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_gc_frame_pop(ScoopGcFrame* frame)`
        let i8_ptr_ty = self.context.i8_type().ptr_type(AddressSpace::default());
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [i8_ptr_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_gc_debug_count_roots_current_thread(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_gc_debug_count_roots_current_thread";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `uint64_t scoop_gc_debug_count_roots_current_thread(void)`
        let fn_ty = self.context.i64_type().fn_type(&[], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_gc_collect(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_gc_collect";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_gc_collect(void)`
        let fn_ty = self.context.void_type().fn_type(&[], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_gc_pin(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_pin";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `uint32_t scoop_pin(void* obj)`
        let i32_ty = self.context.i32_type();
        let i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [i8_ptr_ty.into()];
        let fn_ty = i32_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_gc_unpin(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_unpin";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `uint32_t scoop_unpin(void* obj)`
        let i32_ty = self.context.i32_type();
        let i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [i8_ptr_ty.into()];
        let fn_ty = i32_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_gc_debug_heap_object_count(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_gc_debug_heap_object_count";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `uint64_t scoop_gc_debug_heap_object_count(void)`
        let fn_ty = self.context.i64_type().fn_type(&[], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_gc_debug_alloc_garbage(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_gc_debug_alloc_garbage";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_gc_debug_alloc_garbage(int64_t count)`
        let i64_ty = self.context.i64_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [i64_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_task_spawn_int(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_task_spawn_int";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `uint64_t scoop_task_spawn_int(int64_t value)`
        let i64_ty = self.context.i64_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [i64_ty.into()];
        let fn_ty = i64_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_task_join_int(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_task_join_int";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `int64_t scoop_task_join_int(uint64_t handle)`
        let i64_ty = self.context.i64_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [i64_ty.into()];
        let fn_ty = i64_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_effect_is_active(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_effect_is_active";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `uint32_t scoop_effect_is_active(void)`
        let fn_ty = self.context.i32_type().fn_type(&[], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_effect_set_active(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_effect_set_active";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_effect_set_active(void)`
        let fn_ty = self.context.void_type().fn_type(&[], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_effect_set_active_with_trace(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_effect_set_active_with_trace";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_effect_set_active_with_trace(uint32_t src_line, uint32_t src_col)`
        let i32_ty = self.context.i32_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 2] = [i32_ty.into(), i32_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_effect_clear(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_effect_clear";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_effect_clear(void)`
        let fn_ty = self.context.void_type().fn_type(&[], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_effect_handler_stack_push(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_effect_handler_stack_push";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_effect_handler_stack_push(ScoopEffectHandlerFrame* frame, uint32_t op_tag)`
        let i8_ptr_ty = self.context.i8_type().ptr_type(AddressSpace::default());
        let i32_ty = self.context.i32_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 2] = [i8_ptr_ty.into(), i32_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_effect_handler_stack_pop(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_effect_handler_stack_pop";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_effect_handler_stack_pop(ScoopEffectHandlerFrame* frame)`
        let i8_ptr_ty = self.context.i8_type().ptr_type(AddressSpace::default());
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [i8_ptr_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_effect_handler_stack_set_active(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_effect_handler_stack_set_active";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_effect_handler_stack_set_active(ScoopEffectHandlerFrame* frame, uint32_t active)`
        let i8_ptr_ty = self.context.i8_type().ptr_type(AddressSpace::default());
        let i32_ty = self.context.i32_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 2] = [i8_ptr_ty.into(), i32_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_effect_handler_stack_swap_top(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_effect_handler_stack_swap_top";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void* scoop_effect_handler_stack_swap_top(void* new_top)`
        let i8_ptr_ty = self.context.i8_type().ptr_type(AddressSpace::default());
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [i8_ptr_ty.into()];
        let fn_ty = i8_ptr_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_continuation_alloc(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_continuation_alloc";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void* scoop_continuation_alloc(void* state, void (*step_fn)(void* state, uint64_t value))`
        let state_ptr_ty = self.llvm_gc_i8_ptr_type();
        let i64_ty = self.context.i64_type();
        let step_fn_ty = self
            .context
            .void_type()
            .fn_type(&[state_ptr_ty.into(), i64_ty.into()], false)
            .ptr_type(AddressSpace::default());
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 2] =
            [state_ptr_ty.into(), step_fn_ty.into()];
        let fn_ty = state_ptr_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_continuation_resume_u64(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_continuation_resume_u64";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_continuation_resume_u64(void* k, uint64_t resume_value)`
        let i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let i64_ty = self.context.i64_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 2] = [i8_ptr_ty.into(), i64_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_thread_spawn_join_resume_u64(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_thread_spawn_join_resume_u64";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_thread_spawn_join_resume_u64(void* k, uint64_t resume_value)`
        let i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let i64_ty = self.context.i64_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 2] = [i8_ptr_ty.into(), i64_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_effect_perform_slot_write_u64(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_effect_perform_slot_write_u64";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_effect_perform_slot_write_u64(uint32_t op_tag, uint64_t value)`
        let i32_ty = self.context.i32_type();
        let i64_ty = self.context.i64_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 2] = [i32_ty.into(), i64_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_effect_perform_slot_write_u64_2(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_effect_perform_slot_write_u64_2";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `void scoop_effect_perform_slot_write_u64_2(uint32_t op_tag, uint64_t word0, uint64_t word1)`
        let i32_ty = self.context.i32_type();
        let i64_ty = self.context.i64_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 3] =
            [i32_ty.into(), i64_ty.into(), i64_ty.into()];
        let fn_ty = self.context.void_type().fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_effect_perform_slot_read_op_tag(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_effect_perform_slot_read_op_tag";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `uint32_t scoop_effect_perform_slot_read_op_tag(void)`
        let fn_ty = self.context.i32_type().fn_type(&[], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_effect_perform_slot_read_len_words(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_effect_perform_slot_read_len_words";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `uint32_t scoop_effect_perform_slot_read_len_words(void)`
        let fn_ty = self.context.i32_type().fn_type(&[], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_effect_perform_slot_read_u64(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_effect_perform_slot_read_u64";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `uint64_t scoop_effect_perform_slot_read_u64(void)`
        let fn_ty = self.context.i64_type().fn_type(&[], false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn declare_runtime_effect_perform_slot_read_u64_at(&self) -> FunctionValue<'ctx> {
        const NAME: &str = "scoop_effect_perform_slot_read_u64_at";
        if let Some(existing) = self.module.get_function(NAME) {
            return existing;
        }

        // `uint64_t scoop_effect_perform_slot_read_u64_at(uint32_t index)`
        let i32_ty = self.context.i32_type();
        let i64_ty = self.context.i64_type();
        let param_tys: [BasicMetadataTypeEnum<'ctx>; 1] = [i32_ty.into()];
        let fn_ty = i64_ty.fn_type(&param_tys, false);
        self.module.add_function(NAME, fn_ty, None)
    }

    fn get_or_create_global_bytes(
        &self,
        span: crate::span::Span,
        bytes: &[u8],
    ) -> GlobalValue<'ctx> {
        let name = format!("__scoop_str_data_{}_{}", span.start, span.end);
        if let Some(existing) = self.module.get_global(&name) {
            return existing;
        }

        let arr_ty = self.context.i8_type().array_type(bytes.len() as u32);
        let gv = self.module.add_global(arr_ty, None, &name);
        let init = self.context.const_string(bytes, false);
        gv.set_initializer(&init);
        gv.set_constant(true);
        gv
    }

    fn llvm_basic_type_of(
        &mut self,
        at: crate::span::Span,
        ty: CgTy,
    ) -> Result<BasicTypeEnum<'ctx>, LlvmEmitError> {
        Ok(match ty {
            // 说明：Unit 没有运行期值；当前阶段仅用于“可放入 alloca”与保持 load/store 管线统一。
            CgTy::Unit => self.context.i8_type().into(),
            CgTy::Bool => self.context.bool_type().into(),
            CgTy::Int(int_ty) => self.int_type(int_ty).into(),
            CgTy::String => self.llvm_scoop_string_ptr_type().into(),
            CgTy::Ref => self.llvm_gc_i8_ptr_type().into(),
            CgTy::Tuple(tuple_ty) => self.llvm_tuple_type(at, tuple_ty)?.into(),
            CgTy::Struct(struct_ty) => self.llvm_struct_type(at, struct_ty)?.into(),
            CgTy::Enum(enum_ty) => self.llvm_enum_value_type(at, enum_ty)?,
        })
    }

    fn llvm_struct_type(
        &mut self,
        at: crate::span::Span,
        ty: TypeId,
    ) -> Result<StructType<'ctx>, LlvmEmitError> {
        let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = self.types.kind(ty) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "struct type id",
                at: at.into(),
            });
        };

        let layout =
            self.struct_layouts
                .get(&nominal.fqn)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "struct layout",
                    at: at.into(),
                })?;

        if let Some(existing) = self.context.get_struct_type(&layout.fqn) {
            return Ok(existing);
        }

        let struct_ty = self.context.opaque_struct_type(&layout.fqn);

        let mut llvm_fields: Vec<BasicTypeEnum<'ctx>> = Vec::with_capacity(layout.fields.len());
        for field in &layout.fields {
            let field_cg = self.cg_ty_of_type_fqn(field.span, field.ty_fqn.as_deref())?;
            llvm_fields.push(self.llvm_basic_type_of(field.span, field_cg)?);
        }

        let is_packed = layout
            .c_layout
            .as_ref()
            .and_then(|c| c.packed)
            .is_some();
        struct_ty.set_body(&llvm_fields, is_packed);
        Ok(struct_ty)
    }

    /// 生成（或获取）某个 class 的 payload struct 类型：`{ field0, field1, ... }`。
    ///
    /// 说明：
    /// - payload 不包含对象头（header）；header 由 `llvm_class_object_type` 负责；
    /// - 当前阶段 fields 的顺序来自 `hir::ClassInit.fields`（stable order），用于可回归的字段索引；
    /// - 该类型名使用 runtime 命名空间前缀，避免与用户类型冲突。
    fn llvm_class_payload_type(
        &mut self,
        at: crate::span::Span,
        class: &hir::ClassInit,
    ) -> Result<StructType<'ctx>, LlvmEmitError> {
        let name = format!(
            "scoop.runtime.ClassPayload__{}",
            sanitize_llvm_ident(&class.fqn)
        );
        if let Some(existing) = self.context.get_struct_type(&name) {
            if existing.is_opaque() {
                let mut llvm_fields: Vec<BasicTypeEnum<'ctx>> =
                    Vec::with_capacity(class.fields.len());
                for field in &class.fields {
                    let field_cg =
                        self.cg_ty_of(field.ty)
                            .ok_or(LlvmEmitError::UnsupportedMainBody {
                                kind: "class field type",
                                at: at.into(),
                            })?;
                    llvm_fields.push(self.llvm_basic_type_of(at, field_cg)?);
                }
                existing.set_body(&llvm_fields, false);
            }
            return Ok(existing);
        }

        let payload_ty = self.context.opaque_struct_type(&name);
        let mut llvm_fields: Vec<BasicTypeEnum<'ctx>> = Vec::with_capacity(class.fields.len());
        for field in &class.fields {
            let field_cg = self
                .cg_ty_of(field.ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "class field type",
                    at: at.into(),
                })?;
            llvm_fields.push(self.llvm_basic_type_of(at, field_cg)?);
        }
        payload_ty.set_body(&llvm_fields, false);
        Ok(payload_ty)
    }

    /// 生成（或获取）某个 class 的“对象布局”类型：`{ header, payload }`。
    ///
    /// 说明：
    /// - runtime `scoop_alloc(size)` 返回值指向对象头起始地址；
    /// - codegen 侧通过把该 `i8*` cast 为该 struct 指针，再用 `struct_gep` 访问 payload/field；
    /// - 该类型名仅用于 LLVM module 内部布局推导与 GEP，不直接暴露到语言层面。
    fn llvm_class_object_type(
        &mut self,
        at: crate::span::Span,
        class: &hir::ClassInit,
    ) -> Result<StructType<'ctx>, LlvmEmitError> {
        let name = format!(
            "scoop.runtime.ClassObject__{}",
            sanitize_llvm_ident(&class.fqn)
        );
        if let Some(existing) = self.context.get_struct_type(&name) {
            if existing.is_opaque() {
                let header_ty = self.llvm_gc_object_header_type();
                let payload_ty = self.llvm_class_payload_type(at, class)?;
                existing.set_body(&[header_ty.into(), payload_ty.into()], false);
            }
            return Ok(existing);
        }

        let obj_ty = self.context.opaque_struct_type(&name);
        let header_ty = self.llvm_gc_object_header_type();
        let payload_ty = self.llvm_class_payload_type(at, class)?;
        obj_ty.set_body(&[header_ty.into(), payload_ty.into()], false);
        Ok(obj_ty)
    }

    fn llvm_enum_value_type(
        &mut self,
        at: crate::span::Span,
        ty: TypeId,
    ) -> Result<BasicTypeEnum<'ctx>, LlvmEmitError> {
        // 注意：避免持有 `cg_enum_layout(...)` 返回的引用跨越后续 `&mut self` 调用。
        let (repr, some_field) = {
            let cg_layout = self.cg_enum_layout(at, ty)?;
            let repr = cg_layout.repr;
            let some_field = cg_layout
                .variants
                .iter()
                .find(|v| v.name == "Some")
                .and_then(|v| v.fields.first())
                .copied();
            (repr, some_field)
        };

        match repr {
            CgEnumRepr::TaggedUnion => {
                let fqn = match self.types.kind(ty) {
                    TypeKind::Value(ValueTypeKind::Option(_)) => "scoop.core.Option",
                    TypeKind::Value(ValueTypeKind::Nominal(nominal)) => nominal.fqn.as_str(),
                    _ => {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "enum type id",
                            at: at.into(),
                        });
                    }
                };

                if let Some(existing) = self.context.get_struct_type(fqn) {
                    return Ok(existing.into());
                }

                // 最小 rich enum 表示：`{ tag: i32, payload: iN }`
                // - tag：按声明顺序分配的 variant id
                // - payload：当前阶段用 machine word 承载 payload 或 boxed payload 指针
                let enum_ty = self.context.opaque_struct_type(fqn);
                let tag_ty = self.context.i32_type();
                let payload_ty = self.int_type(IntTy {
                    bits: self.host.word_bit_width(),
                    signed: false,
                });
                enum_ty.set_body(&[tag_ty.into(), payload_ty.into()], false);
                Ok(enum_ty.into())
            }
            CgEnumRepr::Niche { storage, .. } => match storage {
                NicheStorage::Pointer => {
                    let some_field = some_field.ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "Option niche payload type",
                        at: at.into(),
                    })?;
                    Ok(self.llvm_basic_type_of(at, some_field)?)
                }
                NicheStorage::U8 => Ok(self.context.i8_type().into()),
            },
            CgEnumRepr::ValueOnly { underlying } => Ok(self.int_type(underlying).into()),
        }
    }

    fn llvm_tuple_type(
        &mut self,
        at: crate::span::Span,
        ty: TypeId,
    ) -> Result<StructType<'ctx>, LlvmEmitError> {
        let TypeKind::Value(ValueTypeKind::Tuple(elements)) = self.types.kind(ty) else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "tuple type id",
                at: at.into(),
            });
        };

        let mut llvm_fields: Vec<BasicTypeEnum<'ctx>> = Vec::with_capacity(elements.len());
        for elem_ty in elements {
            let elem_cg = self
                .cg_ty_of(*elem_ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "tuple element type",
                    at: at.into(),
                })?;
            llvm_fields.push(self.llvm_basic_type_of(at, elem_cg)?);
        }

        Ok(self.context.struct_type(&llvm_fields, false))
    }

    fn llvm_enum_boxed_payload_struct_type(
        &mut self,
        at: crate::span::Span,
        enum_ty: TypeId,
        variant: &CgEnumVariant,
    ) -> Result<StructType<'ctx>, LlvmEmitError> {
        let enum_fqn = match self.types.kind(enum_ty) {
            TypeKind::Value(ValueTypeKind::Option(_)) => "scoop.core.Option",
            TypeKind::Value(ValueTypeKind::Nominal(nominal)) => nominal.fqn.as_str(),
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "enum boxed payload type",
                    at: at.into(),
                });
            }
        };

        // 说明：boxed payload 在运行期是一个独立的聚合对象；当前阶段用一个具名 LLVM struct 承载其字段布局，
        // 以便 ctor/binder 双方对齐类型（避免 bitcast 到不一致的匿名 struct）。
        let name = format!(
            "scoop_boxed_payload_{}_{}",
            sanitize_llvm_ident(enum_fqn),
            sanitize_llvm_ident(&variant.name)
        );

        if let Some(existing) = self.context.get_struct_type(&name) {
            return Ok(existing);
        }

        let payload_ty = self.context.opaque_struct_type(&name);
        let mut llvm_fields: Vec<BasicTypeEnum<'ctx>> = Vec::with_capacity(variant.fields.len());
        for &field_cg in &variant.fields {
            llvm_fields.push(self.llvm_basic_type_of(at, field_cg)?);
        }
        payload_ty.set_body(&llvm_fields, false);
        Ok(payload_ty)
    }

    fn create_entry_alloca(
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

    fn create_entry_alloca_raw(
        &self,
        at: crate::span::Span,
        name: &str,
        alloca_ty: BasicTypeEnum<'ctx>,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let alloca_builder = self.context.create_builder();
        let insert_block =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: at.into(),
                })?;
        let func = insert_block
            .get_parent()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "builder has no parent function",
                at: at.into(),
            })?;
        let entry = func
            .get_first_basic_block()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "function has no entry block",
                at: at.into(),
            })?;

        match entry.get_first_instruction() {
            Some(inst) => alloca_builder.position_before(&inst),
            None => alloca_builder.position_at_end(entry),
        }

        Ok(alloca_builder.build_alloca(alloca_ty, name)?)
    }

    fn apply_alloca_alignment_for_ty(
        &self,
        at: crate::span::Span,
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

        let inst = ptr.as_instruction_value().ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "alloca instruction value",
            at: at.into(),
        })?;
        inst.set_alignment(aligned)?;
        Ok(())
    }

    fn store_local_value(
        &mut self,
        at: crate::span::Span,
        ptr: PointerValue<'ctx>,
        ty: CgTy,
        value: CgValue<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        // 说明：当前阶段 locals 允许：
        // - 标量：`Unit/Bool/Int*`
        // - struct/enum（值类型）：以 LLVM struct by-value 形式存入栈 slot（`alloca`）
        let v = self.coerce_value(at, value, ty)?;
        match ty {
            CgTy::Unit => {
                let zero = self.context.i8_type().const_int(0, false);
                let _ = self.builder.build_store(ptr, zero)?;
            }
            CgTy::Bool
            | CgTy::Int(_)
            | CgTy::String
            | CgTy::Ref
            | CgTy::Tuple(_)
            | CgTy::Struct(_)
            | CgTy::Enum(_) => {
                let Some(raw) = v.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "store value",
                        at: at.into(),
                    });
                };
                let _ = self.builder.build_store(ptr, raw)?;
            }
        }
        Ok(v)
    }
}

fn object_init_fn_name(object_fqn: &str) -> String {
    format!("__scoop_object_init__{object_fqn}")
}

fn object_guard_global_name(object_fqn: &str) -> String {
    format!("__scoop_object_guard__{object_fqn}")
}

fn object_instance_global_name(object_fqn: &str) -> String {
    format!("__scoop_object_instance__{object_fqn}")
}

fn object_prop_global_name(prop_fqn: &str) -> String {
    format!("__scoop_object_prop__{prop_fqn}")
}

fn top_level_var_global_name(var_fqn: &str) -> String {
    format!("__scoop_top_level_var__{var_fqn}")
}

fn align_to(value: u64, align: u64) -> u64 {
    if align <= 1 {
        return value;
    }
    let mask = align - 1;
    (value + mask) & !mask
}

fn largest_two_sizes(layouts: &[TypeLayout]) -> (u64, u64) {
    let mut max = 0u64;
    let mut second = 0u64;
    for l in layouts {
        let s = l.size;
        if s >= max {
            second = max;
            max = s;
            continue;
        }
        if s > second {
            second = s;
        }
    }
    (max, second)
}

fn unify_int_types(
    lhs_is_lit: bool,
    lhs_ty: IntTy,
    rhs_is_lit: bool,
    rhs_ty: IntTy,
) -> Option<IntTy> {
    if lhs_ty == rhs_ty {
        return Some(lhs_ty);
    }
    if lhs_is_lit {
        return Some(rhs_ty);
    }
    if rhs_is_lit {
        return Some(lhs_ty);
    }
    None
}

fn parse_tuple_member_index(text: &str) -> Option<u32> {
    let digits = text.strip_prefix('_')?;
    if digits.is_empty() {
        return None;
    }
    if !digits.chars().all(|ch| ch.is_ascii_digit()) {
        return None;
    }
    digits.parse::<u32>().ok()
}

fn sanitize_llvm_ident(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() { "_".to_string() } else { out }
}

fn parse_int_literal_decimal(text: &str) -> u128 {
    let mut out: u128 = 0;
    for ch in text.chars() {
        if ch == '_' {
            continue;
        }
        if let Some(d) = ch.to_digit(10) {
            out = out.saturating_mul(10).saturating_add(u128::from(d));
        }
    }
    out
}

fn mask_to_bits(value: u128, bits: u32) -> u128 {
    if bits == 0 {
        return 0;
    }
    if bits >= 128 {
        return value;
    }
    let mask = (1u128 << bits) - 1;
    value & mask
}

fn parse_f_string_text_bytes(raw: bool, text: &str) -> Result<Vec<u8>, StringLiteralParseError> {
    // f-string 的 Text 片段来自 parser 拆分后的“内容区间 slice”，不包含包裹引号。
    // 这里需要补齐两类语义：
    // - `{{` / `}}`：字面量大括号（spec §8.2）；
    // - 非 raw f-string：支持最小转义（与普通字符串一致）。
    if raw {
        let undoubled = undouble_braces(text);
        return Ok(undoubled.into_bytes());
    }

    // 非 raw：先在源码层“去双大括号”，并避免把 `\u{...}` 的 `{}` 当作候选；
    // 再复用普通字符串的转义解析。
    let undoubled = undouble_braces_preserving_escapes(text);
    parse_normal_string_bytes(&undoubled)
}

fn undouble_braces(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '{' if matches!(chars.peek(), Some('{')) => {
                let _ = chars.next();
                out.push('{');
            }
            '}' if matches!(chars.peek(), Some('}')) => {
                let _ = chars.next();
                out.push('}');
            }
            _ => out.push(ch),
        }
    }
    out
}

fn undouble_braces_preserving_escapes(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\\' {
            // 转义序列中的 `{`/`}` 不参与 `{{`/`}}` 的消解。
            out.push('\\');
            let Some(next) = chars.next() else {
                break;
            };
            out.push(next);

            // `\u{...}`：把整个 `{...}` 视为转义语法的一部分，原样拷贝。
            if next == 'u' && matches!(chars.peek(), Some('{')) {
                out.push(chars.next().expect("peek 已保证存在"));
                while let Some(c) = chars.next() {
                    out.push(c);
                    if c == '}' {
                        break;
                    }
                }
            }
            continue;
        }

        match ch {
            '{' if matches!(chars.peek(), Some('{')) => {
                let _ = chars.next();
                out.push('{');
            }
            '}' if matches!(chars.peek(), Some('}')) => {
                let _ = chars.next();
                out.push('}');
            }
            _ => out.push(ch),
        }
    }

    out
}
