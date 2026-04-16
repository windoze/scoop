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
//! - `return`（以及"block 最后表达式"作为隐式返回）。
//! - `when`（T0813：仅支持 enum tag 判别 + variant binder；不支持 guard/or-pattern）。
//!
//! 非目标（后续任务逐步补齐）：
//! - if/loop 等更复杂控制流（依赖 MIR/CFG codegen 任务）。

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;

use inkwell::AddressSpace;
use inkwell::AtomicOrdering;
use inkwell::FloatPredicate;
use inkwell::IntPredicate;
use inkwell::attributes::{Attribute, AttributeLoc};
use inkwell::builder::Builder;
use inkwell::context::Context;
use inkwell::module::Linkage;
use inkwell::module::Module;
use inkwell::targets::TargetData;
use inkwell::types::AnyType;
use inkwell::types::BasicMetadataTypeEnum;
use inkwell::types::BasicType;
use inkwell::types::BasicTypeEnum;
use inkwell::types::IntType;
use inkwell::types::PointerType;
use inkwell::types::StructType;
use inkwell::values::AggregateValueEnum;
use inkwell::values::BasicValue;
use inkwell::values::BasicValueEnum;
use inkwell::values::CallSiteValue;
use inkwell::values::FloatValue;
use inkwell::values::FunctionValue;
use inkwell::values::GlobalValue;
use inkwell::values::IntValue;
use inkwell::values::PointerValue;
use sha2::{Digest as _, Sha256};

use crate::ast;
use crate::hir;
use crate::llvm::target::HostTargetInfo;
use crate::source::{SourceFile, SourceId, SourceMap};
use crate::syntax::int_literal::{parse_int_literal, parse_int_literal_checked};
use crate::syntax::string_literal::{
    StringLiteralParseError, parse_normal_string_bytes, parse_string_literal_bytes,
};
use crate::ty::layout::{NicheStorage, TypeLayout};
use crate::ty::{RefTypeKind, TypeId, TypeKind, TypeStore, ValueTypeKind};

use super::LlvmEmitError;

mod control_flow;
mod effect;
mod expr;
mod gc;
mod layout;
mod runtime_abi;
mod runtime_symbols;
mod stmt;
mod ty;
mod types;

use types::{
    CgEnumLayout, CgEnumPayload, CgEnumRepr, CgEnumVariant, CgTy, CgValue, GC_ADDRSPACE, IntTy,
};

/// Closure parameter bindings: explicit/implicit params paired with their types, plus remaining captures.
type ClosureParamBindings = (Vec<(hir::SymbolId, String, TypeId)>, Vec<hir::Capture>);

/// 一个局部变量（`val`/`var`）在 LLVM 里的存储形态。
///
/// 当前阶段（T0809）统一用栈分配（`alloca`）承载 locals，并用 `load/store` 实现读写。
#[derive(Debug, Clone, Copy)]
struct CgLocal<'ctx> {
    /// 该局部绑定在 HIR/type 层面的原始 `TypeId`（用于需要"精确类型结构"的场景，例如函数值调用）。
    ///
    /// 说明：
    /// - 早期 codegen 的 `CgTy::Ref` 统一覆盖所有引用类型（Any/class/function/union...），
    ///   但某些操作（例如调用函数值）仍需要区分具体的 `RefTypeKind::Function` 并读取其签名。
    /// - 对于无法在 codegen 阶段轻易恢复 `TypeId` 的合成 locals，可为 `None`。
    hir_ty: Option<TypeId>,
    ty: CgTy,
    ptr: PointerValue<'ctx>,
    mutable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AtomicIntLvalueMode {
    ReadOnly,
    ReadWrite,
}

#[derive(Debug, Default, Clone)]
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
    source_map: &'a SourceMap,
    entry_source_id: SourceId,
    current_source_id: SourceId,
    types: &'a TypeStore,
    struct_layouts: &'a hir::StructLayoutIndex,
    enum_layouts: &'a hir::EnumLayoutIndex,
    top_level_vars: &'a hir::TopLevelVarIndex,
    top_level_consts: &'a hir::TopLevelConstIndex,
    extern_funs: &'a hir::ExternFunIndex,
    object_inits: &'a hir::ObjectInitIndex,
    class_inits: &'a hir::ClassInitIndex,
    class_vtables: &'a crate::vtable::ClassVtableIndex,
    interfaces: &'a crate::itable::InterfaceIndex,
    class_itables: &'a crate::itable::ClassItableIndex,
    ctor_call_sites: &'a hir::CtorCallSiteIndex,
    continuation_resume_call_sites: &'a hir::ContinuationResumeCallSiteIndex,
    fun_index: &'a HashMap<String, &'a hir::FunDecl>,
    env: Env<'ctx>,
    /// `TypeId -> TypeLayout`（仅用于 codegen 侧的 niche 决策；不追求覆盖所有类型语法）。
    type_layout_cache: HashMap<TypeId, TypeLayout>,
    /// `Option<T>` niche 表示的 `None` 编码（用于 nested niche，例如 `Option<Option<Bool>>`）。
    ///
    /// 注意：对 GC-managed ref（Pointer niche）仅允许 `None = NULL`，并禁止剩余 domain 继续传播；
    /// 因此 `Option<Option<Ref>>` 外层会回退 tagged union（T1518；spec §2.3.2）。
    option_niche_cache: HashMap<TypeId, Option<(NicheStorage, u64)>>,
    /// `enum/Option` 的 codegen 表示选择与 boxing 决策缓存。
    enum_cg_layout_cache: HashMap<TypeId, CgEnumLayout>,
    /// `class FQN -> 继承链已展开的字段布局` 缓存。
    ///
    /// 说明：
    /// - 对于 `class Derived : Base()`，`Derived` 的对象 payload 需要以前缀形式包含 `Base` 的字段；
    /// - codegen 侧会把该布局"按继承链展开"，并把字段索引写回到 `field_indices`，以便 field GEP 正确。
    class_init_layout_cache: HashMap<String, hir::ClassInit>,
    /// 当前正在生成的函数返回类型。
    ///
    /// 说明：
    /// - 用于 `codegen_return_stmt` 确定返回值类型与 coercion 目标。
    /// - state machine emitter 在生成 step function 时保存/恢复此字段。
    current_fun_return_ty: Option<CgTy>,
    /// Effect op_tag 分配状态（T1608）：整个编译单元共享的 FQN → tag 表。
    ///
    /// 说明：
    /// - 每个 effect operation 的 FQN 在单次编译中对应唯一的 `op_tag`；
    /// - `scoop.core.Raise.raise` 固定为 1（与 runtime 约定兼容）；
    /// - 其余 effect op 从 2 开始递增分配；
    /// - 所有顶层函数 / nested helper / step trampoline 都必须共享同一份状态，
    ///   否则跨函数 perform 的 op_tag 会错位。
    effect_op_tags: Rc<RefCell<EffectOpTagState>>,
    /// T0119: `@CLayout(packed = N)` で N > 1 の場合、LLVM struct に挿入した padding 要素を
    /// 考慮して、「論理フィールド番号 → LLVM struct 要素番号」のマッピングを保持するキャッシュ。
    ///
    /// - `packed = 1`：LLVM ネイティブ packed struct、identity mapping → エントリなし。
    /// - `packed = N > 1`：explicit padding bytes を挿入するため index がずれる → エントリあり。
    /// - unpacked：identity mapping → エントリなし。
    pack_field_indices: HashMap<String, Vec<u32>>,
    /// T0141: Loop context stack for break/continue support.
    /// Each entry holds the break target (loop-after BB) and continue target (loop-head BB).
    loop_context_stack: Vec<LoopContext<'ctx>>,
    /// T0141: Function-level return context for early return support.
    /// When set, `return` statements branch to `return_bb` after storing the value in `return_alloca`.
    return_context: Option<ReturnContext<'ctx>>,
    /// 当前函数若采用 hidden sret ABI，则这里保存返回槽指针。
    ///
    /// 目前仅 higher-order 间接调用生成的 lambda 函数会设置该字段；
    /// `emit_return` 会据此改为“store 到 sret + ret void”。
    current_sret_return_ptr: Option<PointerValue<'ctx>>,
    /// 当前正在展开的顶层 `const val` 栈；用于避免递归引用导致无限 codegen。
    top_level_const_eval_stack: Vec<String>,
}

/// T0141: Loop context for break/continue targets.
#[derive(Debug, Clone, Copy)]
struct LoopContext<'ctx> {
    break_bb: inkwell::basic_block::BasicBlock<'ctx>,
    continue_bb: inkwell::basic_block::BasicBlock<'ctx>,
}

/// T0141: Function-level return context for early return from nested blocks.
#[derive(Debug, Clone, Copy)]
struct ReturnContext<'ctx> {
    return_bb: inkwell::basic_block::BasicBlock<'ctx>,
    /// Alloca for storing the return value (None for Unit return type).
    return_alloca: Option<inkwell::values::PointerValue<'ctx>>,
}

/// 稳定 effect op-tag 分配器：为每个 effect op FQN 分配唯一 u32 tag，
/// 供 state machine emitter、codegen_perform_expr 与 emit_raise 消费。
#[derive(Debug)]
pub(super) struct EffectOpTagState {
    map: HashMap<String, u32>,
    next: u32,
}

impl EffectOpTagState {
    pub(super) fn new() -> Self {
        let mut map = HashMap::new();
        // Raise.raise 固定为 1（与 runtime `scoop_continuation_resume_u64` 等约定兼容）。
        map.insert("scoop.core.Raise.raise".to_string(), 1u32);
        Self { map, next: 2 }
    }
}

pub(super) struct MainCodegenInputs<'a, 'ctx> {
    pub(super) context: &'ctx Context,
    pub(super) module: &'a Module<'ctx>,
    pub(super) builder: &'a Builder<'ctx>,
    pub(super) target_data: &'a TargetData,
    pub(super) host: &'a HostTargetInfo,
    pub(super) source_map: &'a SourceMap,
    pub(super) entry_source_id: SourceId,
    pub(super) types: &'a TypeStore,
    pub(super) struct_layouts: &'a hir::StructLayoutIndex,
    pub(super) enum_layouts: &'a hir::EnumLayoutIndex,
    pub(super) top_level_vars: &'a hir::TopLevelVarIndex,
    pub(super) top_level_consts: &'a hir::TopLevelConstIndex,
    pub(super) object_inits: &'a hir::ObjectInitIndex,
    pub(super) class_inits: &'a hir::ClassInitIndex,
    pub(super) class_vtables: &'a crate::vtable::ClassVtableIndex,
    pub(super) interfaces: &'a crate::itable::InterfaceIndex,
    pub(super) class_itables: &'a crate::itable::ClassItableIndex,
    pub(super) ctor_call_sites: &'a hir::CtorCallSiteIndex,
    pub(super) continuation_resume_call_sites: &'a hir::ContinuationResumeCallSiteIndex,
    pub(super) extern_funs: &'a hir::ExternFunIndex,
    pub(super) fun_index: &'a HashMap<String, &'a hir::FunDecl>,
    pub(super) effect_op_tags: Rc<RefCell<EffectOpTagState>>,
}

pub(super) struct TypeDescriptorSpec<'ctx, 'a> {
    pub(super) at: crate::span::Span,
    pub(super) global_name: &'a str,
    pub(super) canonical_name: &'a str,
    pub(super) obj_ty: StructType<'ctx>,
    pub(super) trace_start_offset_bytes: u64,
    pub(super) parent: Option<GlobalValue<'ctx>>,
    pub(super) itable: Option<PointerValue<'ctx>>,
    pub(super) vtable: Option<PointerValue<'ctx>>,
}

impl<'a, 'ctx> MainCodegen<'a, 'ctx> {
    pub(crate) fn new(inputs: MainCodegenInputs<'a, 'ctx>) -> Self {
        let MainCodegenInputs {
            context,
            module,
            builder,
            target_data,
            host,
            source_map,
            entry_source_id,
            types,
            struct_layouts,
            enum_layouts,
            top_level_vars,
            top_level_consts,
            object_inits,
            class_inits,
            class_vtables,
            interfaces,
            class_itables,
            ctor_call_sites,
            continuation_resume_call_sites,
            extern_funs,
            fun_index,
            effect_op_tags,
        } = inputs;
        Self {
            context,
            module,
            builder,
            target_data,
            host,
            source_map,
            entry_source_id,
            current_source_id: entry_source_id,
            types,
            struct_layouts,
            enum_layouts,
            top_level_vars,
            top_level_consts,
            extern_funs,
            object_inits,
            class_inits,
            class_vtables,
            interfaces,
            class_itables,
            ctor_call_sites,
            continuation_resume_call_sites,
            fun_index,
            env: Env::default(),
            type_layout_cache: HashMap::new(),
            option_niche_cache: HashMap::new(),
            enum_cg_layout_cache: HashMap::new(),
            class_init_layout_cache: HashMap::new(),
            current_fun_return_ty: None,
            effect_op_tags,
            pack_field_indices: HashMap::new(),
            loop_context_stack: Vec::new(),
            return_context: None,
            current_sret_return_ptr: None,
            top_level_const_eval_stack: Vec::new(),
        }
    }

    fn current_source(&self) -> Result<&SourceFile, LlvmEmitError> {
        self.source_map
            .source(self.current_source_id)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "current source lookup",
                at: crate::span::Span::new(0, 0).into(),
            })
    }

    fn source_id_for_path(
        &self,
        path: &Path,
        at: crate::span::Span,
    ) -> Result<SourceId, LlvmEmitError> {
        self.source_map
            .source_id_of_path(path)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "source file lookup",
                at: at.into(),
            })
    }

    fn source_slice_at(
        &self,
        source_id: SourceId,
        span: crate::span::Span,
    ) -> Result<&str, LlvmEmitError> {
        let bound = self.source_map.bind_span(source_id, span).map_err(|_| {
            LlvmEmitError::UnsupportedMainBody {
                kind: "source-backed literal span",
                at: span.into(),
            }
        })?;
        self.source_map
            .slice(bound)
            .map_err(|_| LlvmEmitError::UnsupportedMainBody {
                kind: "source-backed literal slice",
                at: span.into(),
            })
    }

    fn current_source_slice(&self, span: crate::span::Span) -> Result<&str, LlvmEmitError> {
        self.source_slice_at(self.current_source_id, span)
    }

    fn parse_current_int_literal(&self, span: crate::span::Span) -> Result<u128, LlvmEmitError> {
        Ok(parse_int_literal(self.current_source_slice(span)?))
    }

    fn int_literal_bits_for_ty(
        &self,
        span: crate::span::Span,
        int_ty: IntTy,
    ) -> Result<u64, LlvmEmitError> {
        let source = self.current_source()?;
        let text = self.current_source_slice(span)?;
        let raw = self.parse_current_int_literal(span)?;
        let bits = checked_positive_int_literal_bits(raw, int_ty).ok_or_else(|| {
            LlvmEmitError::invalid_literal(
                source,
                span,
                "integer literal",
                "超出目标整数类型可表示范围",
                text,
            )
        })?;
        Ok(bits as u64)
    }

    fn negated_int_literal_bits_for_ty(
        &self,
        span: crate::span::Span,
        literal_span: crate::span::Span,
        int_ty: IntTy,
    ) -> Result<u64, LlvmEmitError> {
        let source = self.current_source()?;
        let text = self.current_source_slice(span)?;
        let raw = self.parse_current_int_literal(literal_span)?;
        let bits = checked_negated_int_literal_bits(raw, int_ty).ok_or_else(|| {
            LlvmEmitError::invalid_literal(
                source,
                span,
                "integer literal",
                "超出目标整数类型可表示范围",
                text,
            )
        })?;
        Ok(bits as u64)
    }

    fn int_literal_bits_from_source_span_if_present(
        &self,
        span: crate::span::Span,
        int_ty: IntTy,
    ) -> Result<Option<u64>, LlvmEmitError> {
        let Ok(text) = self.current_source_slice(span) else {
            return Ok(None);
        };
        let source = self.current_source()?;
        let Some((negative, body)) = source_text_int_literal_body(text) else {
            return Ok(None);
        };
        let raw = parse_int_literal_checked(body).map_err(|err| {
            LlvmEmitError::invalid_literal(source, span, "integer literal", err.reason(), text)
        })?;
        let bits = if negative {
            checked_negated_int_literal_bits(raw, int_ty)
        } else {
            checked_positive_int_literal_bits(raw, int_ty)
        }
        .ok_or_else(|| {
            LlvmEmitError::invalid_literal(
                source,
                span,
                "integer literal",
                "超出目标整数类型可表示范围",
                text,
            )
        })?;
        Ok(Some(bits as u64))
    }

    fn parse_current_string_literal_bytes(
        &self,
        span: crate::span::Span,
    ) -> Result<Vec<u8>, LlvmEmitError> {
        let text = self.current_source_slice(span)?;
        let source = self.current_source()?;
        parse_string_literal_bytes(text).map_err(|err| {
            LlvmEmitError::invalid_literal(
                source,
                span,
                "string literal",
                string_literal_parse_reason(err),
                text,
            )
        })
    }

    /// 获取 effect operation 的稳定 op_tag（T1608）。
    ///
    /// 规则：
    /// - `scoop.core.Raise.raise` → 1（固定；与 runtime 约定兼容）。
    /// - 其余 effect op：首次出现时分配递增编号（从 2 开始），后续查表复用。
    /// - 同一编译单元内 tag 稳定（相同 FQN 总是得到相同 tag）。
    pub(super) fn effect_op_tag(&mut self, fqn: &str) -> u32 {
        let mut state = self.effect_op_tags.borrow_mut();
        if let Some(&tag) = state.map.get(fqn) {
            return tag;
        }
        let tag = state.next;
        state.next = state.next.saturating_add(1);
        state.map.insert(fqn.to_string(), tag);
        tag
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

        let has_body = fun.body.is_some() && !self.extern_funs.contains_key(&fun.fqn);

        // LLVM's gc.result cannot lower aggregate types (struct/tuple/enum) that span
        // multiple physical registers — causes "Cannot emit physreg copy instruction".
        // For functions returning GC-free aggregates, we mark them as `gc-leaf-function`
        // so that `rewrite-statepoints-for-gc` skips statepoint wrapping at call sites,
        // avoiding the aggregate gc.result issue.
        //
        // Safety: GC-free aggregate constructors (e.g. IntProgression.rangeTo/downTo)
        // don't trigger GC, so treating them as leaf is correct. For more complex
        // functions that return GC-free aggregates but perform internal GC operations,
        // the proper fix is to convert aggregate returns to sret (TODO: future task).
        let returns_gc_free_aggregate = self.returns_gc_free_aggregate(fun.return_ty);

        if let Some(existing) = self.module.get_function(llvm_name) {
            if has_body {
                existing.set_gc(super::LLVM_GC_STRATEGY_STATEPOINT_EXAMPLE);
            }
            if returns_gc_free_aggregate {
                let attr = self.context.create_string_attribute("gc-leaf-function", "");
                existing.add_attribute(inkwell::attributes::AttributeLoc::Function, attr);
            }
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
            CgTy::Unit | CgTy::Never => self.context.void_type().fn_type(&llvm_params, false),
            other => self
                .llvm_basic_type_of(fun.span, other)?
                .fn_type(&llvm_params, false),
        };

        let linkage = if self.extern_funs.contains_key(&fun.fqn) {
            Some(Linkage::External)
        } else {
            None
        };
        let llvm_fun = self.module.add_function(llvm_name, fn_ty, linkage);
        // `@CallingConvention(...)`：缺省为 C ABI（LLVM callconv 0）。
        llvm_fun.set_call_conventions(self.llvm_call_convention_for_fqn(&fun.fqn));
        if has_body {
            llvm_fun.set_gc(super::LLVM_GC_STRATEGY_STATEPOINT_EXAMPLE);
        }
        if returns_gc_free_aggregate {
            let attr = self.context.create_string_attribute("gc-leaf-function", "");
            llvm_fun.add_attribute(inkwell::attributes::AttributeLoc::Function, attr);
        }
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

    fn llvm_type_needs_sret(ty: BasicTypeEnum<'ctx>) -> bool {
        matches!(
            ty,
            BasicTypeEnum::StructType(_)
                | BasicTypeEnum::ArrayType(_)
                | BasicTypeEnum::VectorType(_)
                | BasicTypeEnum::ScalableVectorType(_)
        )
    }

    fn hidden_sret_result_ty(
        &mut self,
        at: crate::span::Span,
        ret_cg: CgTy,
    ) -> Result<Option<BasicTypeEnum<'ctx>>, LlvmEmitError> {
        let llvm_ret_ty = self.llvm_basic_type_of(at, ret_cg)?;
        Ok(Self::llvm_type_needs_sret(llvm_ret_ty).then_some(llvm_ret_ty))
    }

    fn sret_type_attribute(&self, result_ty: BasicTypeEnum<'ctx>) -> Attribute {
        let kind_id = Attribute::get_named_enum_kind_id("sret");
        self.context
            .create_type_attribute(kind_id, result_ty.as_any_type_enum())
    }

    fn add_sret_attribute_to_function(
        &self,
        llvm_fun: FunctionValue<'ctx>,
        param_index: u32,
        result_ty: BasicTypeEnum<'ctx>,
    ) {
        llvm_fun.add_attribute(
            AttributeLoc::Param(param_index),
            self.sret_type_attribute(result_ty),
        );
    }

    fn add_sret_attribute_to_call(
        &self,
        call_site: CallSiteValue<'ctx>,
        param_index: u32,
        result_ty: BasicTypeEnum<'ctx>,
    ) {
        call_site.add_attribute(
            AttributeLoc::Param(param_index),
            self.sret_type_attribute(result_ty),
        );
    }

    fn load_sret_result_from_ptr(
        &mut self,
        at: crate::span::Span,
        ret_cg: CgTy,
        result_ptr: PointerValue<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let llvm_ret_ty = self.llvm_basic_type_of(at, ret_cg)?;
        let loaded = self
            .builder
            .build_load(llvm_ret_ty, result_ptr, "sret_result")?;
        self.cg_value_from_loaded(at, ret_cg, loaded)
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

        let saved_source_id = self.current_source_id;
        self.current_source_id = self.source_id_for_path(v.source_path.as_path(), v.span)?;
        let init = self.const_initializer_for_top_level_var(v, cg_ty, llvm_ty);
        self.current_source_id = saved_source_id;
        let init = init?;
        gv.set_initializer(&init);

        // `@CLayout(aligned = N)`：对显式对齐的值类型，在全局存储上透传 alignment。
        if let CgTy::Struct(struct_ty) = cg_ty
            && let Some(aligned) = self.struct_clayout(struct_ty).and_then(|c| c.aligned)
        {
            gv.set_alignment(aligned);
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
            CgTy::Unit | CgTy::Never => self.context.i8_type().const_int(0, false).into(),
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
                let bits = self.const_eval_int_expr_bits(init, int_ty)?.ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "top-level var initializer (int const)",
                        at: init.span.into(),
                    },
                )?;
                let value = mask_to_bits(bits, int_ty.bits) as u64;
                self.int_type(int_ty).const_int(value, false).into()
            }
            CgTy::Float64 | CgTy::Float32 => self.const_eval_float_expr(init, cg_ty).ok_or(
                LlvmEmitError::UnsupportedMainBody {
                    kind: "top-level var initializer (float const)",
                    at: init.span.into(),
                },
            )?,
            // 早期阶段：仅支持"静态全零初始化"；更复杂的值类型常量构造留给后续任务补齐。
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

    fn const_eval_float_expr(
        &self,
        expr: &hir::Expr,
        target_ty: CgTy,
    ) -> Option<BasicValueEnum<'ctx>> {
        let value = self.const_eval_float_expr_value(expr)?;
        match target_ty {
            CgTy::Float64 => Some(self.context.f64_type().const_float(value).into()),
            CgTy::Float32 => Some(
                self.context
                    .f32_type()
                    .const_float(f64::from(value as f32))
                    .into(),
            ),
            _ => None,
        }
    }

    fn const_eval_float_expr_value(&self, expr: &hir::Expr) -> Option<f64> {
        match &expr.kind {
            hir::ExprKind::Literal(hir::LiteralKind::Float64(value)) => Some(*value),
            hir::ExprKind::Literal(hir::LiteralKind::Float32(value)) => Some(f64::from(*value)),
            hir::ExprKind::Unary {
                op: ast::UnaryOp::Neg,
                expr: inner,
                ..
            } => Some(-self.const_eval_float_expr_value(inner)?),
            _ => None,
        }
    }

    fn const_eval_int_expr_bits(
        &self,
        expr: &hir::Expr,
        int_ty: IntTy,
    ) -> Result<Option<u128>, LlvmEmitError> {
        match &expr.kind {
            hir::ExprKind::Literal(hir::LiteralKind::Int) => Ok(Some(u128::from(
                self.int_literal_bits_for_ty(expr.span, int_ty)?,
            ))),
            hir::ExprKind::Literal(hir::LiteralKind::SynthInt(v)) => {
                Ok(Some(mask_to_bits(*v as u128, int_ty.bits)))
            }
            hir::ExprKind::Unary {
                op: ast::UnaryOp::Neg,
                expr: inner,
                ..
            } if matches!(inner.kind, hir::ExprKind::Literal(hir::LiteralKind::Int)) => Ok(Some(
                u128::from(self.negated_int_literal_bits_for_ty(expr.span, inner.span, int_ty)?),
            )),
            hir::ExprKind::Unary {
                op: ast::UnaryOp::Neg,
                expr: inner,
                ..
            } => Ok(self
                .const_eval_int_expr_bits(inner, int_ty)?
                .map(|v| mask_to_bits(0u128.wrapping_sub(v), int_ty.bits))),
            hir::ExprKind::Unary {
                op: ast::UnaryOp::BitNot,
                expr: inner,
                ..
            } => Ok(self
                .const_eval_int_expr_bits(inner, int_ty)?
                .map(|v| mask_to_bits(!v, int_ty.bits))),
            _ => Ok(None),
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

    fn codegen_initializer_expr(
        &mut self,
        expr: &hir::Expr,
        target_ty: CgTy,
        target_hir_ty: TypeId,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match &expr.kind {
            hir::ExprKind::Closure(closure) => {
                self.codegen_closure_expr(expr.span, closure, target_hir_ty)
            }
            // 对 call initializer 传入声明类型，避免泛型 ctor 等路径因为 HIR `expr.ty = Any`
            // 丢失结果类型信息。
            hir::ExprKind::Call { callee, args } => self.codegen_call(
                expr.span,
                callee,
                args,
                Some(target_ty),
                Some(target_hir_ty),
            ),
            _ => self.codegen_expr_in_expected_context(expr, Some(target_ty)),
        }
    }

    fn codegen_top_level_const_ref(
        &mut self,
        span: crate::span::Span,
        top_level_const: &hir::TopLevelConst,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if self
            .top_level_const_eval_stack
            .iter()
            .any(|current| current == &top_level_const.fqn)
        {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "recursive top-level const ref",
                at: span.into(),
            });
        }

        let init = top_level_const
            .init
            .as_ref()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "top-level const without initializer",
                at: top_level_const.span.into(),
            })?;
        let target_ty =
            self.cg_ty_of(top_level_const.ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "top-level const type",
                    at: top_level_const.span.into(),
                })?;
        let source_id = self.source_id_for_path(top_level_const.source_path.as_path(), span)?;
        let saved_source_id = self.current_source_id;
        self.current_source_id = source_id;
        self.top_level_const_eval_stack
            .push(top_level_const.fqn.clone());
        let result = self.codegen_initializer_expr(init, target_ty, top_level_const.ty);
        self.top_level_const_eval_stack.pop();
        self.current_source_id = saved_source_id;
        result
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

        self.current_source_id = self.source_id_for_path(fun.source_path.as_path(), fun.span)?;

        let entry = self.context.append_basic_block(llvm_fun, "entry");
        self.builder.position_at_end(entry);

        self.env.push_scope();
        self.codegen_fun_params(fun, llvm_fun)?;

        let declared_return_cg =
            self.cg_ty_of(fun.return_ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "function return type",
                    at: fun.span.into(),
                })?;
        self.current_fun_return_ty = Some(declared_return_cg);

        // T0141: Set up function-level return context for early return support.
        // Alloca is placed here (still in the entry block before body codegen).
        let return_bb = self.context.append_basic_block(llvm_fun, "return");
        let return_alloca = match declared_return_cg {
            CgTy::Unit | CgTy::Never => None,
            _ => Some(self.builder.build_alloca(
                self.llvm_basic_type_of(fun.span, declared_return_cg)?,
                "return_val",
            )?),
        };
        self.return_context = Some(ReturnContext {
            return_bb,
            return_alloca,
        });

        let ret_v = self.codegen_block_as_return_value(body, declared_return_cg)?;

        // Normal path: store return value and branch to return_bb.
        if self
            .builder
            .get_insert_block()
            .is_some_and(|bb| bb.get_terminator().is_none())
        {
            if let Some(alloca) = return_alloca
                && let Some(raw) = ret_v.value
            {
                self.builder.build_store(alloca, raw)?;
            }
            self.builder.build_unconditional_branch(return_bb)?;
        }

        // Emit the return block.
        self.builder.position_at_end(return_bb);
        match declared_return_cg {
            CgTy::Unit => {
                self.builder.build_return(None)?;
            }
            CgTy::Never => {
                self.builder.build_unreachable()?;
            }
            _ => {
                if let Some(alloca) = return_alloca {
                    let loaded = self.builder.build_load(
                        self.llvm_basic_type_of(fun.span, declared_return_cg)?,
                        alloca,
                        "ret_load",
                    )?;
                    self.builder.build_return(Some(&loaded))?;
                } else {
                    self.builder.build_return(None)?;
                }
            }
        }

        self.return_context = None;
        self.env.pop_scope();
        Ok(())
    }

    pub(crate) fn codegen_main_exit_code(
        mut self,
        fun: &hir::FunDecl,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        // 入口 `i32 @main()` 的返回类型固定为 i32；这里记录下来以便最小 Raise 传播时能"早退"。
        self.current_fun_return_ty = Some(CgTy::Int(IntTy {
            bits: 32,
            signed: true,
        }));

        self.env.push_scope();

        let exit = match fun.body.as_ref() {
            Some(body) => self.codegen_block_as_exit_code(body, fun.return_ty)?,
            None => self.context.i32_type().const_int(0, false),
        };

        self.env.pop_scope();
        Ok(exit)
    }

    // 表达式/语句/控制流 codegen 已拆分到子模块（T0102d）。

    fn codegen_call(
        &mut self,
        span: crate::span::Span,
        callee: &hir::Expr,
        args: &[hir::CallArg],
        expected: Option<CgTy>,
        // T0125：call expression 的结果 TypeId（用于泛型 class ctor 的 mangled FQN 查找）。
        result_ty: Option<TypeId>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
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
                .ok_or_else(|| LlvmEmitError::UnsupportedMainBody {
                    kind: "unknown local value",
                    at: callee.span.into(),
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
                if let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = self.types.kind(hir_ty)
                    && nominal.fqn == "scoop.unsafe.FunPtr"
                {
                    let sig_ty = nominal.args.first().copied().ok_or(
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "funptr signature type",
                            at: callee.span.into(),
                        },
                    )?;
                    let TypeKind::Ref(RefTypeKind::Function(fun_ty)) = self.types.kind(sig_ty)
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

            // T1508b：class virtual dispatch（vtable）端到端。
            //
            // 说明：
            // - HIR lowering（T1508a）会把 `receiver.method(args...)` 改写为顶层调用
            //   `Owner.method(receiver, args...)`；
            // - 对于 `open/abstract/override` 成员，该调用必须走 vtable slot 以实现动态分发；
            // - slot 布局来自 HIR side table `class_vtables`（由前端从 compilation unit AST 计算）。
            if let Some(v) = self.try_codegen_class_vtable_call(span, callee.span, fqn, args)? {
                return Ok(v);
            }

            // T0131：where-bound `ToString.toString` 拦截（monomorphized body 内的 `value.toString()`）。
            //
            // 说明：
            // - 泛型 `print<T> where T: ToString` 的 monomorphized body 调用 `value.toString()`；
            // - where-bound 分发使 HIR 将其改写为 `scoop.core.ToString.toString(value)`；
            // - 对内建类型（Int/Bool/String）直接调用 runtime；对用户类型 fall-through 到 itable。
            if fqn == "scoop.core.ToString.toString"
                && let Some(v) = self.try_codegen_tostring_iface_builtin(span, callee.span, args)?
            {
                return Ok(v);
            }

            // T1508c：interface dispatch（itable）端到端。
            //
            // 说明：
            // - HIR lowering 会把 `iface.method(args...)` 改写为顶层调用 `IFace.method(iface, args...)`；
            // - codegen 阶段根据 interface slot 表选择 slot，并通过 `this.header.type_desc.itable` 查找目标；
            // - itable entry 的布局由 lowering side table `interfaces/class_itables` 提供。
            if let Some(v) = self.try_codegen_interface_itable_call(span, callee.span, fqn, args)? {
                return Ok(v);
            }
            // TODO T0910：GC v0（mark-sweep，测试辅助）。
            if let Some(v) = self.try_codegen_sysroot_gc_debug_intrinsics(span, fqn, args)? {
                return Ok(v);
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

            // T0131：`__scoop_print_string` / `__scoop_println_string`（内部 runtime 映射）。
            //
            // 说明：
            // - 泛型 `print<T>` / `println<T>` 的 monomorphized body 调用这些内部函数；
            // - 它们接受一个 String 参数并映射到 runtime 的 `scoop_print` / `scoop_println`。
            if fqn == "scoop.core.__scoop_print_string"
                || fqn == "scoop.core.__scoop_println_string"
            {
                return self.codegen_sysroot_internal_print_string(span, callee.span, fqn, args);
            }

            // T0131：body-less extension function `toString()`（Int/Bool/String）。
            //
            // 说明：
            // - `fun Int.toString(): String` 等 body-less extension function 在 HIR lowering
            //   阶段被改写为 top-level call `scoop.core.toString(receiver)`；
            // - codegen 按 receiver CgTy 分发到对应 runtime 函数。
            if fqn == "scoop.core.toString" {
                return self.codegen_sysroot_to_string_ext(span, callee.span, args);
            }

            // T0146c2/T0147c：body-less extension function `toInt()`（Char / Float / String）。
            if fqn == "scoop.core.toInt" {
                return self.codegen_sysroot_to_int_ext(span, callee.span, args);
            }

            // T0146c2/T0147c：body-less extension function `hash()`（Char / Float / String / Int）。
            if fqn == "scoop.core.hash" {
                return self.codegen_sysroot_hash_ext(span, callee.span, args);
            }

            // T0147c: Float body-less extension functions.
            if fqn == "scoop.core.abs"
                && matches!(
                    args.first(),
                    Some(hir::CallArg::Positional(expr))
                        if matches!(
                            self.resolve_expr_cg_ty(expr),
                            Some(CgTy::Float64 | CgTy::Float32)
                        )
                )
            {
                return self.codegen_sysroot_abs_ext(span, callee.span, args);
            }
            if fqn == "scoop.core.isNaN"
                && matches!(
                    args.first(),
                    Some(hir::CallArg::Positional(expr))
                        if matches!(
                            self.resolve_expr_cg_ty(expr),
                            Some(CgTy::Float64 | CgTy::Float32)
                        )
                )
            {
                return self.codegen_sysroot_is_nan_ext(span, callee.span, args);
            }
            if fqn == "scoop.core.isInfinite"
                && matches!(
                    args.first(),
                    Some(hir::CallArg::Positional(expr))
                        if matches!(
                            self.resolve_expr_cg_ty(expr),
                            Some(CgTy::Float64 | CgTy::Float32)
                        )
                )
            {
                return self.codegen_sysroot_is_infinite_ext(span, callee.span, args);
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
                return self.codegen_sysroot_task_executor_debug_pending_count(
                    span,
                    callee.span,
                    args,
                );
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
                return self.codegen_sysroot_array_intrinsics(
                    span,
                    callee.span,
                    fqn,
                    args,
                    expected,
                );
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
                // spec §15.10.1：stable GC handles。
                if fqn == "scoop.core.GC.handleNew" {
                    return self.codegen_sysroot_gc_handle_new(span, member.span, args, expected);
                }
                if fqn == "scoop.core.GC.handleGet" {
                    return self.codegen_sysroot_gc_handle_get(span, member.span, args);
                }
                if fqn == "scoop.core.GC.handleDrop" {
                    return self.codegen_sysroot_gc_handle_drop(span, member.span, args);
                }

                if fqn == "scoop.core.GC.pin" {
                    return self.codegen_sysroot_gc_pin(span, member.span, args, expected);
                }
                if fqn == "scoop.core.GC.unpin" {
                    return self.codegen_sysroot_gc_unpin(span, member.span, args);
                }
            }

            if member.name == "trimIndent" {
                return self.codegen_string_trim_indent(span, receiver, args);
            }
            // T1812 / T0146c2 / T0147c: builtin `toInt()` —— Char / Float / String.
            if member.name == "toInt" {
                let recv_ty = match &receiver.kind {
                    hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) => self
                        .env
                        .get(*id)
                        .and_then(|l| l.hir_ty)
                        .unwrap_or(receiver.ty),
                    _ => receiver.ty,
                };
                if matches!(
                    self.types.kind(recv_ty),
                    TypeKind::Value(ValueTypeKind::Char)
                ) {
                    return self.codegen_char_method_to_int(receiver);
                }
                if matches!(
                    self.types.kind(recv_ty),
                    TypeKind::Value(ValueTypeKind::Float64 | ValueTypeKind::Float32)
                ) {
                    let recv = self.codegen_expr(receiver)?;
                    return self.codegen_float_to_int_value(span, receiver.span, recv);
                }
                return self.codegen_string_method(span, receiver, &member.name, args);
            }
            // T1811: String P0 methods + T0115 extended methods.
            // T0122: substring/indexOf/contains/startsWith/endsWith/split/trim/trimStart/trimEnd
            // 已迁移到 sysroot/string.scoop 的纯 Scoop 扩展函数。
            if matches!(
                member.name.as_str(),
                "length"
                    | "concat"
                    | "isEmpty"
                    | "replace"
                    | "charAt"
                    | "repeat"
                    | "compareTo"
                    | "byteLength"
                    | "getByte"
                    | "unsafeSliceBytes"
            ) {
                return self.codegen_string_method(span, receiver, &member.name, args);
            }
            // T0146c2/T1812/T0114: Char/Int/Bool.toString() — route by builtin receiver type.
            //
            // Evaluate the receiver first, then dispatch based on CgTy. This correctly
            // handles both typed variables and expression results (e.g., comparison ops
            // like `(x == y).toString()` where the HIR type might differ from CgTy).
            if member.name == "toString" {
                return self.codegen_to_string_method(span, receiver);
            }
            // T0146c2/T1817: hash() — route to Char/Int/String based on receiver type.
            if member.name == "hash" {
                let recv_ty = match &receiver.kind {
                    hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) => self
                        .env
                        .get(*id)
                        .and_then(|l| l.hir_ty)
                        .unwrap_or(receiver.ty),
                    _ => receiver.ty,
                };
                match self.types.kind(recv_ty) {
                    TypeKind::Value(ValueTypeKind::Char) => {
                        return self.codegen_char_method_hash(span, receiver);
                    }
                    TypeKind::Value(ValueTypeKind::Int) => {
                        return self.codegen_int_method_hash(span, receiver);
                    }
                    TypeKind::Value(ValueTypeKind::Float64 | ValueTypeKind::Float32) => {
                        let recv = self.codegen_expr(receiver)?;
                        return self.codegen_float_hash_value(receiver.span, recv);
                    }
                    _ => {
                        return self.codegen_string_method(span, receiver, "hash", args);
                    }
                }
            }
            if matches!(member.name.as_str(), "abs" | "isNaN" | "isInfinite") {
                let recv = self.codegen_expr(receiver)?;
                return match member.name.as_str() {
                    "abs" => self.codegen_float_abs_value(receiver.span, recv),
                    "isNaN" => self.codegen_float_is_nan_value(receiver.span, recv),
                    "isInfinite" => self.codegen_float_is_infinite_value(receiver.span, recv),
                    _ => unreachable!("filtered by matches!"),
                };
            }
        }

        // 2) enum variant ctor：`Some(x)` 这类调用在 resolver 阶段不会 resolve，
        //    需要依赖"期望类型语境"才能决定属于哪个 enum。
        if let hir::ExprKind::UnresolvedIdent { name } = &callee.kind {
            // T1312：class ctor call —— resolver 在 call-site 写回 ctor candidates，
            // HIR v0 仍把 callee 降为 `UnresolvedIdent`，因此这里需要通过 side table 判断并执行 ctor。
            if let Some(candidates) = self.ctor_call_sites.get(&callee.span) {
                return self.codegen_class_ctor_call(
                    span,
                    callee.span,
                    name,
                    args,
                    candidates,
                    result_ty,
                );
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

        let sig_ty = nominal
            .args
            .first()
            .copied()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
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

        self.codegen_funptr_value_call(span, callee_span, loaded, int_ty, fun_ty, call_args)
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
    /// - ctor 选择规则：按"参数个数"在已收集 ctor 集合中匹配；若不唯一则报错；
    /// - class 单继承初始化链：会从最基类到派生类逐层执行 init steps；
    /// - super ctor args 与 secondary ctor delegation args 同样只支持位置参数，并按源码顺序求值。
    fn codegen_class_ctor_call(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        _name: &str,
        args: &[hir::CallArg],
        candidates: &[String],
        result_ty: Option<TypeId>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        // 1) 选择唯一可用的 class candidate（HIR side table 里必须存在该 class 的 init 信息）。
        //
        // T0125：对于泛型 class，优先使用 mangled FQN 查找（如 "pkg.Box<Int>"）。
        // 通过 result_ty 的 Nominal 类型参数构造 mangled key。
        let mut class_candidates: Vec<&String> = candidates
            .iter()
            .filter(|fqn| self.class_inits.contains_key(*fqn))
            .collect();
        class_candidates.sort();
        class_candidates.dedup();

        let class_fqn = match class_candidates.as_slice() {
            [one] => {
                let base_fqn = (*one).clone();
                // T0125：如果 result_ty 提供了类型参数，尝试使用 mangled FQN 查找。
                if let Some(rty) = result_ty {
                    if let TypeKind::Ref(RefTypeKind::Nominal(nominal)) = self.types.kind(rty) {
                        if !nominal.args.is_empty() {
                            let mangled = self.nominal_layout_key(nominal);
                            if self.class_inits.contains_key(&mangled) {
                                mangled
                            } else {
                                base_fqn
                            }
                        } else {
                            base_fqn
                        }
                    } else {
                        base_fqn
                    }
                } else {
                    base_fqn
                }
            }
            [] => {
                // T0125：base FQN 不在 class_inits 中，但可能仅有 mangled 版本（pure generic class）。
                // 通过 result_ty 查找。
                if let Some(rty) = result_ty {
                    if let TypeKind::Ref(RefTypeKind::Nominal(nominal)) = self.types.kind(rty) {
                        if !nominal.args.is_empty() {
                            let mangled = self.nominal_layout_key(nominal);
                            if self.class_inits.contains_key(&mangled) {
                                mangled
                            } else {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "class ctor call candidate class",
                                    at: callee_span.into(),
                                });
                            }
                        } else {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "class ctor call candidate class",
                                at: callee_span.into(),
                            });
                        }
                    } else {
                        return Err(LlvmEmitError::UnsupportedMainBody {
                            kind: "class ctor call candidate class",
                            at: callee_span.into(),
                        });
                    }
                } else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "class ctor call candidate class",
                        at: callee_span.into(),
                    });
                }
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

        let size_v = self.context.i64_type().const_int(obj_size_bytes, false);

        // 分配点统一走 typed alloc：在 runtime 内部写入对象头 `type_desc`。
        let type_desc = self.get_or_create_class_type_desc_global(span, &class_fqn)?;
        let type_desc_i8 = self.builder.build_pointer_cast(
            type_desc.as_pointer_value(),
            self.llvm_i8_ptr_type(),
            "class_type_desc_i8",
        )?;
        let rt_alloc = self.declare_runtime_alloc_typed();
        let call = self.builder.build_call(
            rt_alloc,
            &[type_desc_i8.into(), size_v.into()],
            "rt_alloc_class",
        )?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc_typed return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(obj_ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc_typed return type",
                at: span.into(),
            });
        };

        let obj_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
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
            let size_ty = self.llvm_ptr_sized_int_type(None);
            let size_v = size_ty.const_int(payload_size_bytes, false);
            let zero = self.context.i8_type().const_int(0, false);
            let _ = self.builder.build_memset(payload_i8, 1, zero, size_v)?;
        }

        // 6) 执行构造调用：支持 super ctor args + secondary ctor delegation（T1327c）。
        //
        // 语义（Kotlin-like，Appendix B.2.2）：
        // - 调用点先按源码顺序求值 ctor 实参；
        // - 进入 ctor 后：
        //   - 若是 `: this(...)`，先执行被委托 ctor，再执行当前 ctor body；
        //   - 否则先执行 super ctor call，再执行本类的参数属性赋值、property initializer、init blocks，
        //     最后执行 secondary ctor body（若有）。

        // 调用点实参求值（按源码顺序），供"被调用的 ctor"注入 params locals。
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
                Err(LlvmEmitError::UnsupportedMainBody {
                    kind,
                    at: at.into(),
                })
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
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind,
                at: at.into(),
            });
        }
        if matching.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind,
                at: at.into(),
            });
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
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind,
                at: at.into(),
            });
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

    #[allow(clippy::too_many_arguments)]
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
                    let field_ptr =
                        self.codegen_class_field_ptr(init.span, class, obj_ptr, field_idx)?;
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
        self.codegen_class_ctor_invoke_inner(
            span,
            callee_span,
            class,
            ctor,
            args,
            obj_ptr,
            &mut stack,
        )
    }

    #[allow(clippy::too_many_arguments)]
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
            None => (
                hir::ClassCtorKind::Primary,
                callee_span,
                &[][..],
                None,
                None,
            ),
        };

        let key = (class.fqn.clone(), ctor_span);
        if !stack.insert(key.clone()) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "class ctor delegation cycle",
                at: callee_span.into(),
            });
        }

        let saved_source_id = self.current_source_id;
        self.current_source_id =
            self.source_id_for_path(class.source_path.as_path(), callee_span)?;
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
                let param_cg =
                    self.cg_ty_of(param.ty)
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "class ctor param type",
                            at: callee_span.into(),
                        })?;
                let param_ptr = self.create_entry_alloca(param.decl_span, &param.name, param_cg)?;
                let stored =
                    self.store_local_value(param.decl_span, param_ptr, param_cg, *arg_v)?;
                stored_args.push(stored);
                self.env.insert(
                    param.id,
                    CgLocal {
                        hir_ty: Some(param.ty),
                        ty: param_cg,
                        ptr: param_ptr,
                        mutable: false,
                    },
                );
            }

            // secondary ctor delegation（T1327c）
            if ctor_kind == hir::ClassCtorKind::Secondary
                && let Some(deleg) = delegation
            {
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

            if ctor_kind == hir::ClassCtorKind::Secondary
                && let Some(body) = ctor_body
            {
                let _ = self.codegen_block_value(body)?;
            }

            self.env.pop_scope();
            Ok(())
        })();

        self.current_source_id = saved_source_id;
        stack.remove(&key);
        result
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

    /// T1811: codegen for String P0 methods (length/substring/startsWith/endsWith/indexOf/contains/split).
    fn codegen_string_method(
        &mut self,
        span: crate::span::Span,
        receiver: &hir::Expr,
        method_name: &str,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        // Evaluate receiver as String pointer.
        let recv = self.codegen_expr_in_expected_context(receiver, Some(CgTy::String))?;
        let coerced = self.coerce_value(receiver.span, recv, CgTy::String)?;
        let Some(raw) = coerced.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "String method receiver value",
                at: receiver.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(recv_ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "String method receiver type",
                at: receiver.span.into(),
            });
        };

        match method_name {
            "length" => {
                // scoop_string_length(s) -> i64
                let rt_fun = self.declare_runtime_string_length();
                let call =
                    self.builder
                        .build_call(rt_fun, &[recv_ptr.into()], "rt_string_length")?;
                let ret = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "String.length return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(iv) = ret else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.length return type",
                        at: span.into(),
                    });
                };
                Ok(CgValue::int(
                    iv,
                    IntTy {
                        bits: 64,
                        signed: true,
                    },
                ))
            }
            // T0122/T0143: substring/indexOf/contains/startsWith/endsWith/split 已迁移到 sysroot/string.scoop
            "toInt" => {
                // scoop_string_to_int(s) -> i64
                let rt_fun = self.declare_runtime_string_to_int();
                let call =
                    self.builder
                        .build_call(rt_fun, &[recv_ptr.into()], "rt_string_to_int")?;
                let ret = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "String.toInt return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(iv) = ret else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.toInt return type",
                        at: span.into(),
                    });
                };
                Ok(CgValue::int(
                    iv,
                    IntTy {
                        bits: 64,
                        signed: true,
                    },
                ))
            }
            "concat" => {
                // scoop_string_concat(a, b) -> ScoopString*
                if args.len() != 1 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.concat arity mismatch",
                        at: span.into(),
                    });
                }
                let hir::CallArg::Positional(other_expr) = &args[0] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.concat named arg",
                        at: span.into(),
                    });
                };
                let other =
                    self.codegen_expr_in_expected_context(other_expr, Some(CgTy::String))?;
                let other_coerced = self.coerce_value(other_expr.span, other, CgTy::String)?;
                let Some(other_raw) = other_coerced.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.concat arg value",
                        at: span.into(),
                    });
                };
                let BasicValueEnum::PointerValue(other_ptr) = other_raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.concat arg type",
                        at: span.into(),
                    });
                };
                let rt_fun = self.declare_runtime_string_concat();
                let call = self.builder.build_call(
                    rt_fun,
                    &[recv_ptr.into(), other_ptr.into()],
                    "rt_string_concat",
                )?;
                let ret = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "String.concat return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::PointerValue(result_ptr) = ret else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.concat return type",
                        at: span.into(),
                    });
                };
                Ok(CgValue {
                    ty: CgTy::String,
                    value: Some(result_ptr.into()),
                })
            }
            // T1817: String.hash() — FNV-1a via C runtime.
            "hash" => {
                let rt_fun = self.declare_runtime_string_hash();
                let call = self
                    .builder
                    .build_call(rt_fun, &[recv_ptr.into()], "rt_string_hash")?;
                let ret = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "String.hash return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(iv) = ret else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.hash return type",
                        at: span.into(),
                    });
                };
                Ok(CgValue::int(
                    iv,
                    IntTy {
                        bits: 64,
                        signed: true,
                    },
                ))
            }
            // T0122/T0143: trim/trimStart/trimEnd 已迁移到 sysroot/string.scoop
            // T0115: String.isEmpty() — 0 args → Bool (i64 0/1 → i1)
            "isEmpty" => {
                let rt_fun = self.declare_runtime_string_is_empty();
                let call =
                    self.builder
                        .build_call(rt_fun, &[recv_ptr.into()], "rt_string_isEmpty")?;
                let ret = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "String.isEmpty return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(iv) = ret else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.isEmpty return type",
                        at: span.into(),
                    });
                };
                let bool_val = self.builder.build_int_compare(
                    inkwell::IntPredicate::NE,
                    iv,
                    self.context.i64_type().const_zero(),
                    "to_bool",
                )?;
                Ok(CgValue::bool(bool_val))
            }
            // T0115: String.replace(old, new) — 2 String args → String
            "replace" => {
                if args.len() != 2 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.replace arity mismatch",
                        at: span.into(),
                    });
                }
                let hir::CallArg::Positional(old_expr) = &args[0] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.replace named arg",
                        at: span.into(),
                    });
                };
                let hir::CallArg::Positional(new_expr) = &args[1] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.replace named arg",
                        at: span.into(),
                    });
                };
                let old_val =
                    self.codegen_expr_in_expected_context(old_expr, Some(CgTy::String))?;
                let old_coerced = self.coerce_value(old_expr.span, old_val, CgTy::String)?;
                let Some(old_raw) = old_coerced.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.replace old value",
                        at: span.into(),
                    });
                };
                let BasicValueEnum::PointerValue(old_ptr) = old_raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.replace old type",
                        at: span.into(),
                    });
                };
                let new_val =
                    self.codegen_expr_in_expected_context(new_expr, Some(CgTy::String))?;
                let new_coerced = self.coerce_value(new_expr.span, new_val, CgTy::String)?;
                let Some(new_raw) = new_coerced.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.replace new value",
                        at: span.into(),
                    });
                };
                let BasicValueEnum::PointerValue(new_ptr) = new_raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.replace new type",
                        at: span.into(),
                    });
                };
                let rt_fun = self.declare_runtime_string_replace();
                let call = self.builder.build_call(
                    rt_fun,
                    &[recv_ptr.into(), old_ptr.into(), new_ptr.into()],
                    "rt_string_replace",
                )?;
                let ret = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "String.replace return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::PointerValue(out_ptr) = ret else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.replace return type",
                        at: span.into(),
                    });
                };
                Ok(CgValue {
                    ty: CgTy::String,
                    value: Some(out_ptr.into()),
                })
            }
            // T0115: String.charAt(index) — 1 Int arg → Int
            "charAt" => {
                if args.len() != 1 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.charAt arity mismatch",
                        at: span.into(),
                    });
                }
                let hir::CallArg::Positional(idx_expr) = &args[0] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.charAt named arg",
                        at: span.into(),
                    });
                };
                let idx = self.codegen_expr_in_expected_context(
                    idx_expr,
                    Some(CgTy::Int(IntTy {
                        bits: 64,
                        signed: true,
                    })),
                )?;
                let idx_val = idx.value.ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "String.charAt index value",
                    at: span.into(),
                })?;
                let rt_fun = self.declare_runtime_string_char_at();
                let call = self.builder.build_call(
                    rt_fun,
                    &[recv_ptr.into(), idx_val.into()],
                    "rt_string_charAt",
                )?;
                let ret = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "String.charAt return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(iv) = ret else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.charAt return type",
                        at: span.into(),
                    });
                };
                Ok(CgValue::int(
                    iv,
                    IntTy {
                        bits: 64,
                        signed: true,
                    },
                ))
            }
            // T0115: String.repeat(n) — 1 Int arg → String
            "repeat" => {
                if args.len() != 1 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.repeat arity mismatch",
                        at: span.into(),
                    });
                }
                let hir::CallArg::Positional(n_expr) = &args[0] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.repeat named arg",
                        at: span.into(),
                    });
                };
                let n = self.codegen_expr_in_expected_context(
                    n_expr,
                    Some(CgTy::Int(IntTy {
                        bits: 64,
                        signed: true,
                    })),
                )?;
                let n_val = n.value.ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "String.repeat n value",
                    at: span.into(),
                })?;
                let rt_fun = self.declare_runtime_string_repeat();
                let call = self.builder.build_call(
                    rt_fun,
                    &[recv_ptr.into(), n_val.into()],
                    "rt_string_repeat",
                )?;
                let ret = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "String.repeat return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::PointerValue(out_ptr) = ret else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.repeat return type",
                        at: span.into(),
                    });
                };
                Ok(CgValue {
                    ty: CgTy::String,
                    value: Some(out_ptr.into()),
                })
            }
            // T0115: String.compareTo(other) — 1 String arg → Int
            "compareTo" => {
                if args.len() != 1 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.compareTo arity mismatch",
                        at: span.into(),
                    });
                }
                let hir::CallArg::Positional(other_expr) = &args[0] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.compareTo named arg",
                        at: span.into(),
                    });
                };
                let other =
                    self.codegen_expr_in_expected_context(other_expr, Some(CgTy::String))?;
                let other_coerced = self.coerce_value(other_expr.span, other, CgTy::String)?;
                let Some(other_raw) = other_coerced.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.compareTo arg value",
                        at: span.into(),
                    });
                };
                let BasicValueEnum::PointerValue(other_ptr) = other_raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.compareTo arg type",
                        at: span.into(),
                    });
                };
                let rt_fun = self.declare_runtime_string_compare_to();
                let call = self.builder.build_call(
                    rt_fun,
                    &[recv_ptr.into(), other_ptr.into()],
                    "rt_string_compareTo",
                )?;
                let ret = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "String.compareTo return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::IntValue(iv) = ret else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.compareTo return type",
                        at: span.into(),
                    });
                };
                Ok(CgValue::int(
                    iv,
                    IntTy {
                        bits: 64,
                        signed: true,
                    },
                ))
            }
            // T0120: String.byteLength() — 0 args → Int (inline LLVM IR: read ScoopString.len)
            "byteLength" => {
                if !args.is_empty() {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.byteLength arity mismatch",
                        at: span.into(),
                    });
                }
                let scoop_str_ty = self.llvm_scoop_string_type();
                let len_ptr = self.builder.build_struct_gep(
                    scoop_str_ty,
                    recv_ptr,
                    1,
                    "str_byte_length_gep",
                )?;
                let len_val =
                    self.builder
                        .build_load(self.context.i64_type(), len_ptr, "str_byte_length")?;
                let BasicValueEnum::IntValue(iv) = len_val else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.byteLength load type",
                        at: span.into(),
                    });
                };
                Ok(CgValue::int(
                    iv,
                    IntTy {
                        bits: 64,
                        signed: true,
                    },
                ))
            }
            // T0120: String.getByte(index) — 1 Int arg → Int (inline LLVM IR: bounds-checked byte read)
            "getByte" => {
                if args.len() != 1 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.getByte arity mismatch",
                        at: span.into(),
                    });
                }
                let hir::CallArg::Positional(idx_expr) = &args[0] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.getByte named arg",
                        at: span.into(),
                    });
                };
                let idx = self.codegen_expr_in_expected_context(
                    idx_expr,
                    Some(CgTy::Int(IntTy {
                        bits: 64,
                        signed: true,
                    })),
                )?;
                let idx_val = idx.value.ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "String.getByte index value",
                    at: span.into(),
                })?;
                let BasicValueEnum::IntValue(idx_int) = idx_val else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.getByte index type",
                        at: span.into(),
                    });
                };

                let scoop_str_ty = self.llvm_scoop_string_type();
                let i64_ty = self.context.i64_type();
                let i8_ty = self.context.i8_type();

                // Load string length for bounds check
                let len_ptr =
                    self.builder
                        .build_struct_gep(scoop_str_ty, recv_ptr, 1, "get_byte_len_gep")?;
                let len_val = self
                    .builder
                    .build_load(i64_ty, len_ptr, "get_byte_len")?
                    .into_int_value();

                // Load data pointer
                let data_ptr_ptr = self.builder.build_struct_gep(
                    scoop_str_ty,
                    recv_ptr,
                    2,
                    "get_byte_data_gep",
                )?;
                let data_ptr = self
                    .builder
                    .build_load(self.llvm_i8_ptr_type(), data_ptr_ptr, "get_byte_data")?
                    .into_pointer_value();

                // Bounds check: index < 0 || index >= len → return 0
                let current_fn = self
                    .builder
                    .get_insert_block()
                    .unwrap()
                    .get_parent()
                    .unwrap();
                let in_bounds_bb = self
                    .context
                    .append_basic_block(current_fn, "getByte_in_bounds");
                let out_of_bounds_bb = self
                    .context
                    .append_basic_block(current_fn, "getByte_out_of_bounds");
                let merge_bb = self.context.append_basic_block(current_fn, "getByte_merge");

                // Check index < 0
                let is_negative = self.builder.build_int_compare(
                    inkwell::IntPredicate::SLT,
                    idx_int,
                    i64_ty.const_zero(),
                    "idx_negative",
                )?;
                let not_negative_bb = self
                    .context
                    .append_basic_block(current_fn, "getByte_not_negative");
                self.builder.build_conditional_branch(
                    is_negative,
                    out_of_bounds_bb,
                    not_negative_bb,
                )?;

                // Check index >= len
                self.builder.position_at_end(not_negative_bb);
                let is_ge_len = self.builder.build_int_compare(
                    inkwell::IntPredicate::SGE,
                    idx_int,
                    len_val,
                    "idx_ge_len",
                )?;
                self.builder
                    .build_conditional_branch(is_ge_len, out_of_bounds_bb, in_bounds_bb)?;

                // Out of bounds: return 0
                self.builder.position_at_end(out_of_bounds_bb);
                let zero_val = i64_ty.const_zero();
                self.builder.build_unconditional_branch(merge_bb)?;

                // In bounds: load byte and zero-extend to i64
                self.builder.position_at_end(in_bounds_bb);
                let byte_ptr = unsafe {
                    self.builder.build_in_bounds_gep(
                        i8_ty,
                        data_ptr,
                        &[idx_int],
                        "get_byte_elem_gep",
                    )?
                };
                let byte_val = self
                    .builder
                    .build_load(i8_ty, byte_ptr, "get_byte_val")?
                    .into_int_value();
                let byte_i64 =
                    self.builder
                        .build_int_z_extend(byte_val, i64_ty, "get_byte_zext")?;
                self.builder.build_unconditional_branch(merge_bb)?;

                // Merge: phi node
                self.builder.position_at_end(merge_bb);
                let phi = self.builder.build_phi(i64_ty, "get_byte_result")?;
                phi.add_incoming(&[(&zero_val, out_of_bounds_bb), (&byte_i64, in_bounds_bb)]);
                let result = phi.as_basic_value().into_int_value();
                Ok(CgValue::int(
                    result,
                    IntTy {
                        bits: 64,
                        signed: true,
                    },
                ))
            }
            // T0121: String.unsafeSliceBytes(byteOffset, byteLength) — @Unsafe, 2 Int args → String
            "unsafeSliceBytes" => {
                if args.len() != 2 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.unsafeSliceBytes arity mismatch",
                        at: span.into(),
                    });
                }
                let hir::CallArg::Positional(offset_expr) = &args[0] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.unsafeSliceBytes named arg",
                        at: span.into(),
                    });
                };
                let hir::CallArg::Positional(len_expr) = &args[1] else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.unsafeSliceBytes named arg",
                        at: span.into(),
                    });
                };
                let offset_cg = self.codegen_expr_in_expected_context(
                    offset_expr,
                    Some(CgTy::Int(IntTy {
                        bits: 64,
                        signed: true,
                    })),
                )?;
                let len_cg = self.codegen_expr_in_expected_context(
                    len_expr,
                    Some(CgTy::Int(IntTy {
                        bits: 64,
                        signed: true,
                    })),
                )?;
                let offset_val = offset_cg.value.ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "String.unsafeSliceBytes offset value",
                    at: span.into(),
                })?;
                let len_val = len_cg.value.ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "String.unsafeSliceBytes len value",
                    at: span.into(),
                })?;
                let rt_fun = self.declare_runtime_string_unsafe_slice_bytes();
                let call = self.builder.build_call(
                    rt_fun,
                    &[recv_ptr.into(), offset_val.into(), len_val.into()],
                    "rt_string_unsafe_slice_bytes",
                )?;
                let ret = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "String.unsafeSliceBytes return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::PointerValue(out_ptr) = ret else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "String.unsafeSliceBytes return type",
                        at: span.into(),
                    });
                };
                Ok(CgValue {
                    ty: CgTy::String,
                    value: Some(out_ptr.into()),
                })
            }
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "unknown String method",
                at: span.into(),
            }),
        }
    }

    /// T0146c2 / T0114 / T1812: Unified `toString()` dispatch。
    ///
    /// 先用 HIR concrete type 识别 `Char`（因为运行期与 `Int` 同为 `CgTy::Int`），
    /// 再按 `CgTy` 处理 Bool/Int/String 等其它内建路径。
    fn codegen_to_string_method(
        &mut self,
        span: crate::span::Span,
        receiver: &hir::Expr,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if self.expr_is_builtin_char(receiver) {
            return self.codegen_char_method_to_string(span, receiver);
        }

        // Evaluate the receiver without a forced expected type so we get its
        // natural CgTy.
        let recv = self.codegen_expr(receiver)?;
        match recv.ty {
            CgTy::Bool => {
                // Bool → zero-extend i1 to i64, call scoop_bool_to_string.
                let Some(raw) = recv.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "Bool.toString receiver value",
                        at: receiver.span.into(),
                    });
                };
                let BasicValueEnum::IntValue(iv) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "Bool.toString receiver type",
                        at: receiver.span.into(),
                    });
                };
                let i64_ty = self.context.i64_type();
                let bool_as_i64 = self.builder.build_int_z_extend(iv, i64_ty, "bool_zext")?;
                let rt_fun = self.declare_runtime_bool_to_string();
                let call =
                    self.builder
                        .build_call(rt_fun, &[bool_as_i64.into()], "rt_bool_to_string")?;
                let ret = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "Bool.toString return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::PointerValue(str_ptr) = ret else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "Bool.toString return type",
                        at: span.into(),
                    });
                };
                Ok(CgValue {
                    ty: CgTy::String,
                    value: Some(str_ptr.into()),
                })
            }
            CgTy::Int(_) => {
                // Int → call scoop_int_to_string (i64 already the right width).
                let Some(raw) = recv.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "Int.toString receiver value",
                        at: receiver.span.into(),
                    });
                };
                let BasicValueEnum::IntValue(int_val) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "Int.toString receiver type",
                        at: receiver.span.into(),
                    });
                };
                let rt_fun = self.declare_runtime_int_to_string();
                let call =
                    self.builder
                        .build_call(rt_fun, &[int_val.into()], "rt_int_to_string")?;
                let ret = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "Int.toString return value",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::PointerValue(str_ptr) = ret else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "Int.toString return type",
                        at: span.into(),
                    });
                };
                Ok(CgValue {
                    ty: CgTy::String,
                    value: Some(str_ptr.into()),
                })
            }
            CgTy::Float64 | CgTy::Float32 => {
                self.codegen_float_to_string_value(span, receiver.span, recv)
            }
            CgTy::String => Ok(recv),
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "toString() unsupported receiver CgTy",
                at: span.into(),
            }),
        }
    }

    /// T0146c1: `Char.toInt()` — zero-extend the runtime `i32` codepoint to `Int`.
    fn codegen_char_method_to_int(
        &mut self,
        receiver: &hir::Expr,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let char_ty = CgTy::Int(IntTy {
            bits: 32,
            signed: false,
        });
        let int_ty = CgTy::Int(IntTy {
            bits: self.host.word_bit_width(),
            signed: true,
        });
        let recv = self.codegen_expr_in_expected_context(receiver, Some(char_ty))?;
        self.coerce_value(receiver.span, recv, int_ty)
    }

    /// T0146c2: `Char.toString()` —— 调 runtime 把 Unicode scalar value 编码为 UTF-8 String。
    fn codegen_char_method_to_string(
        &mut self,
        span: crate::span::Span,
        receiver: &hir::Expr,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let char_ty = CgTy::Int(IntTy {
            bits: 32,
            signed: false,
        });
        let recv = self.codegen_expr_in_expected_context(receiver, Some(char_ty))?;
        let Some(raw) = recv.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "Char.toString receiver value",
                at: receiver.span.into(),
            });
        };
        let BasicValueEnum::IntValue(codepoint) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "Char.toString receiver type",
                at: receiver.span.into(),
            });
        };
        let str_ptr = self.codegen_char_to_string_value(span, codepoint)?;
        Ok(CgValue {
            ty: CgTy::String,
            value: Some(str_ptr.into()),
        })
    }

    /// T0146c2: `Char.hash()` —— 以 codepoint zero-extend 到 i64 后复用 `Int.hash()` mixing。
    fn codegen_char_method_hash(
        &mut self,
        _span: crate::span::Span,
        receiver: &hir::Expr,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let char_ty = CgTy::Int(IntTy {
            bits: 32,
            signed: false,
        });
        let recv = self.codegen_expr_in_expected_context(receiver, Some(char_ty))?;
        let Some(raw) = recv.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "Char.hash receiver value",
                at: receiver.span.into(),
            });
        };
        let BasicValueEnum::IntValue(codepoint) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "Char.hash receiver type",
                at: receiver.span.into(),
            });
        };
        let widened = self.builder.build_int_z_extend(
            codepoint,
            self.context.i64_type(),
            "char_hash_zext",
        )?;
        self.codegen_i64_hash_value(widened)
    }

    fn codegen_char_to_string_value(
        &mut self,
        span: crate::span::Span,
        codepoint: IntValue<'ctx>,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let rt_fun = self.declare_runtime_char_to_string();
        let call = self
            .builder
            .build_call(rt_fun, &[codepoint.into()], "rt_char_to_string")?;
        let ret = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "Char.toString return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(str_ptr) = ret else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "Char.toString return type",
                at: span.into(),
            });
        };
        Ok(str_ptr)
    }

    /// T1817: `Int.hash()` — SplitMix64-style bit-mixing (inline LLVM IR).
    ///
    /// Algorithm: x ^= x >> 30; x *= 0xbf58476d1ce4e5b9;
    ///            x ^= x >> 27; x *= 0x94d049bb133111eb;
    ///            x ^= x >> 31;
    fn codegen_int_method_hash(
        &mut self,
        _span: crate::span::Span,
        receiver: &hir::Expr,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let int_cg_ty = CgTy::Int(IntTy {
            bits: 64,
            signed: true,
        });
        let recv = self.codegen_expr_in_expected_context(receiver, Some(int_cg_ty))?;
        let coerced = self.coerce_value(receiver.span, recv, int_cg_ty)?;
        let Some(raw) = coerced.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "Int.hash receiver value",
                at: receiver.span.into(),
            });
        };
        let BasicValueEnum::IntValue(x) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "Int.hash receiver type",
                at: receiver.span.into(),
            });
        };

        self.codegen_i64_hash_value(x)
    }

    fn codegen_i64_hash_value(
        &mut self,
        x: IntValue<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let i64_ty = self.context.i64_type();
        // x ^= x >> 30
        let s1 =
            self.builder
                .build_right_shift(x, i64_ty.const_int(30, false), false, "hash_s1")?;
        let x1 = self.builder.build_xor(x, s1, "hash_x1")?;
        // x *= 0xbf58476d1ce4e5b9
        let c1 = i64_ty.const_int(0xbf58476d1ce4e5b9, false);
        let x2 = self.builder.build_int_mul(x1, c1, "hash_x2")?;
        // x ^= x >> 27
        let s2 =
            self.builder
                .build_right_shift(x2, i64_ty.const_int(27, false), false, "hash_s2")?;
        let x3 = self.builder.build_xor(x2, s2, "hash_x3")?;
        // x *= 0x94d049bb133111eb
        let c2 = i64_ty.const_int(0x94d049bb133111eb, false);
        let x4 = self.builder.build_int_mul(x3, c2, "hash_x4")?;
        // x ^= x >> 31
        let s3 =
            self.builder
                .build_right_shift(x4, i64_ty.const_int(31, false), false, "hash_s3")?;
        let x5 = self.builder.build_xor(x4, s3, "hash_x5")?;

        Ok(CgValue::int(
            x5,
            IntTy {
                bits: 64,
                signed: true,
            },
        ))
    }

    fn unpack_float_cg_value(
        &self,
        recv: CgValue<'ctx>,
        at: crate::span::Span,
        kind: &'static str,
    ) -> Result<(FloatValue<'ctx>, CgTy), LlvmEmitError> {
        if !matches!(recv.ty, CgTy::Float64 | CgTy::Float32) {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind,
                at: at.into(),
            });
        }
        let Some(raw) = recv.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind,
                at: at.into(),
            });
        };
        let BasicValueEnum::FloatValue(float_val) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind,
                at: at.into(),
            });
        };
        Ok((float_val, recv.ty))
    }

    fn codegen_float_to_string_value(
        &mut self,
        span: crate::span::Span,
        receiver_span: crate::span::Span,
        recv: CgValue<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let (float_val, float_ty) =
            self.unpack_float_cg_value(recv, receiver_span, "Float.toString receiver type")?;
        let rt_fun = match float_ty {
            CgTy::Float64 => self.declare_runtime_float64_to_string(),
            CgTy::Float32 => self.declare_runtime_float32_to_string(),
            _ => unreachable!("filtered by unpack_float_cg_value"),
        };
        let call = self
            .builder
            .build_call(rt_fun, &[float_val.into()], "rt_float_to_string")?;
        let ret = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "Float.toString return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(str_ptr) = ret else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "Float.toString return type",
                at: span.into(),
            });
        };
        Ok(CgValue {
            ty: CgTy::String,
            value: Some(str_ptr.into()),
        })
    }

    fn codegen_float_to_int_value(
        &mut self,
        span: crate::span::Span,
        receiver_span: crate::span::Span,
        recv: CgValue<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let (float_val, float_ty) =
            self.unpack_float_cg_value(recv, receiver_span, "Float.toInt receiver type")?;
        let rt_fun = match float_ty {
            CgTy::Float64 => self.declare_runtime_float64_to_int(),
            CgTy::Float32 => self.declare_runtime_float32_to_int(),
            _ => unreachable!("filtered by unpack_float_cg_value"),
        };
        let call = self
            .builder
            .build_call(rt_fun, &[float_val.into()], "rt_float_to_int")?;
        let ret = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "Float.toInt return value",
                at: span.into(),
            })?;
        let BasicValueEnum::IntValue(int64_val) = ret else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "Float.toInt return type",
                at: span.into(),
            });
        };
        let runtime_int = CgValue::int(
            int64_val,
            IntTy {
                bits: 64,
                signed: true,
            },
        );
        self.coerce_value(
            span,
            runtime_int,
            CgTy::Int(IntTy {
                bits: self.host.word_bit_width(),
                signed: true,
            }),
        )
    }

    fn codegen_float_hash_value(
        &mut self,
        receiver_span: crate::span::Span,
        recv: CgValue<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let (float_val, float_ty) =
            self.unpack_float_cg_value(recv, receiver_span, "Float.hash receiver type")?;
        let i64_bits = match float_ty {
            CgTy::Float64 => {
                let raw = self.builder.build_bit_cast(
                    float_val,
                    self.context.i64_type(),
                    "f64_hash_bits",
                )?;
                let BasicValueEnum::IntValue(bits) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "Float64.hash bits type",
                        at: receiver_span.into(),
                    });
                };
                bits
            }
            CgTy::Float32 => {
                let raw = self.builder.build_bit_cast(
                    float_val,
                    self.context.i32_type(),
                    "f32_hash_bits",
                )?;
                let BasicValueEnum::IntValue(bits32) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "Float32.hash bits type",
                        at: receiver_span.into(),
                    });
                };
                self.builder
                    .build_int_z_extend(bits32, self.context.i64_type(), "f32_hash_zext")?
            }
            _ => unreachable!("filtered by unpack_float_cg_value"),
        };
        self.codegen_i64_hash_value(i64_bits)
    }

    fn codegen_float_abs_value(
        &mut self,
        receiver_span: crate::span::Span,
        recv: CgValue<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let (float_val, float_ty) =
            self.unpack_float_cg_value(recv, receiver_span, "Float.abs receiver type")?;
        match float_ty {
            CgTy::Float64 => {
                let raw = self.builder.build_bit_cast(
                    float_val,
                    self.context.i64_type(),
                    "f64_abs_bits",
                )?;
                let BasicValueEnum::IntValue(bits) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "Float64.abs bits type",
                        at: receiver_span.into(),
                    });
                };
                let masked = self.builder.build_and(
                    bits,
                    self.context
                        .i64_type()
                        .const_int(0x7fff_ffff_ffff_ffff, false),
                    "f64_abs_masked",
                )?;
                let raw =
                    self.builder
                        .build_bit_cast(masked, self.context.f64_type(), "f64_abs")?;
                let BasicValueEnum::FloatValue(abs_val) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "Float64.abs return type",
                        at: receiver_span.into(),
                    });
                };
                Ok(CgValue::float(abs_val, CgTy::Float64))
            }
            CgTy::Float32 => {
                let raw = self.builder.build_bit_cast(
                    float_val,
                    self.context.i32_type(),
                    "f32_abs_bits",
                )?;
                let BasicValueEnum::IntValue(bits) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "Float32.abs bits type",
                        at: receiver_span.into(),
                    });
                };
                let masked = self.builder.build_and(
                    bits,
                    self.context.i32_type().const_int(0x7fff_ffff, false),
                    "f32_abs_masked",
                )?;
                let raw =
                    self.builder
                        .build_bit_cast(masked, self.context.f32_type(), "f32_abs")?;
                let BasicValueEnum::FloatValue(abs_val) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "Float32.abs return type",
                        at: receiver_span.into(),
                    });
                };
                Ok(CgValue::float(abs_val, CgTy::Float32))
            }
            _ => unreachable!("filtered by unpack_float_cg_value"),
        }
    }

    fn codegen_float_is_nan_value(
        &mut self,
        receiver_span: crate::span::Span,
        recv: CgValue<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let (float_val, _) =
            self.unpack_float_cg_value(recv, receiver_span, "Float.isNaN receiver type")?;
        let is_nan = self.builder.build_float_compare(
            FloatPredicate::UNO,
            float_val,
            float_val,
            "float_is_nan",
        )?;
        Ok(CgValue::bool(is_nan))
    }

    fn codegen_float_is_infinite_value(
        &mut self,
        receiver_span: crate::span::Span,
        recv: CgValue<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let abs_value = self.codegen_float_abs_value(receiver_span, recv)?;
        match abs_value.ty {
            CgTy::Float64 => {
                let (float_val, _) = self.unpack_float_cg_value(
                    abs_value,
                    receiver_span,
                    "Float.isInfinite receiver type",
                )?;
                let raw = self.builder.build_bit_cast(
                    float_val,
                    self.context.i64_type(),
                    "f64_inf_bits",
                )?;
                let BasicValueEnum::IntValue(bits) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "Float64.isInfinite bits type",
                        at: receiver_span.into(),
                    });
                };
                let is_inf = self.builder.build_int_compare(
                    IntPredicate::EQ,
                    bits,
                    self.context
                        .i64_type()
                        .const_int(0x7ff0_0000_0000_0000, false),
                    "f64_is_infinite",
                )?;
                Ok(CgValue::bool(is_inf))
            }
            CgTy::Float32 => {
                let (float_val, _) = self.unpack_float_cg_value(
                    abs_value,
                    receiver_span,
                    "Float.isInfinite receiver type",
                )?;
                let raw = self.builder.build_bit_cast(
                    float_val,
                    self.context.i32_type(),
                    "f32_inf_bits",
                )?;
                let BasicValueEnum::IntValue(bits) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "Float32.isInfinite bits type",
                        at: receiver_span.into(),
                    });
                };
                let is_inf = self.builder.build_int_compare(
                    IntPredicate::EQ,
                    bits,
                    self.context.i32_type().const_int(0x7f80_0000, false),
                    "f32_is_infinite",
                )?;
                Ok(CgValue::bool(is_inf))
            }
            _ => unreachable!("Float.abs preserves float CgTy"),
        }
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

        if self.expr_is_builtin_char(expr) {
            let char_ty = CgTy::Int(IntTy {
                bits: 32,
                signed: false,
            });
            let recv = self.codegen_expr_in_expected_context(expr, Some(char_ty))?;
            let Some(raw) = recv.value else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "sysroot print/println char arg value",
                    at: expr.span.into(),
                });
            };
            let BasicValueEnum::IntValue(codepoint) = raw else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "sysroot print/println char arg type",
                    at: expr.span.into(),
                });
            };
            let str_ptr = self.codegen_char_to_string_value(expr.span, codepoint)?;
            let rt_fun = self.declare_runtime_print_like(rt_name);
            let _ =
                self.builder
                    .build_call(rt_fun, &[str_ptr.into()], "rt_print_char_as_string")?;
            return Ok(CgValue::unit());
        }

        // 说明：
        // - sysroot 中允许 `print/println` 以 overload set 的形式声明（例如 `String` 与 `Int`）；
        // - HIR 当前阶段不保留"已选定 overload"的信息，因此这里以实参 codegen 后的 `CgTy`
        //   来决定使用哪条 lowering 路径。
        //
        // 注意：这里**不要**强制把 expected type 设为 `String`：
        // - 对于 `when/if/block` 等表达式，expected 会导致其 arm/body 被强制 coercion 为 `String`，
        //   进而在 `Int -> String` 这类尚未实现的 coercion 上报错；
        // - `print/println` 的整数路径会在 codegen 后把 `Int` 提升/截断到 i64/u64 并调用 runtime 直接打印（见下方分支），
        //   因此应先让表达式产出其"自然值类型"，再在这里做转换。
        let v = self.codegen_expr(expr)?;
        match v.ty {
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

                let rt_fun = self.declare_runtime_print_like(rt_name);
                let _ = self
                    .builder
                    .build_call(rt_fun, &[str_ptr.into()], "rt_print")?;
                Ok(CgValue::unit())
            }
            CgTy::Int(from_ty) => {
                if from_ty.bits > 64 {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "integer width for print/println",
                        at: expr.span.into(),
                    });
                }

                let (raw_int, _) = v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "sysroot print/println int arg value",
                    at: expr.span.into(),
                })?;

                // 统一把整数提升/截断到 i64/u64，并在 codegen 侧构造一个 GC-managed `String` 再打印。
                //
                // 说明：
                // - 早期阶段曾用 `scoop_print{,ln}_{i64,u64}` 绕开 `rewrite-statepoints-for-gc` 的崩溃；
                // - GC-FIX Phase C2c：print/println 的整数路径与字符串路径对齐，确保字符串构造在 statepoint 下稳定。
                let to_ty = IntTy {
                    bits: 64,
                    signed: from_ty.signed,
                };
                let int64 = self.cast_int(raw_int, from_ty, to_ty)?;

                let str_ptr = self.codegen_int_to_string(expr.span, int64, to_ty.signed)?;
                let rt_fun = self.declare_runtime_print_like(rt_name);
                let _ =
                    self.builder
                        .build_call(rt_fun, &[str_ptr.into()], "rt_print_int_as_string")?;
                Ok(CgValue::unit())
            }
            // T0114: Bool → "true"/"false" → print.
            CgTy::Bool => {
                let Some(raw) = v.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "sysroot print/println bool arg value",
                        at: expr.span.into(),
                    });
                };
                let BasicValueEnum::IntValue(bool_val) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "sysroot print/println bool arg type",
                        at: expr.span.into(),
                    });
                };
                let i64_ty = self.context.i64_type();
                let bool_as_i64 =
                    self.builder
                        .build_int_z_extend(bool_val, i64_ty, "bool_zext_print")?;
                let rt_bool = self.declare_runtime_bool_to_string();
                let call = self.builder.build_call(
                    rt_bool,
                    &[bool_as_i64.into()],
                    "rt_bool_to_string_print",
                )?;
                let ret = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "Bool.toString return value for print",
                        at: expr.span.into(),
                    },
                )?;
                let BasicValueEnum::PointerValue(str_ptr) = ret else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "Bool.toString return type for print",
                        at: expr.span.into(),
                    });
                };
                let rt_fun = self.declare_runtime_print_like(rt_name);
                let _ = self.builder.build_call(
                    rt_fun,
                    &[str_ptr.into()],
                    "rt_print_bool_as_string",
                )?;
                Ok(CgValue::unit())
            }
            CgTy::Float64 | CgTy::Float32 => {
                let str_v = self.codegen_float_to_string_value(expr.span, expr.span, v)?;
                let Some(raw) = str_v.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "sysroot print/println float arg value",
                        at: expr.span.into(),
                    });
                };
                let BasicValueEnum::PointerValue(str_ptr) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "sysroot print/println float arg type",
                        at: expr.span.into(),
                    });
                };
                let rt_fun = self.declare_runtime_print_like(rt_name);
                let _ = self.builder.build_call(
                    rt_fun,
                    &[str_ptr.into()],
                    "rt_print_float_as_string",
                )?;
                Ok(CgValue::unit())
            }
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "sysroot print/println arg type",
                at: expr.span.into(),
            }),
        }
    }

    /// T0131/T0146c2：where-bound `ToString.toString` 拦截——内建类型短路分发。
    ///
    /// monomorphized `print<Bool>` body 调用 `value.toString()`，HIR 将其改写为
    /// `scoop.core.ToString.toString(value)`。此函数在 itable dispatch 前拦截：
    /// - 内建类型（Bool/Char/Int/String）→ 调用 runtime → `Some(CgValue)`
    /// - 其它类型（class/struct）→ `None`（fall-through 到 itable 或 fun_index）
    fn try_codegen_tostring_iface_builtin(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<Option<CgValue<'ctx>>, LlvmEmitError> {
        if args.len() != 1 {
            return Ok(None);
        }
        let hir::CallArg::Positional(expr) = &args[0] else {
            return Ok(None);
        };
        if self.expr_is_builtin_char(expr) {
            return self.codegen_char_method_to_string(span, expr).map(Some);
        }
        let recv = self.codegen_expr(expr)?;
        match recv.ty {
            CgTy::Bool => {
                let Some(raw) = recv.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "ToString.toString Bool value",
                        at: expr.span.into(),
                    });
                };
                let BasicValueEnum::IntValue(iv) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "ToString.toString Bool type",
                        at: expr.span.into(),
                    });
                };
                let i64_ty = self.context.i64_type();
                let bool_as_i64 = self.builder.build_int_z_extend(iv, i64_ty, "bool_zext")?;
                let rt_fun = self.declare_runtime_bool_to_string();
                let call =
                    self.builder
                        .build_call(rt_fun, &[bool_as_i64.into()], "rt_bool_to_string")?;
                let ret = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "ToString.toString Bool ret",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::PointerValue(str_ptr) = ret else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "ToString.toString Bool ret type",
                        at: span.into(),
                    });
                };
                Ok(Some(CgValue {
                    ty: CgTy::String,
                    value: Some(str_ptr.into()),
                }))
            }
            CgTy::Int(_) => {
                let Some(raw) = recv.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "ToString.toString Int value",
                        at: expr.span.into(),
                    });
                };
                let BasicValueEnum::IntValue(int_val) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "ToString.toString Int type",
                        at: expr.span.into(),
                    });
                };
                let rt_fun = self.declare_runtime_int_to_string();
                let call =
                    self.builder
                        .build_call(rt_fun, &[int_val.into()], "rt_int_to_string")?;
                let ret = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "ToString.toString Int ret",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::PointerValue(str_ptr) = ret else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "ToString.toString Int ret type",
                        at: span.into(),
                    });
                };
                Ok(Some(CgValue {
                    ty: CgTy::String,
                    value: Some(str_ptr.into()),
                }))
            }
            CgTy::Float64 | CgTy::Float32 => self
                .codegen_float_to_string_value(span, expr.span, recv)
                .map(Some),
            CgTy::String => Ok(Some(recv)),
            _ => Ok(None), // fall-through: let itable/fun_index handle user types
        }
    }

    /// T0131：`__scoop_print_string` / `__scoop_println_string` codegen 拦截。
    ///
    /// 泛型 `print<T>` / `println<T>` 的 monomorphized body 调用这些内部函数，
    /// 接受一个 String 参数并映射到 runtime 的 `scoop_print` / `scoop_println`。
    fn codegen_sysroot_internal_print_string(
        &mut self,
        span: crate::span::Span,
        _callee_span: crate::span::Span,
        fqn: &str,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let rt_name = match fqn {
            "scoop.core.__scoop_print_string" => "scoop_print",
            "scoop.core.__scoop_println_string" => "scoop_println",
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "__scoop_print_string fqn",
                    at: span.into(),
                });
            }
        };

        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "__scoop_print_string arity",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "__scoop_print_string named arg",
                at: span.into(),
            });
        };

        let val = self.codegen_expr(expr)?;
        let coerced = self.coerce_value(expr.span, val, CgTy::String)?;
        let Some(raw) = coerced.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "__scoop_print_string arg value",
                at: expr.span.into(),
            });
        };
        let BasicValueEnum::PointerValue(str_ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "__scoop_print_string arg type",
                at: expr.span.into(),
            });
        };
        let rt_fun = self.declare_runtime_print_like(rt_name);
        let _ = self
            .builder
            .build_call(rt_fun, &[str_ptr.into()], "rt_internal_print")?;
        Ok(CgValue::unit())
    }

    /// T0131/T0146c2：body-less extension function `toString()`（Char/Int/Bool/String）codegen 拦截。
    ///
    /// HIR lowering 将 `receiver.toString()` 改写为 top-level call `scoop.core.toString(receiver)`。
    /// 按 receiver CgTy 分发到对应 runtime 函数。
    fn codegen_sysroot_to_string_ext(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "toString ext arity",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "toString ext named arg",
                at: callee_span.into(),
            });
        };

        if self.expr_is_builtin_char(expr) {
            return self.codegen_char_method_to_string(span, expr);
        }

        let recv = self.codegen_expr(expr)?;
        match recv.ty {
            CgTy::Bool => {
                let Some(raw) = recv.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "toString ext Bool value",
                        at: expr.span.into(),
                    });
                };
                let BasicValueEnum::IntValue(iv) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "toString ext Bool type",
                        at: expr.span.into(),
                    });
                };
                let i64_ty = self.context.i64_type();
                let bool_as_i64 = self.builder.build_int_z_extend(iv, i64_ty, "bool_zext")?;
                let rt_fun = self.declare_runtime_bool_to_string();
                let call =
                    self.builder
                        .build_call(rt_fun, &[bool_as_i64.into()], "rt_bool_to_string")?;
                let ret = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "toString ext Bool ret",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::PointerValue(str_ptr) = ret else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "toString ext Bool ret type",
                        at: span.into(),
                    });
                };
                Ok(CgValue {
                    ty: CgTy::String,
                    value: Some(str_ptr.into()),
                })
            }
            CgTy::Int(_) => {
                let Some(raw) = recv.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "toString ext Int value",
                        at: expr.span.into(),
                    });
                };
                let BasicValueEnum::IntValue(int_val) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "toString ext Int type",
                        at: expr.span.into(),
                    });
                };
                let rt_fun = self.declare_runtime_int_to_string();
                let call =
                    self.builder
                        .build_call(rt_fun, &[int_val.into()], "rt_int_to_string")?;
                let ret = call.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "toString ext Int ret",
                        at: span.into(),
                    },
                )?;
                let BasicValueEnum::PointerValue(str_ptr) = ret else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "toString ext Int ret type",
                        at: span.into(),
                    });
                };
                Ok(CgValue {
                    ty: CgTy::String,
                    value: Some(str_ptr.into()),
                })
            }
            CgTy::Float64 | CgTy::Float32 => {
                self.codegen_float_to_string_value(span, expr.span, recv)
            }
            CgTy::String => {
                // String.toString() → return self.
                Ok(recv)
            }
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "toString ext unsupported CgTy",
                at: span.into(),
            }),
        }
    }

    /// T0146c2：body-less extension function `toInt()` codegen 拦截。
    ///
    /// 当前主要用于 `Char.toInt()`：HIR lowering 会把它改写为 `scoop.core.toInt(receiver)`。
    fn codegen_sysroot_to_int_ext(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "toInt ext arity",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "toInt ext named arg",
                at: callee_span.into(),
            });
        };

        if self.expr_is_builtin_char(expr) {
            return self.codegen_char_method_to_int(expr);
        }

        let recv = self.codegen_expr(expr)?;
        match recv.ty {
            CgTy::Float64 | CgTy::Float32 => self.codegen_float_to_int_value(span, expr.span, recv),
            CgTy::String => self.codegen_string_method(span, expr, "toInt", &[]),
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "toInt ext unsupported CgTy",
                at: span.into(),
            }),
        }
    }

    /// T0146c2：body-less extension function `hash()` codegen 拦截。
    ///
    /// 当前主要用于 `Char.hash()`：HIR lowering 会把它改写为 `scoop.core.hash(receiver)`。
    /// 这里按 receiver 的内建类型分发到对应 hashing 路径。
    fn codegen_sysroot_hash_ext(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "hash ext arity",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "hash ext named arg",
                at: callee_span.into(),
            });
        };

        if self.expr_is_builtin_char(expr) {
            return self.codegen_char_method_hash(span, expr);
        }

        let recv = self.codegen_expr(expr)?;
        match recv.ty {
            CgTy::Int(_) => self.codegen_int_method_hash(span, expr),
            CgTy::Float64 | CgTy::Float32 => self.codegen_float_hash_value(expr.span, recv),
            CgTy::String => self.codegen_string_method(span, expr, "hash", &[]),
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "hash ext unsupported CgTy",
                at: span.into(),
            }),
        }
    }

    fn codegen_sysroot_abs_ext(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "abs ext arity",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "abs ext named arg",
                at: callee_span.into(),
            });
        };

        let recv = self.codegen_expr(expr)?;
        match recv.ty {
            CgTy::Float64 | CgTy::Float32 => self.codegen_float_abs_value(expr.span, recv),
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "abs ext unsupported CgTy",
                at: span.into(),
            }),
        }
    }

    fn codegen_sysroot_is_nan_ext(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "isNaN ext arity",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "isNaN ext named arg",
                at: callee_span.into(),
            });
        };

        let recv = self.codegen_expr(expr)?;
        match recv.ty {
            CgTy::Float64 | CgTy::Float32 => self.codegen_float_is_nan_value(expr.span, recv),
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "isNaN ext unsupported CgTy",
                at: span.into(),
            }),
        }
    }

    fn codegen_sysroot_is_infinite_ext(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if args.len() != 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "isInfinite ext arity",
                at: span.into(),
            });
        }

        let hir::CallArg::Positional(expr) = &args[0] else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "isInfinite ext named arg",
                at: callee_span.into(),
            });
        };

        let recv = self.codegen_expr(expr)?;
        match recv.ty {
            CgTy::Float64 | CgTy::Float32 => self.codegen_float_is_infinite_value(expr.span, recv),
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "isInfinite ext unsupported CgTy",
                at: span.into(),
            }),
        }
    }

    fn codegen_int_to_string(
        &mut self,
        span: crate::span::Span,
        int64: IntValue<'ctx>,
        signed: bool,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let i64_ty = self.context.i64_type();
        let i8_ty = self.context.i8_type();
        let i8_ptr_ty = self.llvm_i8_ptr_type();

        // 1) 先把整数格式化到栈上的临时 buffer（native addrspace(0)），得到实际字节长度。
        //
        // 说明：
        // - `scoop_format_{i64,u64}` 为 "caller 提供 buffer + cap" 形式；
        // - 这里的 `buf` 是纯 native bytes，不应被当作 GC-managed roots。
        let cap = i64_ty.const_int(64, false);
        let buf = self
            .builder
            .build_array_alloca(i8_ty, cap, "print_int_buf")?;

        let fmt_name = if signed {
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
                kind: "print/println int format length",
                at: span.into(),
            })?
            .into_int_value();

        // 2) 分配 heap buffer（malloc）并拷贝 bytes；len==0 时保持 data=NULL。
        let is_zero = self.builder.build_int_compare(
            IntPredicate::EQ,
            len,
            i64_ty.const_zero(),
            "print_int_len_is_zero",
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

        let malloc_bb = self.context.append_basic_block(func, "print_int_malloc");
        let done_bb = self.context.append_basic_block(func, "print_int_done");

        self.builder
            .build_conditional_branch(is_zero, done_bb, malloc_bb)?;

        // --- malloc + memcpy ---
        self.builder.position_at_end(malloc_bb);
        let malloc = self.declare_libc_malloc();
        let call = self
            .builder
            .build_call(malloc, &[len.into()], "print_int_malloc")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "malloc return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(heap_buf) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "malloc return type",
                at: span.into(),
            });
        };
        let _ = self.builder.build_memcpy(heap_buf, 1, buf, 1, len)?;
        self.builder.build_unconditional_branch(done_bb)?;

        // --- done ---
        self.builder.position_at_end(done_bb);
        let buf_phi = self.builder.build_phi(i8_ptr_ty, "print_int_data_buf")?;
        let buf_null: BasicValueEnum<'ctx> = i8_ptr_ty.const_null().into();
        let buf_value: BasicValueEnum<'ctx> = heap_buf.into();
        buf_phi.add_incoming(&[(&buf_null, insert_block), (&buf_value, malloc_bb)]);
        let data_ptr = buf_phi.as_basic_value().into_pointer_value();

        // 3) 分配并初始化 `ScoopString` 对象（GC-managed）。
        //
        // 注意：必须在 codegen 侧通过 `scoop_alloc_typed` 触发 statepoint safepoint，
        // 不能在 runtime helper 内部隐式分配并触发 GC（否则 caller frame 无 stackmap roots）。
        let scoop_str_ty = self.llvm_scoop_string_type();
        let obj_size = self.target_data.get_store_size(&scoop_str_ty);
        let size_v = i64_ty.const_int(obj_size, false);

        let str_desc = self.get_or_create_string_type_desc_global(span)?;
        let str_desc_i8 = self.builder.build_pointer_cast(
            str_desc.as_pointer_value(),
            self.llvm_i8_ptr_type(),
            "print_int_type_desc_i8",
        )?;
        let rt_alloc = self.declare_runtime_alloc_typed();
        let call = self.builder.build_call(
            rt_alloc,
            &[str_desc_i8.into(), size_v.into()],
            "rt_alloc_print_int_str",
        )?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc_typed return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(raw_ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc_typed return type",
                at: span.into(),
            });
        };

        let str_ptr_ty = self.llvm_scoop_string_ptr_type();
        let str_ptr =
            self.builder
                .build_pointer_cast(raw_ptr, str_ptr_ty, "print_int_str_obj_ptr")?;

        let len_ptr =
            self.builder
                .build_struct_gep(scoop_str_ty, str_ptr, 1, "print_int_len_gep")?;
        let data_ptr_gep =
            self.builder
                .build_struct_gep(scoop_str_ty, str_ptr, 2, "print_int_data_gep")?;

        let _ = self.builder.build_store(len_ptr, len)?;
        let _ = self.builder.build_store(data_ptr_gep, data_ptr)?;
        Ok(str_ptr)
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
        let call = self.builder.build_call(rt, &[key_ptr.into()], "env_get")?;
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
        let call = self.builder.build_call(rt, &[], "time_now_unix_millis")?;
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
        let code_v =
            self.codegen_expr_in_expected_context(code_expr, Some(CgTy::Int(value_word)))?;
        let code_v = self.coerce_value(code_expr.span, code_v, CgTy::Int(value_word))?;
        let (code_raw, code_from) = code_v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
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
        let call =
            self.builder
                .build_call(rt, &[base_ptr.into(), child_ptr.into()], "path_join")?;
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
        let call = self.builder.build_call(rt, &[], "sync_mutex_create")?;
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
        let call = self.builder.build_call(rt, &[], "sync_condvar_create")?;
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
        let _ = self
            .builder
            .build_call(rt, &[cv_ptr.into(), m_ptr.into()], "sync_condvar_wait")?;
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
        let _ = self
            .builder
            .build_call(rt, &[cv_ptr.into()], "sync_condvar_notify_one")?;
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
        let _ = self
            .builder
            .build_call(rt, &[cv_ptr.into()], "sync_condvar_notify_all")?;
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
                // - 但 early stage 的 `fun_index` 只包含"本编译单元内有 body 的函数"，不含 sysroot 声明；
                // - 同时 HIR v0 对 closure expr 的 `ty` 也不总是可用作 expected type（需要 MIR/CFG 才能更稳）。
                //
                // 因此这里从 `TypeStore` 中查找一个"无参、返回 Unit、Pure"的函数类型作为 expected context。
                let expected_fun_ty = self.lookup_pure_unit_closure_type().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "sync.Once.run block fun type",
                        at: block_expr.span.into(),
                    },
                )?;
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
        let closure_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
        let closure_ptr =
            self.builder
                .build_pointer_cast(block_obj_i8, closure_ptr_ty, "once_block_ptr")?;

        let i8_ptr_ty = self.llvm_i8_ptr_type();
        let env_gep = self
            .builder
            .build_struct_gep(closure_ty, closure_ptr, 1, "once_env_gep")?;
        let fn_gep = self
            .builder
            .build_struct_gep(closure_ty, closure_ptr, 2, "once_fn_gep")?;

        let env_ptr = self
            .builder
            .build_load(i8_ptr_ty, env_gep, "once_env")?
            .into_pointer_value();
        let fn_ptr_raw = self
            .builder
            .build_load(i8_ptr_ty, fn_gep, "once_fn_raw")?
            .into_pointer_value();

        let init_fn_ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        let init_fn_ptr =
            self.builder
                .build_pointer_cast(fn_ptr_raw, init_fn_ptr_ty, "once_fn_typed")?;

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
                //   查找一个"无参、返回 Unit、Pure"的函数类型作为 expected context。
                let expected_fun_ty = self.lookup_pure_unit_closure_type().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "thread.threadSpawn block fun type",
                        at: block_expr.span.into(),
                    },
                )?;
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
        let closure_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
        let closure_ptr =
            self.builder
                .build_pointer_cast(block_obj_i8, closure_ptr_ty, "thread_block_ptr")?;

        let i8_ptr_ty = self.llvm_i8_ptr_type();
        let env_gep =
            self.builder
                .build_struct_gep(closure_ty, closure_ptr, 1, "thread_env_gep")?;
        let fn_gep = self
            .builder
            .build_struct_gep(closure_ty, closure_ptr, 2, "thread_fn_gep")?;

        let env_ptr = self
            .builder
            .build_load(i8_ptr_ty, env_gep, "thread_env")?
            .into_pointer_value();
        let fn_ptr_raw = self
            .builder
            .build_load(i8_ptr_ty, fn_gep, "thread_fn_raw")?
            .into_pointer_value();

        let start_fn_ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        let start_fn_ptr =
            self.builder
                .build_pointer_cast(fn_ptr_raw, start_fn_ptr_ty, "thread_fn_typed")?;

        let rt = self.declare_runtime_thread_spawn();
        let call =
            self.builder
                .build_call(rt, &[env_ptr.into(), start_fn_ptr.into()], "thread_spawn")?;
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
        let call = self.builder.build_call(rt, &[], "thread_current_id")?;
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
        //
        // 注意（GC-FIX C2b）：当前 runtime 的 channel nodes 用 `malloc/free` 管理且不参与 GC trace，
        // 因此这里暂只允许 word payload；Ref/String 若进入队列会变成 silent roots hole。
        let elem_cg = match self.types.kind(channel_expr.ty) {
            TypeKind::Ref(RefTypeKind::Nominal(nominal))
                if nominal.fqn == "scoop.channels.Channel" && nominal.args.len() == 1 =>
            {
                self.cg_ty_of(nominal.args[0])
                    .filter(|ty| matches!(ty, CgTy::Unit | CgTy::Bool | CgTy::Int(_)))
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

        // gate：确保元素是 "u64 word 可编码"的类型（与 `coerce_u64_word` 对齐）。
        let elem_cg = self
            .cg_ty_of(elem_ty)
            .filter(|ty| matches!(ty, CgTy::Unit | CgTy::Bool | CgTy::Int(_)));
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
        let some_v = self.build_enum_value(
            span,
            option_ty,
            0,
            CgEnumPayload {
                word: Some(payload_word),
                gc_ptr: None,
            },
        )?;
        let some_raw = some_v.value.ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "channels.Channel.recv Some value",
            at: span.into(),
        })?;
        self.builder.build_unconditional_branch(merge_bb)?;
        let some_end =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: span.into(),
                })?;

        // none branch：构造 `None`。
        self.builder.position_at_end(none_bb);
        let none_v = self.build_enum_value(span, option_ty, 1, CgEnumPayload::default())?;
        let none_raw = none_v.value.ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "channels.Channel.recv None value",
            at: span.into(),
        })?;
        self.builder.build_unconditional_branch(merge_bb)?;
        let none_end =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: span.into(),
                })?;

        // merge：phi 合并结果。
        self.builder.position_at_end(merge_bb);
        let phi = self
            .builder
            .build_phi(option_llvm_ty, "channels_recv_phi")?;
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
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "task.executorCreate return value",
                at: span.into(),
            })?;
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
        let executor_v =
            self.coerce_value(executor_expr.span, executor_v, CgTy::Int(handle_word))?;
        let (raw_handle, from) = executor_v
            .as_int()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
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
        let executor_v =
            self.coerce_value(executor_expr.span, executor_v, CgTy::Int(handle_word))?;
        let (raw_handle, from) = executor_v
            .as_int()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
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
        let call =
            self.builder
                .build_call(rt, &[handle_u64.into()], "executor_debug_pending_count")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "task.Executor.debugPendingCount return value",
                at: span.into(),
            })?;
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
        let executor_v =
            self.coerce_value(executor_expr.span, executor_v, CgTy::Int(handle_word))?;
        let (raw_handle, from) = executor_v
            .as_int()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
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
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "task.Executor.runNext return value",
                at: span.into(),
            })?;
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
        let executor_v =
            self.coerce_value(executor_expr.span, executor_v, CgTy::Int(handle_word))?;
        let (raw_handle, from) = executor_v
            .as_int()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
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
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "task.Executor.runUntilIdle return value",
                at: span.into(),
            })?;
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
        let closure_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
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
        let body_fn_ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        let body_fn_ptr =
            self.builder
                .build_pointer_cast(fn_ptr_raw, body_fn_ptr_ty, "task_body_fn_typed")?;

        let rt = self.declare_runtime_task_u64_create();
        let call = self.builder.build_call(
            rt,
            &[body_fn_ptr.into(), env_ptr.into()],
            "task_u64_create",
        )?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "task.taskCreate return value",
                at: span.into(),
            })?;
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

        let i8_ptr_ty = self.llvm_i8_ptr_type();
        let body_fn_ptr_ty = self.llvm_ptr_type(AddressSpace::default());

        let rt = self.declare_runtime_task_u64_create();
        let call = self.builder.build_call(
            rt,
            &[
                body_fn_ptr_ty.const_null().into(),
                i8_ptr_ty.const_null().into(),
            ],
            "task_u64_create_manual",
        )?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "task.taskCreateManual return value",
                at: span.into(),
            })?;
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

        let task_v =
            self.codegen_expr_in_expected_context(task_expr, Some(CgTy::Int(handle_word)))?;
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
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "task.Task.state return value",
                at: span.into(),
            })?;
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

        let task_v =
            self.codegen_expr_in_expected_context(task_expr, Some(CgTy::Int(handle_word)))?;
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
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "task.Task.result return value",
                at: span.into(),
            })?;
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

        let task_v =
            self.codegen_expr_in_expected_context(task_expr, Some(CgTy::Int(handle_word)))?;
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
        let (raw_exec, from) = executor_v
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
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "task.Task.tryStart return value",
                at: span.into(),
            })?;
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

        let task_v =
            self.codegen_expr_in_expected_context(task_expr, Some(CgTy::Int(handle_word)))?;
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
        let (raw_value, from) = value_v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
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
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "task.Task.complete return value",
                at: span.into(),
            })?;
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

        let task_v =
            self.codegen_expr_in_expected_context(task_expr, Some(CgTy::Int(handle_word)))?;
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
        let (raw_exec, from) = executor_v
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
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "task.Task.onComplete return value",
                at: span.into(),
            })?;
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
            "scoop.core.__scoop_array_builder_push"
            | "scoop.core.__scoop_array_builder_push_string" => {
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
            | "scoop.core.__scoop_array_builder_build_mutable_array"
            | "scoop.core.__scoop_array_builder_build_array_string" => {
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
                    "scoop.core.__scoop_array_builder_build_array"
                    | "scoop.core.__scoop_array_builder_build_array_string" => {
                        self.declare_runtime_array_builder_build_array()
                    }
                    "scoop.core.__scoop_array_builder_build_mutable_array" => {
                        self.declare_runtime_array_builder_build_mutable_array()
                    }
                    _ => unreachable!("match arms cover all cases"),
                };

                let call =
                    self.builder
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
        let _gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();

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

                let idx_v =
                    self.codegen_expr_in_expected_context(idx_expr, Some(CgTy::Int(value_word)))?;
                let idx_v = self.coerce_value(idx_expr.span, idx_v, CgTy::Int(value_word))?;
                let (idx_raw, idx_from) =
                    idx_v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
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
                    .or_else(|| {
                        expected.filter(|ty| {
                            matches!(
                                ty,
                                CgTy::Unit | CgTy::Bool | CgTy::Int(_) | CgTy::String | CgTy::Ref
                            )
                        })
                    })
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
                                let casted = self.builder.build_pointer_cast(
                                    ptr,
                                    str_ptr_ty,
                                    "ref_to_str",
                                )?;
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
                        self.decode_u64_word_to_cg_value(span, word_u64, elem_ty)
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

                let idx_v =
                    self.codegen_expr_in_expected_context(idx_expr, Some(CgTy::Int(value_word)))?;
                let idx_v = self.coerce_value(idx_expr.span, idx_v, CgTy::Int(value_word))?;
                let (idx_raw, idx_from) =
                    idx_v.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "MutableArray.set index value",
                        at: idx_expr.span.into(),
                    })?;
                let idx_to = IntTy {
                    bits: 64,
                    signed: true,
                };
                let idx_i64 = self.cast_int(idx_raw, idx_from, idx_to)?;

                // 尽量使用 receiver 的静态类型（type args）来决定 value 的 codegen/编码方式；
                // 若无法恢复，则退化为"按 value 表达式自身的 codegen 类型编码为 u64"。
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
                                let v = self
                                    .codegen_expr_in_expected_context(value_expr, Some(elem_ty))?;
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
            hir::ExprKind::VarRef(hir::ValueRef::TopLevel { fqn, .. }) => self
                .top_level_vars
                .get(fqn)
                .map(|var| var.ty)
                .or_else(|| self.top_level_consts.get(fqn).map(|value| value.ty))?,
            _ => return None,
        };

        let TypeKind::Ref(RefTypeKind::Nominal(nominal)) = self.types.kind(receiver_ty) else {
            return None;
        };
        // T1317f2：`List/MutableList` 在 sysroot 中作为 `Array/MutableArray` 的 typealias。
        // codegen 侧需要把它们视为"array-like"，否则 `xs.get(i)` 在被 `print/println` 等
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

        // 当前 runtime array 以 "u64 word buffer" 表示元素，因此这里限制为可编码为 u64 的类型。
        match cg {
            CgTy::Unit
            | CgTy::Bool
            | CgTy::Float64
            | CgTy::Float32
            | CgTy::Int(_)
            | CgTy::String
            | CgTy::Ref => Some(cg),
            CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_) | CgTy::Never => None,
        }
    }

    fn decode_u64_word_to_cg_value(
        &mut self,
        at: crate::span::Span,
        word_u64: IntValue<'ctx>,
        to: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let from_u64 = IntTy {
            bits: 64,
            signed: false,
        };

        match to {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Never => Ok(CgValue::never()),
            CgTy::Bool => {
                let is_true = self.builder.build_int_compare(
                    IntPredicate::NE,
                    word_u64,
                    self.context.i64_type().const_zero(),
                    "u64_to_bool",
                )?;
                Ok(CgValue::bool(is_true))
            }
            CgTy::Float64 => {
                let raw = self
                    .builder
                    .build_bit_cast(word_u64, self.context.f64_type(), "u64_to_f64_bits")?
                    .into_float_value();
                Ok(CgValue::float(raw, CgTy::Float64))
            }
            CgTy::Float32 => {
                let bits32 = self.builder.build_int_truncate(
                    word_u64,
                    self.context.i32_type(),
                    "u64_to_f32_bits",
                )?;
                let raw = self
                    .builder
                    .build_bit_cast(bits32, self.context.f32_type(), "i32_to_f32_bits")?
                    .into_float_value();
                Ok(CgValue::float(raw, CgTy::Float32))
            }
            CgTy::Int(int_ty) => {
                let decoded = self.cast_int(word_u64, from_u64, int_ty)?;
                Ok(CgValue::int(decoded, int_ty))
            }
            CgTy::Ref | CgTy::String => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "decode u64 word to gc pointer (ptr<->int is forbidden)",
                at: at.into(),
            }),
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
                let k_i8 = self.builder.build_pointer_cast(
                    k_ptr,
                    self.llvm_gc_i8_ptr_type(),
                    "thread_resume_k_i8",
                )?;
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
                let loaded = self.builder.build_load(llvm_ty, ptr, "atomic_int_load")?;
                let inst =
                    loaded
                        .as_instruction_value()
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "atomicIntLoad load instruction",
                            at: target_expr.span.into(),
                        })?;
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

                let v = self
                    .codegen_expr_in_expected_context(value_expr, Some(CgTy::Int(atomic_word)))?;
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

                let expected_v = self.codegen_expr_in_expected_context(
                    expected_expr,
                    Some(CgTy::Int(atomic_word)),
                )?;
                let expected_v =
                    self.coerce_value(expected_expr.span, expected_v, CgTy::Int(atomic_word))?;
                let (expected_raw, expected_from) =
                    expected_v
                        .as_int()
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "atomicIntCompareExchange expected",
                            at: expected_expr.span.into(),
                        })?;
                let expected_raw = self.cast_int(expected_raw, expected_from, atomic_word)?;

                let desired_v = self
                    .codegen_expr_in_expected_context(desired_expr, Some(CgTy::Int(atomic_word)))?;
                let desired_v =
                    self.coerce_value(desired_expr.span, desired_v, CgTy::Int(atomic_word))?;
                let (desired_raw, desired_from) =
                    desired_v
                        .as_int()
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
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
                let success = self.builder.build_extract_value(cx, 1, "cmpxchg_success")?;
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
                let cg_ty = self
                    .cg_ty_of(var.ty)
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

    /// T0126: 尝试将泛型 class 成员方法调用的 FQN 解析为单态化后的变体。
    ///
    /// 例如：`Box.get` + receiver `Box<Int>` → `Box.get::<Int>`
    ///
    /// 返回 `Some(mangled_fqn)` 表示应使用单态化变体；`None` 表示无需重写。
    /// T0130: 递归检查类型是否包含 `TypeKind::Param`（包括 Nominal 内部的类型参数）。
    fn ty_contains_param(&self, ty: crate::ty::TypeId) -> bool {
        let mut stack = vec![ty];
        while let Some(id) = stack.pop() {
            match self.types.kind(id) {
                crate::ty::TypeKind::Param(_) => return true,
                crate::ty::TypeKind::Ref(crate::ty::RefTypeKind::Nominal(n))
                | crate::ty::TypeKind::Value(crate::ty::ValueTypeKind::Nominal(n)) => {
                    stack.extend(n.args.iter().copied());
                }
                _ => {}
            }
        }
        false
    }

    fn try_resolve_monomorphized_member_fqn(
        &self,
        fqn: &str,
        args: &[hir::CallArg],
    ) -> Option<String> {
        // 检查 fun_index 中原始函数是否存在以及其签名是否包含 Param 类型
        // T0130: 递归检查 Nominal 类型参数内部的 Param（例如 `this: Printer<T>`）
        let sig_fun = self.fun_index.get(fqn).copied()?;

        let has_param = sig_fun.params.iter().any(|p| self.ty_contains_param(p.ty))
            || self.ty_contains_param(sig_fun.return_ty);

        if !has_param {
            return None;
        }

        // 从第一个参数（receiver）的 HIR 表达式中提取 hir_ty
        let first_arg = args.first()?;
        let hir::CallArg::Positional(receiver_expr) = first_arg else {
            return None;
        };

        let receiver_hir_ty =
            if let hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) = &receiver_expr.kind {
                self.env.get(*id).and_then(|local| local.hir_ty)?
            } else {
                return None;
            };

        // 从 receiver 的 nominal 类型中提取 type args
        let crate::ty::TypeKind::Ref(crate::ty::RefTypeKind::Nominal(nominal)) =
            self.types.kind(receiver_hir_ty)
        else {
            return None;
        };

        if nominal.args.is_empty() {
            return None;
        }

        // 构造单态化后的 FQN：`fqn::<Arg1, Arg2, ...>`
        let args_str = nominal
            .args
            .iter()
            .map(|id| self.types.display(*id).to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let mangled_fqn = format!("{fqn}::<{args_str}>");

        // 只在 fun_index 中确实存在单态化变体时才返回
        if self.fun_index.contains_key(&mangled_fqn) {
            Some(mangled_fqn)
        } else {
            None
        }
    }

    /// T0127: 尝试将泛型独立函数调用的 FQN 解析为单态化后的变体。
    ///
    /// 例如：`id(42)` 其中 `fun <T> id(x: T): T` → `id::<Int>`
    ///
    /// 工作原理：
    /// 1. 检查 fun_index 中原始函数的签名是否包含 Param 类型
    /// 2. 从 Param 类型与 call arg 类型的对应关系推导 type param → concrete type 映射
    /// 3. 构造 mangled FQN 并在 fun_index 中查找
    fn try_resolve_monomorphized_standalone_fun_fqn(
        &self,
        fqn: &str,
        args: &[hir::CallArg],
    ) -> Option<String> {
        let sig_fun = self.fun_index.get(fqn).copied()?;

        // 收集签名中出现的所有 Param 类型名（保持出现顺序）
        let mut param_names: Vec<String> = Vec::new();
        for p in &sig_fun.params {
            if let crate::ty::TypeKind::Param(tp) = self.types.kind(p.ty)
                && !param_names.contains(&tp.name)
            {
                param_names.push(tp.name.clone());
            }
        }
        if let crate::ty::TypeKind::Param(tp) = self.types.kind(sig_fun.return_ty)
            && !param_names.contains(&tp.name)
        {
            param_names.push(tp.name.clone());
        }

        if param_names.is_empty() {
            return None;
        }

        // 从 call args 的类型推导 Param → concrete TypeId 映射
        let mut bindings: std::collections::HashMap<String, crate::ty::TypeId> =
            std::collections::HashMap::new();

        for (idx, param) in sig_fun.params.iter().enumerate() {
            let crate::ty::TypeKind::Param(tp) = self.types.kind(param.ty) else {
                continue;
            };
            if bindings.contains_key(&tp.name) {
                continue;
            }

            // 获取对应 call arg 的具体类型
            let arg = args.get(idx)?;
            let hir::CallArg::Positional(arg_expr) = arg else {
                continue;
            };
            let concrete_ty = self.resolve_expr_concrete_type(arg_expr);
            if let Some(ty) = concrete_ty {
                bindings.insert(tp.name.clone(), ty);
            }
        }

        // 检查是否所有 Param 都已绑定
        if bindings.len() != param_names.len() {
            // 未能全部推导，尝试 fallback：在 fun_index 中查找唯一的 monomorphized 变体
            let prefix = format!("{fqn}::<");
            let variants: Vec<&str> = self
                .fun_index
                .keys()
                .filter(|k| k.starts_with(&prefix))
                .map(|k| k.as_str())
                .collect();
            if variants.len() == 1 {
                return Some(variants[0].to_string());
            }
            return None;
        }

        // 按 param_names 顺序构造 mangled FQN
        let args_str = param_names
            .iter()
            .map(|name| {
                let ty = bindings[name];
                self.types.display(ty).to_string()
            })
            .collect::<Vec<_>>()
            .join(", ");
        let mangled_fqn = format!("{fqn}::<{args_str}>");

        if self.fun_index.contains_key(&mangled_fqn) {
            Some(mangled_fqn)
        } else {
            None
        }
    }

    /// T0127: 从 HIR 表达式中尽量提取具体（非 Any/Param）的 TypeId。
    ///
    /// 对于字面量表达式直接返回其 HIR type；对于变量引用尝试从 env 中获取 hir_ty。
    /// T0130: 对于 Call 表达式，尝试通过 callee 的已知签名推导返回类型。
    fn resolve_expr_concrete_type(&self, expr: &hir::Expr) -> Option<crate::ty::TypeId> {
        let ty = expr.ty;
        // 如果 HIR type 不是 Any，直接使用
        if !matches!(
            self.types.kind(ty),
            crate::ty::TypeKind::Ref(crate::ty::RefTypeKind::Any)
        ) {
            return Some(ty);
        }

        // 对于 VarRef，尝试从 env 中获取 hir_ty
        if let hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) = &expr.kind {
            return self.env.get(*id).and_then(|local| local.hir_ty);
        }

        if let hir::ExprKind::VarRef(hir::ValueRef::TopLevel { fqn, .. }) = &expr.kind {
            return self
                .top_level_vars
                .get(fqn)
                .map(|var| var.ty)
                .or_else(|| self.top_level_consts.get(fqn).map(|value| value.ty));
        }

        // T0130: 对于 Call 表达式，尝试通过 callee 推导返回类型。
        // 典型场景：`printDescription(Dog())` 中 `Dog()` 是一个 class 构造器调用。
        if let hir::ExprKind::Call { callee, .. } = &expr.kind {
            return self.resolve_call_result_type(callee);
        }

        None
    }

    /// `Char` 在 LLVM 侧与 `Int` 同为 `CgTy::Int`，因此 builtin 分发需要额外看 HIR concrete type。
    fn expr_is_builtin_char(&self, expr: &hir::Expr) -> bool {
        let ty = self.resolve_expr_concrete_type(expr).unwrap_or(expr.ty);
        matches!(self.types.kind(ty), TypeKind::Value(ValueTypeKind::Char))
    }

    /// T0130: 尝试从 callee 表达式推导 Call 结果的具体类型。
    fn resolve_call_result_type(&self, callee: &hir::Expr) -> Option<crate::ty::TypeId> {
        let fqn = match &callee.kind {
            hir::ExprKind::VarRef(hir::ValueRef::TopLevel { fqn, .. }) => Some(fqn.as_str()),
            hir::ExprKind::UnresolvedIdent { name } => Some(name.as_str()),
            _ => None,
        }?;

        // 查找 class_inits 中是否有这个 FQN（class 构造器调用）
        if self.class_inits.contains_key(fqn) {
            return self.types.find_nominal_ref_by_fqn(fqn);
        }
        // 查找 fun_index 中的返回类型
        if let Some(fun) = self.fun_index.get(fqn) {
            let ret_kind = self.types.kind(fun.return_ty);
            if !matches!(
                ret_kind,
                crate::ty::TypeKind::Ref(crate::ty::RefTypeKind::Any)
            ) && !matches!(ret_kind, crate::ty::TypeKind::Param(_))
            {
                return Some(fun.return_ty);
            }
        }
        None
    }

    fn codegen_top_level_fun_call(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        fqn: &str,
        args: &[hir::CallArg],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        // T0126：对泛型 class 的成员方法调用，尝试重定向到单态化后的变体。
        // T0127：对泛型独立函数调用，同样尝试重定向到单态化后的变体。
        let effective_fqn = self
            .try_resolve_monomorphized_member_fqn(fqn, args)
            .or_else(|| self.try_resolve_monomorphized_standalone_fun_fqn(fqn, args));
        let fqn = effective_fqn.as_deref().unwrap_or(fqn);

        // T1510c：`@Extern` 调用点需要显式标记"进入 native"并向 runtime 暴露 roots slots，
        // 以便 stop-the-world GC 在 InNative 线程上扫描/更新 roots（moving GC 也需写回 slot）。
        let is_extern = self.extern_funs.contains_key(fqn);

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

        if is_extern {
            self.emit_enter_native_for_extern_call(span)?;
        }

        let call_site = self.builder.build_call(llvm_fun, &llvm_args, "call")?;
        call_site.set_call_convention(self.llvm_call_convention_for_fqn(fqn));

        // extern/native 调用返回后，必须调用 leave_native 将线程状态从 InNative 切回 InManaged，
        // 否则后续 effect 状态机检查（如 perform/Raise 设置的 TLS active flag）会在错误的线程状态下执行。
        if is_extern {
            let leave = self.declare_runtime_leave_native();
            let _ = self.builder.build_call(leave, &[], "leave_native")?;
        }

        self.emit_ordinary_call_effect_propagation_check(span, "direct_call_effect")?;

        let ret_cg =
            self.cg_ty_of(sig_fun.return_ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "call return type",
                    at: span.into(),
                })?;

        match ret_cg {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Never => Ok(CgValue::never()),
            _ => {
                let raw = call_site.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "call return value",
                        at: span.into(),
                    },
                )?;
                self.cg_value_from_loaded(span, ret_cg, raw)
            }
        }
    }

    /// 为 `@Extern` 调用点生成 `scoop_enter_native(root_slots, len)`。
    ///
    /// 设计取舍（v0）：
    /// - 这里采用保守策略：把当前 scope 中所有 `Ref/String` locals 的栈槽地址作为 roots slots；
    /// - 这样可以保证 GC 在 native 期间能扫描/更新这些 slots（moving GC 也可写回更新后的指针）；
    /// - 代价是可能多保活一些对象（但不会漏 roots）。
    fn emit_enter_native_for_extern_call(
        &mut self,
        at: crate::span::Span,
    ) -> Result<(), LlvmEmitError> {
        let slot_ptr_ty = self.llvm_ptr_type(AddressSpace::default()); // `void**`
        let slots_ptr_ty = self.llvm_ptr_type(AddressSpace::default()); // `void***`
        let i32_ty = self.context.i32_type();

        // 收集当前作用域内的 roots slots：使用局部变量自身的 alloca 槽位（而不是 shadow stack slot），
        // 以便 moving GC 更新时能被后续 `load local.ptr` 读取到最新指针值。
        let mut slots: Vec<(u32, PointerValue<'ctx>)> = Vec::new();
        for frame in &self.env.scopes {
            for (id, local) in frame {
                if matches!(local.ty, CgTy::Ref | CgTy::String) {
                    slots.push((id.as_u32(), local.ptr));
                }
            }
        }
        slots.sort_by_key(|(id, _)| *id);

        let (slots_base, slots_len) = if slots.is_empty() {
            (slots_ptr_ty.const_null(), i32_ty.const_zero())
        } else {
            let arr_ty = slot_ptr_ty.array_type(slots.len() as u32);
            let arr_ptr = self.create_entry_alloca_raw(at, "native_root_slots", arr_ty.into())?;
            let base =
                self.builder
                    .build_pointer_cast(arr_ptr, slots_ptr_ty, "native_root_slots_base")?;

            for (idx, (_id, local_ptr)) in slots.iter().enumerate() {
                let slot_ptr = self.builder.build_pointer_cast(
                    *local_ptr,
                    slot_ptr_ty,
                    "native_root_slot_cast",
                )?;
                let idx_v = i32_ty.const_int(idx as u64, false);
                let elem_ptr = unsafe {
                    self.builder.build_in_bounds_gep(
                        slot_ptr_ty,
                        base,
                        &[idx_v],
                        &format!("native_root_slot_gep_{idx}"),
                    )?
                };
                let _ = self.builder.build_store(elem_ptr, slot_ptr)?;
            }

            (base, i32_ty.const_int(slots.len() as u64, false))
        };

        let enter = self.declare_runtime_enter_native();
        let _ = self.builder.build_call(
            enter,
            &[slots_base.into(), slots_len.into()],
            "enter_native",
        )?;
        Ok(())
    }

    fn try_codegen_class_vtable_call(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        fqn: &str,
        args: &[hir::CallArg],
    ) -> Result<Option<CgValue<'ctx>>, LlvmEmitError> {
        let Some((owner_fqn, method_name)) = fqn.rsplit_once('.') else {
            return Ok(None);
        };

        let Some(slots) = self.class_vtables.get(owner_fqn) else {
            return Ok(None);
        };
        if slots.is_empty() {
            return Ok(None);
        }

        let Some((receiver_arg, _call_args)) = args.split_first() else {
            return Ok(None);
        };
        let hir::CallArg::Positional(receiver_expr) = receiver_arg else {
            return Ok(None);
        };

        // 注意：不能依赖 `receiver_expr.ty` 来决定是否走虚调用。
        //
        // 原因：
        // - lowering/early typecheck 阶段里，很多 HIR expression 的 `ty` 仍可能是 placeholder（例如 Any）；
        // - 但 member call lowering 已经把 `receiver.method(args...)` 解析为顶层调用
        //   `Owner.method(receiver, args...)`，并且 `class_vtables` 也仅包含 open/abstract/override 成员；
        // - 因此只要能在 vtable slot 中找到对应条目，就应当生成 vtable 间接调用。
        let explicit_params_len = args.len().saturating_sub(1) as u32;
        let slot = slots
            .iter()
            .find(|s| s.name == method_name && s.params_len == explicit_params_len)
            .map(|s| s.slot);

        let Some(slot) = slot else {
            return Ok(None);
        };

        // T1603：去虚化（receiver 类型已知时直调用）。
        //
        // 设计取舍（v0）：
        // - 仍以 "能在 vtable slot 表里命中" 作为是否需要虚调用的主判断（保证动态分发语义成立）；
        // - 仅在我们能证明该调用点为"单一目标"时，把 vtable 间接调用降级为 direct call；
        // - 当前实现优先覆盖最常见、最容易证明的 case：receiver 的静态类型对应的 class 在本次编译单元内
        //   **不存在任何子类**（等价于 final class / 单一实现的 sealed class）。
        //
        // 重要：这里优先尝试使用"局部绑定的原始 HIR 类型"（`env.local.hir_ty`）作为 receiver 静态类型。
        // - 这样可以避开 call-site 处的隐式 upcast/coerce 把 `receiver_expr.ty` 擦成父类的问题，
        //   让 "`val d: Derived = ...; d.ping()`" 这类典型场景能被正确去虚化；
        // - 若无法恢复精确类型，则回退到 `receiver_expr.ty`（保持保守正确性）。
        let devirt_receiver_ty = match &receiver_expr.kind {
            hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) => self
                .env
                .get(*id)
                .and_then(|l| l.hir_ty)
                .unwrap_or(receiver_expr.ty),
            _ => receiver_expr.ty,
        };
        if let Some(target_fqn) = self.try_devirtualize_class_vtable_call_target(
            devirt_receiver_ty,
            slot,
            method_name,
            explicit_params_len,
        ) {
            let v = self.codegen_top_level_fun_call(span, callee_span, &target_fqn, args)?;
            return Ok(Some(v));
        }

        let sig_fun =
            self.fun_index
                .get(fqn)
                .copied()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "vtable call callee type",
                    at: callee_span.into(),
                })?;

        if args.len() != sig_fun.params.len() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "vtable call arity mismatch",
                at: span.into(),
            });
        }

        // 1) 组装 indirect call 的 LLVM 函数类型与参数列表。
        let mut llvm_param_tys: Vec<BasicMetadataTypeEnum<'ctx>> =
            Vec::with_capacity(sig_fun.params.len());
        for p in &sig_fun.params {
            llvm_param_tys.push(self.llvm_param_ty(callee_span, p.ty)?);
        }

        let ret_cg =
            self.cg_ty_of(sig_fun.return_ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "vtable call return type",
                    at: span.into(),
                })?;

        let llvm_fun_ty = match ret_cg {
            CgTy::Unit | CgTy::Never => self.context.void_type().fn_type(&llvm_param_tys, false),
            other => self
                .llvm_basic_type_of(callee_span, other)?
                .fn_type(&llvm_param_tys, false),
        };

        // 2) 求值实参，并记录 receiver（第 0 个参数）用于 vtable slot lookup。
        let mut receiver_ptr: Option<PointerValue<'ctx>> = None;
        let mut llvm_args: Vec<inkwell::values::BasicMetadataValueEnum<'ctx>> =
            Vec::with_capacity(args.len());
        for (idx, arg) in args.iter().enumerate() {
            let hir::CallArg::Positional(expr) = arg else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "named vtable call arg",
                    at: span.into(),
                });
            };

            let param_ty = sig_fun.params[idx].ty;
            let target_cg = self
                .cg_ty_of(param_ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "vtable call arg type",
                    at: expr.span.into(),
                })?;

            let v = match &expr.kind {
                hir::ExprKind::Closure(closure) => {
                    self.codegen_closure_expr(expr.span, closure, param_ty)?
                }
                _ => self.codegen_expr_in_expected_context(expr, Some(target_cg))?,
            };
            let coerced = self.coerce_value(expr.span, v, target_cg)?;

            if idx == 0 {
                let Some(raw) = coerced.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "vtable call receiver value",
                        at: expr.span.into(),
                    });
                };
                let BasicValueEnum::PointerValue(ptr) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "vtable call receiver type",
                        at: expr.span.into(),
                    });
                };
                receiver_ptr = Some(ptr);
            }

            llvm_args.push(self.as_llvm_arg_value(expr.span, target_cg, coerced)?);
        }

        let receiver_ptr = receiver_ptr.ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "vtable call receiver",
            at: callee_span.into(),
        })?;

        // 3) 从 `this.header.type_desc.vtable[slot]` 取出目标函数指针并执行 indirect call。
        let fn_i8 = self.load_class_vtable_slot_fn_ptr_i8(span, receiver_ptr, slot)?;
        let typed_fn_ptr = self.builder.build_pointer_cast(
            fn_i8,
            self.llvm_ptr_type(AddressSpace::default()),
            "vtable_fn_typed",
        )?;

        let call_site = self.builder.build_indirect_call(
            llvm_fun_ty,
            typed_fn_ptr,
            &llvm_args,
            "call_vtable",
        )?;
        call_site.set_call_convention(self.llvm_call_convention_for_fqn(fqn));
        self.emit_ordinary_call_effect_propagation_check(span, "vtable_call_effect")?;

        // 4) 返回值装箱（保持与 `codegen_top_level_fun_call` 一致）。
        match ret_cg {
            CgTy::Unit => Ok(Some(CgValue::unit())),
            CgTy::Never => Ok(Some(CgValue::never())),
            _ => {
                let raw = call_site.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "vtable call return value",
                        at: span.into(),
                    },
                )?;
                Ok(Some(self.cg_value_from_loaded(span, ret_cg, raw)?))
            }
        }
    }

    fn try_devirtualize_class_vtable_call_target(
        &mut self,
        receiver_ty: TypeId,
        slot: u32,
        method_name: &str,
        explicit_params_len: u32,
    ) -> Option<String> {
        let receiver_fqn = match self.types.kind(receiver_ty) {
            TypeKind::Ref(RefTypeKind::Nominal(nominal)) => nominal.fqn.as_str(),
            _ => return None,
        };

        // 当前阶段（v0）只对"本次 codegen 侧已收集到 class init 信息"的 class 启用去虚化：
        // - `class_inits` 是后端可见的最小 class 元数据来源（字段/ctor/super 链等）；
        // - 对 sysroot 或其它未收集到 init 信息的 class，无法可靠判断其继承关系，保持保守回退到 vtable 调用。
        if !self.class_inits.contains_key(receiver_fqn) {
            return None;
        }

        // 若该 receiver class 在编译单元内存在子类，则该调用点仍可能动态分发到 override 目标，
        // 不应在后端做直调用替换。
        let has_known_subclass = self
            .class_inits
            .values()
            .any(|c| c.super_class_fqn.as_deref() == Some(receiver_fqn));
        if has_known_subclass {
            return None;
        }

        let receiver_slots = self.class_vtables.get(receiver_fqn)?;
        let slot_entry = receiver_slots.get(slot as usize)?;
        if slot_entry.name != method_name || slot_entry.params_len != explicit_params_len {
            return None;
        }

        // 只在目标成员为"可 codegen 的函数实体"时启用去虚化：
        // - 普通成员函数：要求有 body；
        // - @Extern：允许无 body（由链接器提供实现）。
        let target_fqn = slot_entry.impl_member_fqn.as_str();
        let target_fun = self.fun_index.get(target_fqn).copied()?;
        if target_fun.body.is_none() && !self.extern_funs.contains_key(target_fqn) {
            return None;
        }

        Some(target_fqn.to_string())
    }

    fn try_codegen_interface_itable_call(
        &mut self,
        span: crate::span::Span,
        callee_span: crate::span::Span,
        fqn: &str,
        args: &[hir::CallArg],
    ) -> Result<Option<CgValue<'ctx>>, LlvmEmitError> {
        let Some((owner_fqn, method_name)) = fqn.rsplit_once('.') else {
            return Ok(None);
        };

        let Some(iface) = self.interfaces.get(owner_fqn) else {
            return Ok(None);
        };

        if args.is_empty() {
            return Ok(None);
        }

        let Some((receiver_arg, _call_args)) = args.split_first() else {
            return Ok(None);
        };
        let hir::CallArg::Positional(_receiver_expr) = receiver_arg else {
            return Ok(None);
        };

        // v0：slot key 仅用 name + arity（不含 receiver）；interface 内必须唯一。
        let explicit_params_len = args.len().saturating_sub(1) as u32;
        let mut candidates = iface
            .method_slots
            .iter()
            .filter(|s| s.name == method_name && s.params_len == explicit_params_len);
        let Some(first) = candidates.next() else {
            return Ok(None);
        };
        if candidates.next().is_some() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "itable call slot ambiguous",
                at: callee_span.into(),
            });
        }
        let slot = first.slot;

        let sig_fun =
            self.fun_index
                .get(fqn)
                .copied()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "itable call callee type",
                    at: callee_span.into(),
                })?;

        if args.len() != sig_fun.params.len() {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "itable call arity mismatch",
                at: span.into(),
            });
        }

        // 1) 组装 indirect call 的 LLVM 函数类型与参数列表。
        let mut llvm_param_tys: Vec<BasicMetadataTypeEnum<'ctx>> =
            Vec::with_capacity(sig_fun.params.len());
        for p in &sig_fun.params {
            llvm_param_tys.push(self.llvm_param_ty(callee_span, p.ty)?);
        }

        let ret_cg =
            self.cg_ty_of(sig_fun.return_ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "itable call return type",
                    at: span.into(),
                })?;

        let llvm_fun_ty = match ret_cg {
            CgTy::Unit | CgTy::Never => self.context.void_type().fn_type(&llvm_param_tys, false),
            other => self
                .llvm_basic_type_of(callee_span, other)?
                .fn_type(&llvm_param_tys, false),
        };

        // 2) 求值实参，并记录 receiver（第 0 个参数）用于 itable lookup。
        let mut receiver_ptr: Option<PointerValue<'ctx>> = None;
        let mut llvm_args: Vec<inkwell::values::BasicMetadataValueEnum<'ctx>> =
            Vec::with_capacity(args.len());
        for (idx, arg) in args.iter().enumerate() {
            let hir::CallArg::Positional(expr) = arg else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "named itable call arg",
                    at: span.into(),
                });
            };

            let param_ty = sig_fun.params[idx].ty;
            let target_cg = self
                .cg_ty_of(param_ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "itable call arg type",
                    at: expr.span.into(),
                })?;

            let v = match &expr.kind {
                hir::ExprKind::Closure(closure) => {
                    self.codegen_closure_expr(expr.span, closure, param_ty)?
                }
                _ => self.codegen_expr_in_expected_context(expr, Some(target_cg))?,
            };
            let coerced = self.coerce_value(expr.span, v, target_cg)?;

            if idx == 0 {
                let Some(raw) = coerced.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "itable call receiver value",
                        at: expr.span.into(),
                    });
                };
                let BasicValueEnum::PointerValue(ptr) = raw else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "itable call receiver type",
                        at: expr.span.into(),
                    });
                };
                receiver_ptr = Some(ptr);
            }

            llvm_args.push(self.as_llvm_arg_value(expr.span, target_cg, coerced)?);
        }

        let receiver_ptr = receiver_ptr.ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "itable call receiver",
            at: callee_span.into(),
        })?;

        // 3) 从 `this.header.type_desc.itable` 查找 interface entry 并取出 `methods[slot]`。
        let fn_i8 = self.load_interface_itable_slot_fn_ptr_i8(
            span,
            receiver_ptr,
            iface.interface_id,
            slot,
        )?;

        // 防御：不应发生（typecheck 已保证实现存在）；若发生，直接退出避免 indirect call on NULL。
        let fn_is_null = self.builder.build_is_null(fn_i8, "itable_fn_is_null")?;
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
        let ok_bb = self.context.append_basic_block(func, "itable_fn_ok");
        let bad_bb = self.context.append_basic_block(func, "itable_fn_null");
        self.builder
            .build_conditional_branch(fn_is_null, bad_bb, ok_bb)?;
        self.builder.position_at_end(bad_bb);
        let exit = self.declare_libc_exit();
        let code = self.context.i32_type().const_int(7, false);
        let _ = self
            .builder
            .build_call(exit, &[code.into()], "itable_fn_null_exit")?;
        self.builder.build_unreachable()?;
        self.builder.position_at_end(ok_bb);

        let typed_fn_ptr = self.builder.build_pointer_cast(
            fn_i8,
            self.llvm_ptr_type(AddressSpace::default()),
            "itable_fn_typed",
        )?;

        let call_site = self.builder.build_indirect_call(
            llvm_fun_ty,
            typed_fn_ptr,
            &llvm_args,
            "call_itable",
        )?;
        call_site.set_call_convention(self.llvm_call_convention_for_fqn(fqn));
        self.emit_ordinary_call_effect_propagation_check(span, "itable_call_effect")?;

        // 4) 返回值装箱（保持与 `codegen_top_level_fun_call` 一致）。
        match ret_cg {
            CgTy::Unit => Ok(Some(CgValue::unit())),
            CgTy::Never => Ok(Some(CgValue::never())),
            _ => {
                let raw = call_site.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "itable call return value",
                        at: span.into(),
                    },
                )?;
                Ok(Some(self.cg_value_from_loaded(span, ret_cg, raw)?))
            }
        }
    }

    fn load_class_vtable_slot_fn_ptr_i8(
        &mut self,
        _at: crate::span::Span,
        receiver: PointerValue<'ctx>,
        slot: u32,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let i32_ty = self.context.i32_type();
        let i8_ptr_ty = self.llvm_i8_ptr_type();

        // receiver 指向对象头起始地址：先把它 cast 为 `ScoopGcObjectHeader*`。
        let header_ty = self.llvm_gc_object_header_type();
        let header_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
        let header_ptr =
            self.builder
                .build_pointer_cast(receiver, header_ptr_ty, "vtable_hdr_ptr")?;

        // header.type_desc : i8*
        let type_desc_ptr =
            self.builder
                .build_struct_gep(header_ty, header_ptr, 1, "vtable_type_desc_gep")?;
        let type_desc_i8 = self
            .builder
            .build_load(i8_ptr_ty, type_desc_ptr, "load_type_desc")?
            .into_pointer_value();

        // type_desc.vtable : i8*
        let desc_ty = self.llvm_scoop_type_descriptor_type();
        let desc_ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        let desc_ptr = self
            .builder
            .build_pointer_cast(type_desc_i8, desc_ptr_ty, "type_desc")?;
        let vtable_field_ptr =
            self.builder
                .build_struct_gep(desc_ty, desc_ptr, 13, "type_desc_vtable_gep")?;
        let vtable_i8 = self
            .builder
            .build_load(i8_ptr_ty, vtable_field_ptr, "load_vtable")?
            .into_pointer_value();

        // vtable[slot] : i8*（函数指针）
        let vtable_entries_ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        let vtable_entries =
            self.builder
                .build_pointer_cast(vtable_i8, vtable_entries_ptr_ty, "vtable_entries")?;
        let slot_idx = i32_ty.const_int(slot as u64, false);
        let slot_ptr = unsafe {
            self.builder.build_in_bounds_gep(
                i8_ptr_ty,
                vtable_entries,
                &[slot_idx],
                "vtable_slot_ptr",
            )?
        };
        let fn_i8 = self
            .builder
            .build_load(i8_ptr_ty, slot_ptr, "load_vtable_fn")?
            .into_pointer_value();

        Ok(fn_i8)
    }

    fn llvm_scoop_itable_entry_type(&self) -> StructType<'ctx> {
        const TY_NAME: &str = "scoop.runtime.ScoopItableEntry";
        if let Some(existing) = self.context.get_struct_type(TY_NAME) {
            return existing;
        }

        let ty = self.context.opaque_struct_type(TY_NAME);
        let i64_ty = self.context.i64_type();
        let i8_ptr_ty = self.llvm_i8_ptr_type();
        ty.set_body(&[i64_ty.into(), i8_ptr_ty.into()], false);
        ty
    }

    fn llvm_scoop_itable_type(&self) -> StructType<'ctx> {
        // flexible array 模型：entries 使用 `[0 x Entry]`，仅用于 GEP/字段偏移计算。
        const TY_NAME: &str = "scoop.runtime.ScoopItable";
        if let Some(existing) = self.context.get_struct_type(TY_NAME) {
            return existing;
        }

        let ty = self.context.opaque_struct_type(TY_NAME);
        let i32_ty = self.context.i32_type();
        let entry_ty = self.llvm_scoop_itable_entry_type();
        let entries_ty = entry_ty.array_type(0);
        ty.set_body(&[i32_ty.into(), i32_ty.into(), entries_ty.into()], false);
        ty
    }

    fn load_interface_itable_slot_fn_ptr_i8(
        &mut self,
        at: crate::span::Span,
        receiver: PointerValue<'ctx>,
        interface_id: u64,
        slot: u32,
    ) -> Result<PointerValue<'ctx>, LlvmEmitError> {
        let i32_ty = self.context.i32_type();
        let i64_ty = self.context.i64_type();
        let i8_ptr_ty = self.llvm_i8_ptr_type();

        // receiver 指向对象头起始地址：先把它 cast 为 `ScoopGcObjectHeader*`。
        let header_ty = self.llvm_gc_object_header_type();
        let header_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
        let header_ptr =
            self.builder
                .build_pointer_cast(receiver, header_ptr_ty, "itable_hdr_ptr")?;

        // header.type_desc : i8*
        let type_desc_ptr =
            self.builder
                .build_struct_gep(header_ty, header_ptr, 1, "itable_type_desc_gep")?;
        let type_desc_i8 = self
            .builder
            .build_load(i8_ptr_ty, type_desc_ptr, "load_type_desc")?
            .into_pointer_value();

        // type_desc.itable : i8*
        let desc_ty = self.llvm_scoop_type_descriptor_type();
        let desc_ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        let desc_ptr = self
            .builder
            .build_pointer_cast(type_desc_i8, desc_ptr_ty, "type_desc")?;
        let itable_field_ptr =
            self.builder
                .build_struct_gep(desc_ty, desc_ptr, 12, "type_desc_itable_gep")?;
        let itable_i8 = self
            .builder
            .build_load(i8_ptr_ty, itable_field_ptr, "load_itable")?
            .into_pointer_value();

        // itable == NULL：直接返回 NULL（caller 负责防御）。
        let itable_is_null = self.builder.build_is_null(itable_i8, "itable_is_null")?;

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
        let null_bb = self.context.append_basic_block(func, "itable_null");
        let lookup_bb = self.context.append_basic_block(func, "itable_lookup");
        let done_bb = self.context.append_basic_block(func, "itable_done");
        self.builder
            .build_conditional_branch(itable_is_null, null_bb, lookup_bb)?;

        // null -> done（返回 NULL）。
        self.builder.position_at_end(null_bb);
        self.builder.build_unconditional_branch(done_bb)?;

        // lookup -> 扫描 entries 查找 interface_id。
        self.builder.position_at_end(lookup_bb);
        let itable_ty = self.llvm_scoop_itable_type();
        let itable_ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        let itable_ptr = self
            .builder
            .build_pointer_cast(itable_i8, itable_ptr_ty, "itable_ptr")?;

        let len_ptr = self
            .builder
            .build_struct_gep(itable_ty, itable_ptr, 0, "itable_len_gep")?;
        let len_i32 = self
            .builder
            .build_load(i32_ty, len_ptr, "itable_len")?
            .into_int_value();

        let entry_ty = self.llvm_scoop_itable_entry_type();
        let entries_field_ptr =
            self.builder
                .build_struct_gep(itable_ty, itable_ptr, 2, "itable_entries_gep")?;
        let entry_ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        let entries_base =
            self.builder
                .build_pointer_cast(entries_field_ptr, entry_ptr_ty, "itable_entries")?;

        let loop_bb = self.context.append_basic_block(func, "itable_loop");
        let found_bb = self.context.append_basic_block(func, "itable_found");
        let not_found_bb = self.context.append_basic_block(func, "itable_not_found");

        self.builder.build_unconditional_branch(loop_bb)?;
        self.builder.position_at_end(loop_bb);

        let idx_phi = self.builder.build_phi(i32_ty, "itable_idx")?;
        idx_phi.add_incoming(&[(&i32_ty.const_zero(), lookup_bb)]);
        let idx_i32 = idx_phi.as_basic_value().into_int_value();

        let cond = self.builder.build_int_compare(
            IntPredicate::ULT,
            idx_i32,
            len_i32,
            "itable_idx_lt_len",
        )?;
        self.builder
            .build_conditional_branch(cond, found_bb, not_found_bb)?;

        // found_bb：检查 entries[idx].interface_id 是否匹配；不匹配则 idx++ 回到 loop。
        self.builder.position_at_end(found_bb);
        let entry_ptr = unsafe {
            self.builder.build_in_bounds_gep(
                entry_ty,
                entries_base,
                &[idx_i32],
                "itable_entry_ptr",
            )?
        };
        let id_ptr =
            self.builder
                .build_struct_gep(entry_ty, entry_ptr, 0, "itable_entry_id_gep")?;
        let id_i64 = self
            .builder
            .build_load(i64_ty, id_ptr, "itable_entry_id")?
            .into_int_value();

        let target_id = i64_ty.const_int(interface_id, false);
        let id_ok =
            self.builder
                .build_int_compare(IntPredicate::EQ, id_i64, target_id, "itable_id_eq")?;

        let hit_bb = self.context.append_basic_block(func, "itable_hit");
        let miss_bb = self.context.append_basic_block(func, "itable_miss");
        self.builder
            .build_conditional_branch(id_ok, hit_bb, miss_bb)?;

        // miss_bb：idx++ 继续。
        self.builder.position_at_end(miss_bb);
        let next =
            self.builder
                .build_int_add(idx_i32, i32_ty.const_int(1, false), "itable_idx_next")?;
        idx_phi.add_incoming(&[(&next, miss_bb)]);
        self.builder.build_unconditional_branch(loop_bb)?;

        // hit_bb：取 methods 指针并跳到 done_bb。
        self.builder.position_at_end(hit_bb);
        let methods_ptr =
            self.builder
                .build_struct_gep(entry_ty, entry_ptr, 1, "itable_entry_methods_gep")?;
        let methods_i8 = self
            .builder
            .build_load(i8_ptr_ty, methods_ptr, "itable_entry_methods")?
            .into_pointer_value();
        self.builder.build_unconditional_branch(done_bb)?;

        // not_found_bb：直接到 done_bb（返回 NULL）。
        self.builder.position_at_end(not_found_bb);
        self.builder.build_unconditional_branch(done_bb)?;

        // done_bb：phi 合并 methods 指针（hit -> methods；其它 -> NULL），再取 methods[slot]。
        self.builder.position_at_end(done_bb);
        let methods_phi = self.builder.build_phi(i8_ptr_ty, "itable_methods")?;
        methods_phi.add_incoming(&[
            (&i8_ptr_ty.const_null(), null_bb),
            (&i8_ptr_ty.const_null(), not_found_bb),
            (&methods_i8, hit_bb),
        ]);
        let methods_i8 = methods_phi.as_basic_value().into_pointer_value();

        // methods == NULL：直接返回 NULL（caller 负责防御），避免解引用 NULL。
        let methods_is_null = self
            .builder
            .build_is_null(methods_i8, "itable_methods_is_null")?;
        let slot_null_bb = self.context.append_basic_block(func, "itable_slot_null");
        let slot_ok_bb = self.context.append_basic_block(func, "itable_slot_ok");
        let slot_done_bb = self.context.append_basic_block(func, "itable_slot_done");
        self.builder
            .build_conditional_branch(methods_is_null, slot_null_bb, slot_ok_bb)?;

        self.builder.position_at_end(slot_null_bb);
        self.builder.build_unconditional_branch(slot_done_bb)?;

        self.builder.position_at_end(slot_ok_bb);
        let methods_ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        let methods_entries = self.builder.build_pointer_cast(
            methods_i8,
            methods_ptr_ty,
            "itable_methods_entries",
        )?;
        let slot_idx = i32_ty.const_int(slot as u64, false);
        let slot_ptr = unsafe {
            self.builder.build_in_bounds_gep(
                i8_ptr_ty,
                methods_entries,
                &[slot_idx],
                "itable_slot_ptr",
            )?
        };
        let fn_i8 = self
            .builder
            .build_load(i8_ptr_ty, slot_ptr, "load_itable_fn")?
            .into_pointer_value();
        self.builder.build_unconditional_branch(slot_done_bb)?;

        self.builder.position_at_end(slot_done_bb);
        let fn_phi = self.builder.build_phi(i8_ptr_ty, "itable_fn_i8")?;
        fn_phi.add_incoming(&[
            (&i8_ptr_ty.const_null(), slot_null_bb),
            (&fn_i8, slot_ok_bb),
        ]);
        let fn_i8 = fn_phi.as_basic_value().into_pointer_value();

        Ok(fn_i8)
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

        let ret_cg = self
            .cg_ty_of(fun_ty.return_ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "funptr call return type",
                at: callee_span.into(),
            })?;
        let hidden_sret_result_ty = self.hidden_sret_result_ty(callee_span, ret_cg)?;

        // 1) 组装 indirect call 的 LLVM 函数类型与参数列表。
        let mut llvm_param_tys: Vec<BasicMetadataTypeEnum<'ctx>> =
            Vec::with_capacity(fun_ty.params.len() + usize::from(hidden_sret_result_ty.is_some()));
        if let Some(result_ty) = hidden_sret_result_ty {
            let _ = result_ty;
            llvm_param_tys.push(self.context.ptr_type(AddressSpace::default()).into());
        }
        for ty in &fun_ty.params {
            llvm_param_tys.push(self.llvm_param_ty(callee_span, *ty)?);
        }

        let llvm_fun_ty = match (hidden_sret_result_ty, ret_cg) {
            (Some(_), _) | (None, CgTy::Unit | CgTy::Never) => {
                self.context.void_type().fn_type(&llvm_param_tys, false)
            }
            (None, other) => self
                .llvm_basic_type_of(callee_span, other)?
                .fn_type(&llvm_param_tys, false),
        };

        // 2) 将 `word-sized address` 转换为函数指针。
        //
        // 说明：
        // - 当前阶段我们把 `scoop.unsafe.FunPtr<F>` 视为 "opaque native function address"；
        // - `fp(args...)` 会在 codegen 阶段执行 `inttoptr` 并生成 indirect call；
        // - v0 阶段仅支持 C ABI（callconv 0）。
        let fun_ptr_ty = self.llvm_ptr_type(AddressSpace::default());
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
        let typed_fn_ptr =
            self.builder
                .build_int_to_ptr(casted_addr, fun_ptr_ty, "funptr_typed")?;

        // 3) 求值实参并执行调用。
        let mut llvm_args: Vec<inkwell::values::BasicMetadataValueEnum<'ctx>> =
            Vec::with_capacity(args.len() + usize::from(hidden_sret_result_ty.is_some()));
        let sret_result_slot = if hidden_sret_result_ty.is_some() {
            let slot = self.create_entry_alloca(callee_span, "funptr_call_sret", ret_cg)?;
            llvm_args.push(slot.into());
            Some(slot)
        } else {
            None
        };
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
        if let Some(result_ty) = hidden_sret_result_ty {
            self.add_sret_attribute_to_call(call_site, 0, result_ty);
        }
        call_site.set_call_convention(0);
        self.emit_ordinary_call_effect_propagation_check(span, "funptr_call_effect")?;

        match ret_cg {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Never => Ok(CgValue::never()),
            _ => {
                if let Some(result_ptr) = sret_result_slot {
                    self.load_sret_result_from_ptr(span, ret_cg, result_ptr)
                } else {
                    let raw = call_site.try_as_basic_value().basic().ok_or(
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "funptr call return value",
                            at: span.into(),
                        },
                    )?;
                    self.cg_value_from_loaded(span, ret_cg, raw)
                }
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
        let expected_arity = fun_ty.params.len() + usize::from(fun_ty.receiver.is_some());
        if args.len() != expected_arity {
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
        let closure_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
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
            .build_load(gc_i8_ptr_ty, env_ptr_gep, "closure_env")?
            .into_pointer_value();
        let fn_ptr_raw = self
            .builder
            .build_load(i8_ptr_ty, fn_ptr_gep, "closure_fn")?
            .into_pointer_value();

        let ret_cg = self
            .cg_ty_of(fun_ty.return_ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "function value call return type",
                at: callee_span.into(),
            })?;
        let hidden_sret_result_ty = self.hidden_sret_result_ty(callee_span, ret_cg)?;

        // 2) 组装 indirect call 的 LLVM 函数类型与参数。
        let mut llvm_param_tys: Vec<BasicMetadataTypeEnum<'ctx>> =
            Vec::with_capacity(1 + expected_arity + usize::from(hidden_sret_result_ty.is_some()));
        if let Some(result_ty) = hidden_sret_result_ty {
            let _ = result_ty;
            llvm_param_tys.push(self.context.ptr_type(AddressSpace::default()).into());
        }
        llvm_param_tys.push(gc_i8_ptr_ty.into());
        if let Some(receiver_ty) = fun_ty.receiver {
            llvm_param_tys.push(self.llvm_param_ty(callee_span, receiver_ty)?);
        }
        for ty in &fun_ty.params {
            llvm_param_tys.push(self.llvm_param_ty(callee_span, *ty)?);
        }

        let llvm_fun_ty = match (hidden_sret_result_ty, ret_cg) {
            (Some(_), _) | (None, CgTy::Unit | CgTy::Never) => {
                self.context.void_type().fn_type(&llvm_param_tys, false)
            }
            (None, CgTy::Bool) => self.context.bool_type().fn_type(&llvm_param_tys, false),
            (None, CgTy::Float64) => self.context.f64_type().fn_type(&llvm_param_tys, false),
            (None, CgTy::Float32) => self.context.f32_type().fn_type(&llvm_param_tys, false),
            (None, CgTy::Int(int_ty)) => self.int_type(int_ty).fn_type(&llvm_param_tys, false),
            (None, CgTy::String) => self
                .llvm_scoop_string_ptr_type()
                .fn_type(&llvm_param_tys, false),
            (None, CgTy::Ref) => gc_i8_ptr_ty.fn_type(&llvm_param_tys, false),
            (None, CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_)) => unreachable!(
                "aggregate function-value returns should have been lowered through hidden sret"
            ),
        };

        let typed_fn_ptr = self.builder.build_pointer_cast(
            fn_ptr_raw,
            self.llvm_ptr_type(AddressSpace::default()),
            "closure_fn_typed",
        )?;

        // 3) 求值实参并执行调用（env 作为第一个参数）。
        let mut llvm_args: Vec<inkwell::values::BasicMetadataValueEnum<'ctx>> =
            Vec::with_capacity(1 + args.len() + usize::from(hidden_sret_result_ty.is_some()));
        let sret_result_slot = if hidden_sret_result_ty.is_some() {
            let slot = self.create_entry_alloca(callee_span, "closure_call_sret", ret_cg)?;
            llvm_args.push(slot.into());
            Some(slot)
        } else {
            None
        };
        llvm_args.push(env_ptr.into());
        for (idx, arg) in args.iter().enumerate() {
            let hir::CallArg::Positional(expr) = arg else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "named function value call arg",
                    at: span.into(),
                });
            };

            let param_ty = match fun_ty.receiver {
                Some(receiver_ty) if idx == 0 => receiver_ty,
                Some(_) => fun_ty.params[idx - 1],
                None => fun_ty.params[idx],
            };
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
        if let Some(result_ty) = hidden_sret_result_ty {
            self.add_sret_attribute_to_call(call_site, 0, result_ty);
        }
        self.emit_ordinary_call_effect_propagation_check(span, "closure_call_effect")?;

        match ret_cg {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Never => Ok(CgValue::never()),
            _ => {
                if let Some(result_ptr) = sret_result_slot {
                    self.load_sret_result_from_ptr(span, ret_cg, result_ptr)
                } else {
                    let raw = call_site.try_as_basic_value().basic().ok_or(
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "function value call return value",
                            at: span.into(),
                        },
                    )?;
                    self.cg_value_from_loaded(span, ret_cg, raw)
                }
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
        // 注意：我们会在"第一次 codegen 到该 lambda 表达式"时生成其函数体；之后复用同名符号。
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
            let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
            let ret_cg =
                self.cg_ty_of(fun_ty.return_ty)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "lambda return type",
                        at: span.into(),
                    })?;
            let hidden_sret_result_ty = self.hidden_sret_result_ty(span, ret_cg)?;
            let mut llvm_param_tys: Vec<BasicMetadataTypeEnum<'ctx>> = Vec::with_capacity(
                1 + fun_ty.params.len()
                    + usize::from(fun_ty.receiver.is_some())
                    + usize::from(hidden_sret_result_ty.is_some()),
            );
            if let Some(result_ty) = hidden_sret_result_ty {
                let _ = result_ty;
                llvm_param_tys.push(self.context.ptr_type(AddressSpace::default()).into());
            }
            // env ptr：GC-managed 引用（closure env 是一个 heap object）。
            llvm_param_tys.push(gc_i8_ptr_ty.into());
            if let Some(receiver_ty) = fun_ty.receiver {
                llvm_param_tys.push(self.llvm_param_ty(span, receiver_ty)?);
            }
            for ty in &fun_ty.params {
                llvm_param_tys.push(self.llvm_param_ty(span, *ty)?);
            }

            let fn_ty = match (hidden_sret_result_ty, ret_cg) {
                (Some(_), _) | (None, CgTy::Unit | CgTy::Never) => {
                    self.context.void_type().fn_type(&llvm_param_tys, false)
                }
                (None, CgTy::Bool) => self.context.bool_type().fn_type(&llvm_param_tys, false),
                (None, CgTy::Float64) => self.context.f64_type().fn_type(&llvm_param_tys, false),
                (None, CgTy::Float32) => self.context.f32_type().fn_type(&llvm_param_tys, false),
                (None, CgTy::Int(int_ty)) => self.int_type(int_ty).fn_type(&llvm_param_tys, false),
                (None, CgTy::String) => self
                    .llvm_scoop_string_ptr_type()
                    .fn_type(&llvm_param_tys, false),
                (None, CgTy::Ref) => gc_i8_ptr_ty.fn_type(&llvm_param_tys, false),
                (None, CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_)) => unreachable!(
                    "aggregate lambda returns should have been lowered through hidden sret"
                ),
            };

            let llvm_fun = self.module.add_function(&fun_name, fn_ty, None);
            llvm_fun.set_call_conventions(0);
            llvm_fun.set_gc(super::LLVM_GC_STRATEGY_STATEPOINT_EXAMPLE);
            if let Some(result_ty) = hidden_sret_result_ty {
                self.add_sret_attribute_to_function(llvm_fun, 0, result_ty);
            }

            let mut cg = MainCodegen::new(MainCodegenInputs {
                context: self.context,
                module: self.module,
                builder: self.builder,
                target_data: self.target_data,
                host: self.host,
                source_map: self.source_map,
                entry_source_id: self.entry_source_id,
                types: self.types,
                struct_layouts: self.struct_layouts,
                enum_layouts: self.enum_layouts,
                top_level_vars: self.top_level_vars,
                top_level_consts: self.top_level_consts,
                object_inits: self.object_inits,
                class_inits: self.class_inits,
                class_vtables: self.class_vtables,
                interfaces: self.interfaces,
                class_itables: self.class_itables,
                ctor_call_sites: self.ctor_call_sites,
                continuation_resume_call_sites: self.continuation_resume_call_sites,
                extern_funs: self.extern_funs,
                fun_index: self.fun_index,
                effect_op_tags: Rc::clone(&self.effect_op_tags),
            });
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

        let size_v = self.context.i64_type().const_int(obj_size_bytes, false);

        let closure_desc = self.get_or_create_closure_object_type_desc_global(span)?;
        let closure_desc_i8 = self.builder.build_pointer_cast(
            closure_desc.as_pointer_value(),
            self.llvm_i8_ptr_type(),
            "closure_type_desc_i8",
        )?;
        let rt_alloc = self.declare_runtime_alloc_typed();
        let call = self.builder.build_call(
            rt_alloc,
            &[closure_desc_i8.into(), size_v.into()],
            "rt_alloc_closure",
        )?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc_typed return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(obj_i8) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc_typed return type",
                at: span.into(),
            });
        };

        let i8_ptr_ty = self.llvm_i8_ptr_type();
        let gc_i8_ptr_ty = self.llvm_gc_i8_ptr_type();
        let obj_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
        let obj_ptr = self
            .builder
            .build_pointer_cast(obj_i8, obj_ptr_ty, "closure_obj_ptr")?;

        let env_gep =
            self.builder
                .build_struct_gep(closure_obj_ty, obj_ptr, 1, "closure_env_gep")?;
        let fn_gep = self
            .builder
            .build_struct_gep(closure_obj_ty, obj_ptr, 2, "closure_fn_gep")?;

        // 重要：先把 env_ptr 初始化为 NULL。
        //
        // 说明：
        // - closure object 的 type descriptor 会把 `env_ptr` 视为 GC pointer slot；
        // - 若在分配 env 期间发生 safepoint/GC，则必须避免扫描到未初始化的垃圾值。
        let _ = self.store_local_value(
            span,
            env_gep,
            CgTy::Ref,
            CgValue {
                ty: CgTy::Ref,
                value: Some(gc_i8_ptr_ty.const_null().into()),
            },
        )?;

        // 若有捕获，则分配 env 并写入捕获值；否则 env_ptr 为 NULL。
        let env_i8 = if captures.is_empty() {
            gc_i8_ptr_ty.const_null()
        } else {
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

            let size_v = self.context.i64_type().const_int(env_size_bytes, false);

            let env_desc =
                self.get_or_create_closure_env_type_desc_global(span, closure.id, env_ty)?;
            let env_desc_i8 = self.builder.build_pointer_cast(
                env_desc.as_pointer_value(),
                self.llvm_i8_ptr_type(),
                "closure_env_type_desc_i8",
            )?;
            let rt_alloc = self.declare_runtime_alloc_typed();
            let call = self.builder.build_call(
                rt_alloc,
                &[env_desc_i8.into(), size_v.into()],
                "rt_alloc_closure_env",
            )?;
            let raw =
                call.try_as_basic_value()
                    .basic()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "scoop_alloc_typed return value",
                        at: span.into(),
                    })?;
            let BasicValueEnum::PointerValue(env_i8) = raw else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "scoop_alloc_typed return type",
                    at: span.into(),
                });
            };

            let env_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
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

                let cg_ty = self
                    .cg_ty_of(*ty_id)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
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
                let loaded =
                    self.builder
                        .build_load(llvm_ty, local.ptr, &format!("capture_load_{name}"))?;

                let field_gep = self.builder.build_struct_gep(
                    env_ty,
                    env_ptr,
                    (idx + 1) as u32,
                    &format!("capture_gep_{name}"),
                )?;
                let v = if cg_ty == CgTy::Unit {
                    CgValue::unit()
                } else {
                    CgValue {
                        ty: cg_ty,
                        value: Some(loaded),
                    }
                };
                let _ = self.store_local_value(span, field_gep, cg_ty, v)?;
            }

            env_i8
        };
        let _ = self.store_local_value(
            span,
            env_gep,
            CgTy::Ref,
            CgValue {
                ty: CgTy::Ref,
                value: Some(env_i8.into()),
            },
        )?;

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
    ) -> Result<ClosureParamBindings, LlvmEmitError> {
        // 显式 params：`{ x -> ... }`
        //
        // 说明：
        // - receiver function type 的 receiver 不出现在 lambda params 列表里；
        // - 因此这里始终只按"非 receiver 形参"做显式 params / 隐式 `it` 绑定。
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

        self.env.push_scope();

        // 入口的返回类型由期望函数类型决定（用于 Raise 的"早退默认值"）。
        let declared_return_cg =
            self.cg_ty_of(fun_ty.return_ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "lambda return type",
                    at: closure.span.into(),
                })?;
        self.current_fun_return_ty = Some(declared_return_cg);
        let uses_hidden_sret = self
            .hidden_sret_result_ty(closure.span, declared_return_cg)?
            .is_some();
        self.current_sret_return_ptr = if uses_hidden_sret {
            Some(
                llvm_fun
                    .get_nth_param(0)
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "missing llvm lambda sret param",
                        at: closure.span.into(),
                    })?
                    .into_pointer_value(),
            )
        } else {
            None
        };
        let env_param_index = u32::from(uses_hidden_sret);

        // captures：从 env（第 0 个 LLVM param）读取并绑定为 locals。
        if !capture_bindings.is_empty() {
            let env_i8 = llvm_fun
                .get_nth_param(env_param_index)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "missing llvm lambda env param",
                    at: closure.span.into(),
                })?
                .into_pointer_value();

            let env_ty = self.llvm_closure_env_type(closure.span, closure.id, capture_bindings)?;
            let env_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
            let env_ptr = self
                .builder
                .build_pointer_cast(env_i8, env_ptr_ty, "closure_env_ptr")?;

            for (idx, (id, name, ty_id)) in capture_bindings.iter().enumerate() {
                let target_ty =
                    self.cg_ty_of(*ty_id)
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
                    (idx + 1) as u32,
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
                let _stored = self.store_local_value(closure.span, ptr, target_ty, init)?;

                self.env.insert(
                    *id,
                    CgLocal {
                        hir_ty: Some(*ty_id),
                        ty: target_ty,
                        ptr,
                        mutable: false,
                    },
                );
            }
        }

        // params：env 固定占用第 0 个 LLVM param；若函数类型带 receiver，则第 1 个 LLVM param
        // 为 receiver，用户显式声明的 params 从其后开始。
        let llvm_param_offset = env_param_index + 1 + u32::from(fun_ty.receiver.is_some());
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
                CgTy::Never => CgValue::never(),
                CgTy::Bool => {
                    let raw = llvm_fun
                        .get_nth_param(idx as u32 + llvm_param_offset)
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "missing llvm lambda param",
                            at: closure.span.into(),
                        })?
                        .into_int_value();
                    CgValue::bool(raw)
                }
                CgTy::Float64 | CgTy::Float32 => {
                    let raw = llvm_fun
                        .get_nth_param(idx as u32 + llvm_param_offset)
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "missing llvm lambda param",
                            at: closure.span.into(),
                        })?
                        .into_float_value();
                    CgValue::float(raw, target_ty)
                }
                CgTy::Int(int_ty) => {
                    let raw = llvm_fun
                        .get_nth_param(idx as u32 + llvm_param_offset)
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "missing llvm lambda param",
                            at: closure.span.into(),
                        })?
                        .into_int_value();
                    CgValue::int(raw, int_ty)
                }
                CgTy::String => {
                    let raw = llvm_fun
                        .get_nth_param(idx as u32 + llvm_param_offset)
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
                        .get_nth_param(idx as u32 + llvm_param_offset)
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

            let _stored = self.store_local_value(closure.span, ptr, target_ty, init)?;

            self.env.insert(
                *id,
                CgLocal {
                    hir_ty: Some(*ty_id),
                    ty: target_ty,
                    ptr,
                    mutable: false,
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

        self.current_sret_return_ptr = None;
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
        // closure env 是 GC-managed heap object：以对象头开头，再跟 capture 字段。
        let header_ty = self.llvm_gc_object_header_type();
        let mut fields: Vec<BasicTypeEnum<'ctx>> = Vec::with_capacity(1 + capture_bindings.len());
        fields.push(header_ty.into());
        for (_id, _name, ty_id) in capture_bindings {
            let cg_ty = self
                .cg_ty_of(*ty_id)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
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

        self.build_enum_value(span, enum_ty, tag, CgEnumPayload::default())
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

        // 先把所有实参在"字段期望类型"下 codegen 并做最小 coercion，避免后续重复走 codegen。
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

            // GC safety（T1516）：
            // - boxed payload 不能暂存在栈上然后把栈指针塞进 enum 的 word payload；
            //   否则其中的 GC refs 无法被 stackmap/bitmap 扫描，触发 GC 后会出现悬挂指针。
            // - 因此 boxed payload 必须是一个 GC-managed heap object，并把对象指针写入 enum 的
            //   GC pointer slot（payload_ptr）。
            let payload_obj_ty =
                self.llvm_enum_boxed_payload_object_type(span, enum_ty, &variant)?;
            let obj_size_bytes = self.target_data.get_store_size(&payload_obj_ty);
            let size_v = self.context.i64_type().const_int(obj_size_bytes, false);

            let desc = self.get_or_create_enum_boxed_payload_type_desc_global(
                span,
                enum_ty,
                &variant,
                payload_obj_ty,
            )?;
            let desc_i8 = self.builder.build_pointer_cast(
                desc.as_pointer_value(),
                self.llvm_i8_ptr_type(),
                "enum_boxed_payload_type_desc_i8",
            )?;
            let rt_alloc = self.declare_runtime_alloc_typed();
            let call = self.builder.build_call(
                rt_alloc,
                &[desc_i8.into(), size_v.into()],
                "rt_alloc_enum_boxed_payload",
            )?;
            let raw =
                call.try_as_basic_value()
                    .basic()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "scoop_alloc_typed return value (enum boxed payload)",
                        at: span.into(),
                    })?;
            let BasicValueEnum::PointerValue(raw_ptr) = raw else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "scoop_alloc_typed return type (enum boxed payload)",
                    at: span.into(),
                });
            };

            let payload_obj_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
            let payload_obj_ptr = self.builder.build_pointer_cast(
                raw_ptr,
                payload_obj_ptr_ty,
                "enum_boxed_payload_obj_ptr",
            )?;
            let payload_gep = self.builder.build_struct_gep(
                payload_obj_ty,
                payload_obj_ptr,
                1,
                "enum_boxed_payload_gep",
            )?;
            let _ = self
                .builder
                .build_store(payload_gep, payload.as_basic_value_enum())?;

            let payload_ptr_ty = self.llvm_gc_i8_ptr_type();
            let payload_ptr_i8 = self.builder.build_pointer_cast(
                payload_obj_ptr,
                payload_ptr_ty,
                "enum_boxed_payload_as_i8",
            )?;

            let word_ty = self.int_type(self.enum_payload_ty());
            let payload_word = word_ty.const_int(0, false);
            return self.build_enum_value(
                span,
                enum_ty,
                variant.tag,
                CgEnumPayload {
                    word: Some(payload_word),
                    gc_ptr: Some(payload_ptr_i8),
                },
            );
        }

        // 2) inline（非 boxed）variant：当前阶段仍采用 "word payload" 承载的小 payload。
        if variant.fields.len() > 1 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "enum variant payload (multi-field, not boxed)",
                at: span.into(),
            });
        }

        let payload = if let Some((field_cg, field_v)) = field_values.first().copied() {
            self.coerce_enum_payload(span, field_v, field_cg)?
        } else {
            CgEnumPayload::default()
        };

        self.build_enum_value(span, enum_ty, variant.tag, payload)
    }

    fn coerce_enum_payload(
        &mut self,
        at: crate::span::Span,
        value: CgValue<'ctx>,
        value_ty: CgTy,
    ) -> Result<CgEnumPayload<'ctx>, LlvmEmitError> {
        let payload_ty = self.enum_payload_ty();
        let payload_int_ty = self.int_type(payload_ty);

        match value_ty {
            CgTy::Unit | CgTy::Never => Ok(CgEnumPayload {
                word: Some(payload_int_ty.const_int(0, false)),
                gc_ptr: None,
            }),
            CgTy::Bool => {
                let b = value.as_bool().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "enum payload bool",
                    at: at.into(),
                })?;
                let widened =
                    self.builder
                        .build_int_z_extend(b, payload_int_ty, "enum_payload_bool")?;
                Ok(CgEnumPayload {
                    word: Some(widened),
                    gc_ptr: None,
                })
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
                let casted = self.cast_int(v, from, payload_ty)?;
                Ok(CgEnumPayload {
                    word: Some(casted),
                    gc_ptr: None,
                })
            }
            CgTy::Float64 | CgTy::Float32 => {
                let word = self.coerce_u64_word(at, value)?;
                Ok(CgEnumPayload {
                    word: Some(word),
                    gc_ptr: None,
                })
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
                let casted = self.builder.build_pointer_cast(
                    ptr,
                    self.llvm_gc_i8_ptr_type(),
                    "enum_payload_str_as_ref",
                )?;
                Ok(CgEnumPayload {
                    word: None,
                    gc_ptr: Some(casted),
                })
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
                let casted = self.builder.build_pointer_cast(
                    ptr,
                    self.llvm_gc_i8_ptr_type(),
                    "enum_payload_ref_as_i8",
                )?;
                Ok(CgEnumPayload {
                    word: None,
                    gc_ptr: Some(casted),
                })
            }
            CgTy::Enum(nested_enum_ty) => {
                // 允许把 "niche enum（当前主要是 `Option<...>`）" 作为 payload 承载到外层 enum/Option 中。
                //
                // 关键点：
                // - niche enum 的运行期值本身就是一个"标量存储"（ptr 或 u8）；
                // - 因此可以映射到 tagged union 的 `{ payload_word, payload_ptr }` 载体上，
                //   且不引入 ptr<->int 编码（GC safety）。
                let Some(raw) = value.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "enum payload nested enum",
                        at: at.into(),
                    });
                };

                let repr = self.cg_enum_layout(at, nested_enum_ty)?.repr;
                match repr {
                    CgEnumRepr::Niche {
                        storage,
                        none_value,
                    } => match storage {
                        NicheStorage::Pointer => {
                            // GC safety（T1518）：pointer niche 只允许 `None = NULL`。
                            if none_value != 0 {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "nested niche enum pointer none_value (must be NULL)",
                                    at: at.into(),
                                });
                            }

                            let BasicValueEnum::PointerValue(ptr) = raw else {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "nested niche enum payload (ptr)",
                                    at: at.into(),
                                });
                            };

                            let casted = self.builder.build_pointer_cast(
                                ptr,
                                self.llvm_gc_i8_ptr_type(),
                                "enum_payload_nested_niche_ptr_as_i8",
                            )?;
                            Ok(CgEnumPayload {
                                word: None,
                                gc_ptr: Some(casted),
                            })
                        }
                        NicheStorage::U8 => {
                            let BasicValueEnum::IntValue(v) = raw else {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "nested niche enum payload (u8)",
                                    at: at.into(),
                                });
                            };
                            let widened = self.builder.build_int_z_extend(
                                v,
                                payload_int_ty,
                                "enum_payload_nested_niche_u8",
                            )?;
                            Ok(CgEnumPayload {
                                word: Some(widened),
                                gc_ptr: None,
                            })
                        }
                    },
                    _ => Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "enum payload (nested enum, unsupported repr)",
                        at: at.into(),
                    }),
                }
            }
            CgTy::Tuple(_) | CgTy::Struct(_) => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "enum payload (non-scalar)",
                at: at.into(),
            }),
        }
    }

    fn build_enum_value(
        &mut self,
        at: crate::span::Span,
        enum_ty: TypeId,
        tag: u64,
        payload: CgEnumPayload<'ctx>,
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
                let payload_word_ty = self.int_type(self.enum_payload_ty());
                let payload_ptr_ty = self.llvm_gc_i8_ptr_type();

                agg = self.builder.build_insert_value(
                    agg,
                    tag_ty.const_int(tag, false),
                    0,
                    "enum_tag",
                )?;

                let payload_word_v = payload
                    .word
                    .unwrap_or_else(|| payload_word_ty.const_int(0, false));
                agg =
                    self.builder
                        .build_insert_value(agg, payload_word_v, 1, "enum_payload_word")?;

                let payload_ptr_v = payload
                    .gc_ptr
                    .unwrap_or_else(|| payload_ptr_ty.const_null());
                agg = self
                    .builder
                    .build_insert_value(agg, payload_ptr_v, 2, "enum_payload_ptr")?;

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
                let raw: BasicValueEnum<'ctx> = match storage {
                    NicheStorage::Pointer => {
                        if none_value != 0 {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "Option niche pointer none_value (must be NULL)",
                                at: at.into(),
                            });
                        }

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

                        match tag {
                            0 => {
                                let Some(raw_ptr) = payload.gc_ptr else {
                                    return Err(LlvmEmitError::UnsupportedMainBody {
                                        kind: "Option niche Some payload missing",
                                        at: at.into(),
                                    });
                                };
                                self.builder
                                    .build_pointer_cast(raw_ptr, ptr_ty, "option_some_cast")?
                                    .into()
                            }
                            1 => ptr_ty.const_null().into(),
                            _ => {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "Option niche tag",
                                    at: at.into(),
                                });
                            }
                        }
                    }
                    NicheStorage::U8 => {
                        let encoded = payload
                            .word
                            .unwrap_or_else(|| word_ty.const_int(none_value, false));
                        self.builder
                            .build_int_truncate(encoded, self.context.i8_type(), "option_niche_u8")?
                            .into()
                    }
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

    // 控制流 codegen（if/when 等）已拆分到子模块（T0102d）。

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
            CgTy::Unit | CgTy::Never => self.context.i8_type().const_int(0, false).into(),
            CgTy::Bool
            | CgTy::Float64
            | CgTy::Float32
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
                CgTy::Never => CgValue::never(),
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
                CgTy::Float64 | CgTy::Float32 => {
                    let raw = llvm_fun
                        .get_nth_param(idx as u32)
                        .ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "missing llvm param",
                            at: param.span.into(),
                        })?
                        .into_float_value();
                    CgValue::float(raw, target_ty)
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
                    let raw = llvm_fun.get_nth_param(idx as u32).ok_or(
                        LlvmEmitError::UnsupportedMainBody {
                            kind: "missing llvm param",
                            at: param.span.into(),
                        },
                    )?;
                    CgValue {
                        ty: target_ty,
                        value: Some(raw),
                    }
                }
            };

            let _stored = self.store_local_value(param.span, ptr, target_ty, init)?;
            self.env.insert(
                param.id,
                CgLocal {
                    hir_ty: Some(param.ty),
                    ty: target_ty,
                    ptr,
                    mutable: false,
                },
            );
        }
        Ok(())
    }

    fn default_value(
        &mut self,
        at: crate::span::Span,
        ty: CgTy,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match ty {
            CgTy::Unit => Ok(CgValue::unit()),
            // T1612: Nothing/Never has no runtime value.
            CgTy::Never => Ok(CgValue::never()),
            _ => {
                let llvm_ty = self.llvm_basic_type_of(at, ty)?;
                let raw = self.zero_initializer_for_basic_type(llvm_ty);
                self.cg_value_from_loaded(at, ty, raw)
            }
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
        let i8_ptr_ty = self.llvm_i8_ptr_type();
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

    /// T0141: Codegen an early `return` from inside a nested block or loop.
    /// Stores the return value into the function-level return alloca and branches to the return BB.
    pub(super) fn codegen_early_return(
        &mut self,
        span: crate::span::Span,
        value: Option<&hir::Expr>,
    ) -> Result<(), LlvmEmitError> {
        let return_ctx = self
            .return_context
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "return outside function with return context",
                at: span.into(),
            })?;
        let declared_return_cg = self.current_fun_return_ty.unwrap_or(CgTy::Unit);

        match value {
            Some(expr) => {
                let v = self.codegen_expr_in_expected_context(expr, Some(declared_return_cg))?;
                if let Some(alloca) = return_ctx.return_alloca {
                    let coerced = self.coerce_value(expr.span, v, declared_return_cg)?;
                    if let Some(raw) = coerced.value {
                        self.builder.build_store(alloca, raw)?;
                    }
                }
            }
            None => {
                // `return` without value — for Unit functions, no store needed.
            }
        }

        self.builder
            .build_unconditional_branch(return_ctx.return_bb)?;
        Ok(())
    }

    fn emit_return(
        &mut self,
        span: crate::span::Span,
        declared_return_ty: CgTy,
        value: CgValue<'ctx>,
    ) -> Result<(), LlvmEmitError> {
        match declared_return_ty {
            CgTy::Unit => {
                self.builder.build_return(None)?;
                Ok(())
            }
            // T1612: A function declared as returning Nothing never returns normally.
            // Emit `unreachable` instead of a return instruction.
            CgTy::Never => {
                self.builder.build_unreachable()?;
                Ok(())
            }
            CgTy::Bool
            | CgTy::Float64
            | CgTy::Float32
            | CgTy::Int(_)
            | CgTy::String
            | CgTy::Ref
            | CgTy::Tuple(_)
            | CgTy::Struct(_)
            | CgTy::Enum(_) => {
                let Some(raw) = value.value else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: if self.current_sret_return_ptr.is_some() {
                            "sret return value"
                        } else {
                            "return value"
                        },
                        at: span.into(),
                    });
                };
                if let Some(sret_ptr) = self.current_sret_return_ptr
                    && matches!(
                        declared_return_ty,
                        CgTy::Tuple(_) | CgTy::Struct(_) | CgTy::Enum(_)
                    )
                {
                    let _ = self.builder.build_store(sret_ptr, raw)?;
                    self.builder.build_return(None)?;
                } else {
                    self.builder.build_return(Some(&raw))?;
                }
                Ok(())
            }
        }
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
            hir::LiteralKind::Char(value) => Ok(CgValue::int(
                self.context.i32_type().const_int(*value as u64, false),
                IntTy {
                    bits: 32,
                    signed: false,
                },
            )),
            hir::LiteralKind::Int => {
                let Some(CgTy::Int(int_ty)) = self.cg_ty_of(ty) else {
                    return Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "int literal type",
                        at: span.into(),
                    });
                };
                let value = self.int_literal_bits_for_ty(span, int_ty)?;
                Ok(CgValue::int(
                    self.int_type(int_ty).const_int(value, false),
                    int_ty,
                ))
            }
            hir::LiteralKind::Float64(value) => Ok(CgValue::float(
                self.context.f64_type().const_float(*value),
                CgTy::Float64,
            )),
            hir::LiteralKind::Float32(value) => Ok(CgValue::float(
                self.context.f32_type().const_float(f64::from(*value)),
                CgTy::Float32,
            )),
            hir::LiteralKind::String => self.codegen_string_literal(span),
            hir::LiteralKind::SynthInt(value) => {
                // Synthesized integer literal from compiler desugaring (T0110).
                let int_ty = IntTy {
                    bits: 64,
                    signed: true,
                };
                Ok(CgValue::int(
                    self.int_type(int_ty).const_int(*value as u64, false),
                    int_ty,
                ))
            }
        }
    }

    /// Emit LLVM IR for a string literal by parsing the current source text on demand.
    fn codegen_string_literal(
        &mut self,
        span: crate::span::Span,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let bytes = self.parse_current_string_literal_bytes(span)?;
        self.codegen_string_literal_from_bytes(span, &bytes)
    }

    /// Emit LLVM IR for a string literal from already parsed bytes.
    fn codegen_string_literal_from_bytes(
        &mut self,
        span: crate::span::Span,
        bytes: &[u8],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        // 1) 分配一个 GC-managed `ScoopString` 对象：
        //    - LLVM 侧类型为 `ScoopString addrspace(1)*`
        //    - 分配通过 `scoop_alloc_typed(desc, sizeof(ScoopString))`（runtime 写入对象头 type_desc）
        let scoop_str_ty = self.llvm_scoop_string_type();
        let obj_size = self.target_data.get_store_size(&scoop_str_ty);
        let size_v = self.context.i64_type().const_int(obj_size, false);

        let str_desc = self.get_or_create_string_type_desc_global(span)?;
        let str_desc_i8 = self.builder.build_pointer_cast(
            str_desc.as_pointer_value(),
            self.llvm_i8_ptr_type(),
            "str_type_desc_i8",
        )?;
        let rt_alloc = self.declare_runtime_alloc_typed();
        let call = self.builder.build_call(
            rt_alloc,
            &[str_desc_i8.into(), size_v.into()],
            "rt_alloc_string_lit",
        )?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc_typed return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(raw_ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc_typed return type",
                at: span.into(),
            });
        };

        let str_ptr_ty = self.llvm_scoop_string_ptr_type();
        let str_ptr = self
            .builder
            .build_pointer_cast(raw_ptr, str_ptr_ty, "str_obj_ptr")?;

        // 2) 写入 `{ len, data }`。
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
            let i8_ptr_ty = self.llvm_i8_ptr_type();
            let _ = self.builder.build_store(data_ptr, i8_ptr_ty.const_null())?;
        } else {
            // 把字节序列落到一个只读全局常量：`[N x i8] @__scoop_str_data_*`
            let data_gv = self.get_or_create_global_bytes(span, bytes);
            let i8_ptr_ty = self.llvm_i8_ptr_type();
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

    fn load_scoop_string_len_and_data(
        &mut self,
        str_obj_ptr: PointerValue<'ctx>,
    ) -> Result<(IntValue<'ctx>, PointerValue<'ctx>), LlvmEmitError> {
        let scoop_str_ty = self.llvm_scoop_string_type();
        let i64_ty = self.context.i64_type();
        let i8_ptr_ty = self.llvm_i8_ptr_type();

        let len_ptr =
            self.builder
                .build_struct_gep(scoop_str_ty, str_obj_ptr, 1, "str_len_gep_interp")?;
        let data_ptr =
            self.builder
                .build_struct_gep(scoop_str_ty, str_obj_ptr, 2, "str_data_gep_interp")?;

        let len = self
            .builder
            .build_load(i64_ty, len_ptr, "str_len_interp")?
            .into_int_value();
        let data = self
            .builder
            .build_load(i8_ptr_ty, data_ptr, "str_data_interp")?
            .into_pointer_value();

        Ok((len, data))
    }

    fn codegen_interpolated_string(
        &mut self,
        span: crate::span::Span,
        raw: bool,
        parts: &[hir::InterpolatedStringPart],
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        // 当前阶段的落点：把 f-string 分片后"拼接"为一段连续 UTF-8 字节序列，
        // 返回一个 GC-managed `ScoopString` 对象（addrspace(1)），其 `data` 指向 `malloc` 的 bytes buffer。
        //
        // 约束（与 TODO T0823 对齐）：
        // - 目前支持 `{Bool}` / `{Char}` / `{Int}` / `{String}` / `{Float}`；
        // - 先不支持 format spec / locale；
        // - 当前阶段不接入 type descriptor/release：`data` 的释放留给后续任务补齐（T1507/T1514）。

        #[derive(Clone, Copy)]
        struct Segment<'ctx> {
            ptr: PointerValue<'ctx>,
            len: IntValue<'ctx>,
        }

        let i64_ty = self.context.i64_type();
        let i8_ty = self.context.i8_type();
        let i8_ptr_ty = self.llvm_i8_ptr_type();
        let scoop_str_ty = self.llvm_scoop_string_type();

        // 1) 先做一遍：收集所有片段的 (ptr, len)，并计算总长度（运行期）。
        let mut segments: Vec<Segment<'ctx>> = Vec::new();
        let mut total_len = i64_ty.const_zero();

        for part in parts {
            match part {
                hir::InterpolatedStringPart::Text { span: text_span } => {
                    let text = self.current_source_slice(*text_span)?;
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
                    if self.expr_is_builtin_char(expr) {
                        let str_v = self.codegen_char_method_to_string(expr.span, expr)?;
                        let Some(raw) = str_v.value else {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "string interpolation char expr value",
                                at: expr.span.into(),
                            });
                        };
                        let BasicValueEnum::PointerValue(str_obj_ptr) = raw else {
                            return Err(LlvmEmitError::UnsupportedMainBody {
                                kind: "string interpolation char expr type",
                                at: expr.span.into(),
                            });
                        };

                        let (len, data) = self.load_scoop_string_len_and_data(str_obj_ptr)?;

                        segments.push(Segment { ptr: data, len });
                        total_len = self
                            .builder
                            .build_int_add(total_len, len, "fstr_total_len")?;
                        continue;
                    }

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

                            let (len, data) = self.load_scoop_string_len_and_data(str_obj_ptr)?;

                            segments.push(Segment { ptr: data, len });
                            total_len =
                                self.builder
                                    .build_int_add(total_len, len, "fstr_total_len")?;
                        }
                        CgTy::Bool => {
                            let Some(raw) = v.value else {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "string interpolation bool expr value",
                                    at: expr.span.into(),
                                });
                            };
                            let BasicValueEnum::IntValue(bool_val) = raw else {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "string interpolation bool expr type",
                                    at: expr.span.into(),
                                });
                            };

                            let bool_as_i64 = self.builder.build_int_z_extend(
                                bool_val,
                                i64_ty,
                                "fstr_bool_zext",
                            )?;
                            let rt_bool = self.declare_runtime_bool_to_string();
                            let call = self.builder.build_call(
                                rt_bool,
                                &[bool_as_i64.into()],
                                "rt_bool_to_string_for_fstr",
                            )?;
                            let ret = call.try_as_basic_value().basic().ok_or(
                                LlvmEmitError::UnsupportedMainBody {
                                    kind: "string interpolation bool return value",
                                    at: expr.span.into(),
                                },
                            )?;
                            let BasicValueEnum::PointerValue(str_obj_ptr) = ret else {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "string interpolation bool return type",
                                    at: expr.span.into(),
                                });
                            };

                            let (len, data) = self.load_scoop_string_len_and_data(str_obj_ptr)?;

                            segments.push(Segment { ptr: data, len });
                            total_len =
                                self.builder
                                    .build_int_add(total_len, len, "fstr_total_len")?;
                        }
                        CgTy::Float64 | CgTy::Float32 => {
                            let str_v =
                                self.codegen_float_to_string_value(expr.span, expr.span, v)?;
                            let Some(raw) = str_v.value else {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "string interpolation float expr value",
                                    at: expr.span.into(),
                                });
                            };
                            let BasicValueEnum::PointerValue(str_obj_ptr) = raw else {
                                return Err(LlvmEmitError::UnsupportedMainBody {
                                    kind: "string interpolation float expr type",
                                    at: expr.span.into(),
                                });
                            };

                            let (len, data) = self.load_scoop_string_len_and_data(str_obj_ptr)?;

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

        let str_desc = self.get_or_create_string_type_desc_global(span)?;
        let str_desc_i8 = self.builder.build_pointer_cast(
            str_desc.as_pointer_value(),
            self.llvm_i8_ptr_type(),
            "fstr_type_desc_i8",
        )?;
        let rt_alloc = self.declare_runtime_alloc_typed();
        let call = self.builder.build_call(
            rt_alloc,
            &[str_desc_i8.into(), size_v.into()],
            "rt_alloc_fstr",
        )?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc_typed return value",
                at: span.into(),
            })?;
        let BasicValueEnum::PointerValue(raw_ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc_typed return type",
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
                // - 运行期用一个 module-local 的唯一地址作为"单例实例指针"（ref type）。
                if self.object_inits.contains_key(fqn) {
                    return self.codegen_object_value_access(span, fqn);
                }

                if let Some(top_level_const) = self.top_level_consts.get(fqn).cloned() {
                    return self.codegen_top_level_const_ref(span, &top_level_const);
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
                    CgTy::Float64 | CgTy::Float32 => {
                        CgValue::float(loaded.into_float_value(), cg_ty)
                    }
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
                    CgTy::Never => CgValue::never(),
                })
            }
            hir::ValueRef::Local { id, .. } => {
                let local =
                    self.env
                        .get(*id)
                        .ok_or_else(|| LlvmEmitError::UnsupportedMainBody {
                            kind: "unknown local value",
                            at: span.into(),
                        })?;

                match local.ty {
                    CgTy::Unit => Ok(CgValue::unit()),
                    CgTy::Never => Ok(CgValue::never()),
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
                    CgTy::Float64 | CgTy::Float32 => {
                        let raw = self
                            .builder
                            .build_load(
                                self.llvm_basic_type_of(span, local.ty)?,
                                local.ptr,
                                "load_float",
                            )?
                            .into_float_value();
                        Ok(CgValue::float(raw, local.ty))
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

        let layout_key = self.nominal_layout_key(nominal);
        let layout =
            self.struct_layouts
                .get(&layout_key)
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

            // 重要：struct 字段 initializer 需要以字段类型作为 expected context。
            //
            // 例如：`Wrap { e: B(7) }` 中的 `B(7)` 是 enum variant ctor call：
            // - 若缺少 expected enum type，后端无法决定该 ctor 对应哪个 enum 的表示；
            // - 这里把 `field_cg` 作为 expected 传入，可与 `val x: E = B(7)` 的路径保持一致。
            let init_v = self.codegen_expr_in_expected_context(&init.value, Some(field_cg))?;
            let coerced = if field_cg == CgTy::Unit {
                CgValue::unit()
            } else if init_v.ty != field_cg {
                self.coerce_value(init.value.span, init_v, field_cg)?
            } else {
                init_v
            };

            let raw = match field_cg {
                CgTy::Unit => self.context.i8_type().const_int(0, false).into(),
                _ => coerced.value.ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "struct field value",
                    at: init.value.span.into(),
                })?,
            };

            // T0119: For `@CLayout(packed = N)` with N > 1, use the remapped LLVM element index.
            let llvm_idx = self
                .pack_field_indices
                .get(&layout_key)
                .map_or(idx as u32, |indices| indices[idx]);

            let name = format!("insert_{}", field.name);
            agg = self.builder.build_insert_value(agg, raw, llvm_idx, &name)?;
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
                // T1311：`TypeName.NestedObject` / `Obj.NestedObject` 的"object 值"访问。
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

                // T0125：从 receiver 局部变量获取精确的 hir_ty（receiver.ty 在 HIR 中总是 Any，
                // 但 CgLocal.hir_ty 保留了声明时的参数化类型，例如 Box<Int>）。
                let receiver_hir_ty = if let hir::ExprKind::VarRef(hir::ValueRef::Local {
                    id,
                    ..
                }) = &receiver.kind
                {
                    self.env
                        .get(*id)
                        .and_then(|local| local.hir_ty)
                        .unwrap_or(receiver.ty)
                } else {
                    receiver.ty
                };

                // T1312：class 实例字段访问（`this.x` / `obj.x`）。
                if let Some((class, field_idx, field_cg)) =
                    self.lookup_class_field_by_fqn(fqn, member.span, Some(receiver_hir_ty))?
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
                if let hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) = &receiver.kind
                    && let Some(local) = self.env.get(*id)
                    && let CgTy::Struct(struct_ty) = local.ty
                {
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
                    let loaded = self
                        .builder
                        .build_load(llvm_field_ty, field_ptr, "load_field")?;
                    // `@CLayout(packed = N)`：字段地址可能是非自然对齐的，需要把 load
                    // alignment 降到 `min(field_natural_align, N)` 以避免 UB。
                    if let Some(pack_n) = self.struct_clayout(struct_ty).and_then(|c| c.packed)
                        && let Some(inst) = loaded.as_instruction_value()
                    {
                        let natural = self.target_data.get_abi_alignment(&llvm_field_ty);
                        let effective = std::cmp::min(natural, pack_n);
                        inst.set_alignment(effective)?;
                    }
                    return self.cg_value_from_loaded(member.span, field_ty, loaded);
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
        if let hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) = &receiver.kind
            && let Some(local) = self.env.get(*id)
            && let CgTy::Tuple(tuple_ty) = local.ty
        {
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
            let loaded = self
                .builder
                .build_load(llvm_elem_ty, elem_ptr, "load_tuple_elem")?;
            return self.cg_value_from_loaded(member.span, elem_ty, loaded);
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

    /// 将一个"限定名 enum unit variant 值"（例如 `RuntimeError.NullAssertionFailed`）降低为 enum 常量。
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

        let v = self.build_enum_value(at, enum_ty, tag, CgEnumPayload::default())?;
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

        let mut init_codegen = MainCodegen::new(MainCodegenInputs {
            context: self.context,
            module: self.module,
            builder: self.builder,
            target_data: self.target_data,
            host: self.host,
            source_map: self.source_map,
            entry_source_id: self.entry_source_id,
            types: self.types,
            struct_layouts: self.struct_layouts,
            enum_layouts: self.enum_layouts,
            top_level_vars: self.top_level_vars,
            top_level_consts: self.top_level_consts,
            object_inits: self.object_inits,
            class_inits: self.class_inits,
            class_vtables: self.class_vtables,
            interfaces: self.interfaces,
            class_itables: self.class_itables,
            ctor_call_sites: self.ctor_call_sites,
            continuation_resume_call_sites: self.continuation_resume_call_sites,
            extern_funs: self.extern_funs,
            fun_index: self.fun_index,
            effect_op_tags: Rc::clone(&self.effect_op_tags),
        });
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

        self.current_source_id = self.source_id_for_path(obj.source_path.as_path(), err_span)?;

        let entry = self.context.append_basic_block(llvm_fun, "entry");
        let init_bb = self.context.append_basic_block(llvm_fun, "init");
        let done_bb = self.context.append_basic_block(llvm_fun, "done");

        self.builder.position_at_end(entry);
        // object init 是一个内部 `void` 函数：设置 current_fun_return_ty 以便 codegen_return_stmt 使用正确的返回类型。
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
        // - 早期阶段我们用一个 module-local 的唯一地址充当 object 单例实例的"身份"（指针值）；
        // - 该地址不参与 GC，也不承载字段布局；静态属性仍单独走 `__scoop_object_prop__*` 全局存储。
        let gv = self.module.add_global(self.context.i8_type(), None, &name);
        gv.set_linkage(Linkage::Internal);
        gv.set_initializer(&self.context.i8_type().const_int(0, false));
        gv
    }

    fn codegen_object_value_access(
        &mut self,
        at: crate::span::Span,
        object_fqn: &str,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let init_fn = self.ensure_object_init_function_defined(object_fqn)?;
        let _ = self.builder.build_call(init_fn, &[], "obj_init")?;
        self.emit_ordinary_call_effect_propagation_check(at, "object_init_effect")?;

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

    fn cg_value_from_loaded(
        &self,
        _at: crate::span::Span,
        ty: CgTy,
        raw: BasicValueEnum<'ctx>,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        Ok(match ty {
            CgTy::Unit => CgValue::unit(),
            CgTy::Bool => CgValue::bool(raw.into_int_value()),
            CgTy::Float64 | CgTy::Float32 => CgValue::float(raw.into_float_value(), ty),
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
            CgTy::Never => CgValue::never(),
        })
    }

    fn codegen_unary(
        &mut self,
        span: crate::span::Span,
        result_ty: TypeId,
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
                if matches!(expr.kind, hir::ExprKind::Literal(hir::LiteralKind::Int))
                    && let Some(CgTy::Int(int_ty)) = self.cg_ty_of(result_ty)
                {
                    let bits = self.negated_int_literal_bits_for_ty(span, expr.span, int_ty)?;
                    return Ok(CgValue::int(
                        self.int_type(int_ty).const_int(bits, false),
                        int_ty,
                    ));
                }

                let value = self.codegen_expr(expr)?;
                match value.ty {
                    CgTy::Int(ty) => {
                        let (v, _) = value.as_int().ok_or(LlvmEmitError::UnsupportedMainBody {
                            kind: "unary - operand",
                            at: span.into(),
                        })?;
                        let out = self.builder.build_int_neg(v, "neg")?;
                        Ok(CgValue::int(out, ty))
                    }
                    CgTy::Float64 | CgTy::Float32 => {
                        let (v, ty) =
                            value.as_float().ok_or(LlvmEmitError::UnsupportedMainBody {
                                kind: "unary - operand",
                                at: span.into(),
                            })?;
                        let out = self.builder.build_float_neg(v, "fneg")?;
                        Ok(CgValue::float(out, ty))
                    }
                    _ => Err(LlvmEmitError::UnsupportedMainBody {
                        kind: "unary - operand",
                        at: span.into(),
                    }),
                }
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

    /// Map a binary operator to its operator overload method name (Spec B.8).
    fn operator_overload_method_name(op: ast::BinaryOp) -> Option<&'static str> {
        match op {
            ast::BinaryOp::Add => Some("plus"),
            ast::BinaryOp::Sub => Some("minus"),
            ast::BinaryOp::Mul => Some("times"),
            ast::BinaryOp::Div => Some("div"),
            ast::BinaryOp::Rem => Some("rem"),
            ast::BinaryOp::BitAnd => Some("and"),
            ast::BinaryOp::BitOr => Some("or"),
            ast::BinaryOp::BitXor => Some("xor"),
            ast::BinaryOp::Shl => Some("shl"),
            ast::BinaryOp::Shr => Some("shr"),
            _ => None,
        }
    }

    /// Try to dispatch a binary operator to a user-defined method on a struct type.
    /// Returns `Some(result)` if the LHS is a struct with the corresponding operator method,
    /// `None` if the LHS is not a struct type (caller should use builtin integer path).
    /// Resolve the effective CgTy for an expression, checking the local env first
    /// (since HIR lowering often assigns `Any` to VarRef expressions).
    fn resolve_expr_cg_ty(&self, expr: &hir::Expr) -> Option<CgTy> {
        // Try the local environment first (has accurate types for locals).
        if let hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) = &expr.kind
            && let Some(local) = self.env.get(*id)
        {
            return Some(local.ty);
        }
        // Fall back to HIR expression type.
        self.cg_ty_of(expr.ty)
    }

    fn expr_uses_float_codegen(&self, expr: &hir::Expr) -> bool {
        matches!(
            self.resolve_expr_cg_ty(expr),
            Some(CgTy::Float64 | CgTy::Float32)
        ) || matches!(
            expr.kind,
            hir::ExprKind::Literal(hir::LiteralKind::Float64(_))
                | hir::ExprKind::Literal(hir::LiteralKind::Float32(_))
        )
    }

    fn is_unsuffixed_float64_literal(expr: &hir::Expr) -> bool {
        matches!(
            expr.kind,
            hir::ExprKind::Literal(hir::LiteralKind::Float64(_))
        )
    }

    fn unify_float_cg_types(
        &self,
        lhs: &hir::Expr,
        lhs_ty: CgTy,
        rhs: &hir::Expr,
        rhs_ty: CgTy,
    ) -> Option<CgTy> {
        match (lhs_ty, rhs_ty) {
            (CgTy::Float64, CgTy::Float64) => Some(CgTy::Float64),
            (CgTy::Float32, CgTy::Float32) => Some(CgTy::Float32),
            (CgTy::Float64, CgTy::Float32) if Self::is_unsuffixed_float64_literal(lhs) => {
                Some(CgTy::Float32)
            }
            (CgTy::Float32, CgTy::Float64) if Self::is_unsuffixed_float64_literal(rhs) => {
                Some(CgTy::Float32)
            }
            _ => None,
        }
    }

    fn try_codegen_operator_overload(
        &mut self,
        span: crate::span::Span,
        op: ast::BinaryOp,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
    ) -> Result<Option<CgValue<'ctx>>, LlvmEmitError> {
        // Check if LHS has a struct type, resolving through local env if needed.
        let Some(CgTy::Struct(lhs_type_id)) = self.resolve_expr_cg_ty(lhs) else {
            return Ok(None);
        };

        let method = match Self::operator_overload_method_name(op) {
            Some(m) => m,
            None => return Ok(None),
        };

        // Get the struct FQN from TypeId.
        let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = self.types.kind(lhs_type_id) else {
            return Ok(None);
        };
        let struct_fqn = nominal.fqn.clone();
        let method_fqn = format!("{struct_fqn}.{method}");

        // Look up the method in fun_index.
        let sig_fun = match self.fun_index.get(method_fqn.as_str()) {
            Some(f) => *f,
            None => return Ok(None),
        };

        // Generate the call: StructType.method(lhs, rhs)
        let result = self.codegen_operator_overload_call(span, &method_fqn, sig_fun, lhs, rhs)?;
        Ok(Some(result))
    }

    /// Try to dispatch a comparison operator to a `compareTo` method on a struct type.
    /// `compareTo(other) -> Int`, then compare the result with 0.
    fn try_codegen_compare_to_overload(
        &mut self,
        span: crate::span::Span,
        op: ast::BinaryOp,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
    ) -> Result<Option<CgValue<'ctx>>, LlvmEmitError> {
        let Some(CgTy::Struct(lhs_type_id)) = self.resolve_expr_cg_ty(lhs) else {
            return Ok(None);
        };

        let TypeKind::Value(ValueTypeKind::Nominal(nominal)) = self.types.kind(lhs_type_id) else {
            return Ok(None);
        };
        let struct_fqn = nominal.fqn.clone();
        let method_fqn = format!("{struct_fqn}.compareTo");

        let sig_fun = match self.fun_index.get(method_fqn.as_str()) {
            Some(f) => *f,
            None => return Ok(None),
        };

        // Call compareTo: returns Int
        let cmp_result =
            self.codegen_operator_overload_call(span, &method_fqn, sig_fun, lhs, rhs)?;
        let (cmp_int, _) = cmp_result
            .as_int()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "compareTo return type (expected Int)",
                at: span.into(),
            })?;

        // Compare result with 0: result < 0 for Lt, result <= 0 for Le, etc.
        let zero = self.context.i64_type().const_zero();
        let pred = match op {
            ast::BinaryOp::Lt => IntPredicate::SLT,
            ast::BinaryOp::Le => IntPredicate::SLE,
            ast::BinaryOp::Gt => IntPredicate::SGT,
            ast::BinaryOp::Ge => IntPredicate::SGE,
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "compareTo comparison op",
                    at: span.into(),
                });
            }
        };
        let result = self
            .builder
            .build_int_compare(pred, cmp_int, zero, "cmp_to")?;
        Ok(Some(CgValue::bool(result)))
    }

    /// Generate a call to a struct's operator overload method.
    /// The method has signature: `fun StructType.method(this: StructType, rhs: RhsType): RetType`
    fn codegen_operator_overload_call(
        &mut self,
        span: crate::span::Span,
        method_fqn: &str,
        sig_fun: &hir::FunDecl,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        if sig_fun.params.len() != 2 {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "operator overload arity (expected 2)",
                at: span.into(),
            });
        }

        // Evaluate LHS (this/receiver parameter)
        let this_target_cg =
            self.cg_ty_of(sig_fun.params[0].ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "operator overload receiver type",
                    at: lhs.span.into(),
                })?;
        let lhs_v = self.codegen_expr(lhs)?;
        let lhs_coerced = self.coerce_value(lhs.span, lhs_v, this_target_cg)?;
        let lhs_arg = self.as_llvm_arg_value(lhs.span, this_target_cg, lhs_coerced)?;

        // Evaluate RHS (method parameter)
        let rhs_target_cg =
            self.cg_ty_of(sig_fun.params[1].ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "operator overload rhs type",
                    at: rhs.span.into(),
                })?;
        let rhs_v = self.codegen_expr(rhs)?;
        let rhs_coerced = self.coerce_value(rhs.span, rhs_v, rhs_target_cg)?;
        let rhs_arg = self.as_llvm_arg_value(rhs.span, rhs_target_cg, rhs_coerced)?;

        // Get or declare the LLVM function
        let llvm_fun = match self.module.get_function(method_fqn) {
            Some(f) => f,
            None => self.declare_top_level_fun(sig_fun)?,
        };

        let call_site = self
            .builder
            .build_call(llvm_fun, &[lhs_arg, rhs_arg], "op_overload")?;
        call_site.set_call_convention(self.llvm_call_convention_for_fqn(method_fqn));
        self.emit_ordinary_call_effect_propagation_check(span, "operator_call_effect")?;

        // Convert return value to CgValue (same pattern as codegen_top_level_fun_call).
        let ret_cg =
            self.cg_ty_of(sig_fun.return_ty)
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "operator overload return type",
                    at: span.into(),
                })?;
        match ret_cg {
            CgTy::Unit => Ok(CgValue::unit()),
            CgTy::Never => Ok(CgValue::never()),
            _ => {
                let raw = call_site.try_as_basic_value().basic().ok_or(
                    LlvmEmitError::UnsupportedMainBody {
                        kind: "operator overload return value",
                        at: span.into(),
                    },
                )?;
                self.cg_value_from_loaded(span, ret_cg, raw)
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
            | ast::BinaryOp::Rem => {
                // T0111: try operator overload dispatch for user-defined types first.
                if let Some(result) = self.try_codegen_operator_overload(span, op, lhs, rhs)? {
                    return Ok(result);
                }
                if self.expr_uses_float_codegen(lhs) || self.expr_uses_float_codegen(rhs) {
                    return self.codegen_float_binary_same_type(span, op, lhs, rhs);
                }
                self.codegen_int_binary_same_type(span, op, lhs, rhs)
            }
            ast::BinaryOp::BitAnd | ast::BinaryOp::BitXor | ast::BinaryOp::BitOr => {
                // T0111: try operator overload dispatch for user-defined types first.
                if let Some(result) = self.try_codegen_operator_overload(span, op, lhs, rhs)? {
                    return Ok(result);
                }
                self.codegen_int_binary_same_type(span, op, lhs, rhs)
            }

            ast::BinaryOp::Shl | ast::BinaryOp::Shr => {
                if let Some(result) = self.try_codegen_operator_overload(span, op, lhs, rhs)? {
                    return Ok(result);
                }
                self.codegen_shift(span, op, lhs, rhs)
            }

            ast::BinaryOp::Lt | ast::BinaryOp::Le | ast::BinaryOp::Gt | ast::BinaryOp::Ge => {
                // T0111: try compareTo overload for user-defined types first.
                if let Some(result) = self.try_codegen_compare_to_overload(span, op, lhs, rhs)? {
                    return Ok(result);
                }
                if self.expr_uses_float_codegen(lhs) || self.expr_uses_float_codegen(rhs) {
                    return self.codegen_float_compare(span, op, lhs, rhs);
                }
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

    fn codegen_type_check_expr(
        &mut self,
        span: crate::span::Span,
        op: ast::TypeCheckOp,
        expr: &hir::Expr,
        target_ty: TypeId,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        // 当前阶段只实现 ref→ref 的运行期检查（T1509b）：
        // - class：沿 type descriptor parent 链查找；
        // - interface：扫描 itable 是否包含 interface_id。
        //
        // 说明：typecheck 阶段对 `is/!is` 的静态约束仍偏弱（只保证 type lowering），
        // 因此 codegen 侧需要做"不可支持场景"的防御式报错，避免 silent miscompile。
        let v = self.codegen_expr(expr)?;
        let v = match v.ty {
            CgTy::Ref => v,
            CgTy::String => self.coerce_value(expr.span, v, CgTy::Ref)?,
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "type check operand (ref)",
                    at: span.into(),
                });
            }
        };
        let Some(raw) = v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "type check operand value",
                at: span.into(),
            });
        };
        let BasicValueEnum::PointerValue(obj_ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "type check operand type",
                at: span.into(),
            });
        };

        let is_ok = self.codegen_ref_is_instance_of(span, obj_ptr, target_ty)?;
        let out = match op {
            ast::TypeCheckOp::Is => is_ok,
            ast::TypeCheckOp::NotIs => self.builder.build_not(is_ok, "typecheck_not")?,
        };
        Ok(CgValue::bool(out))
    }

    fn codegen_cast_expr(
        &mut self,
        span: crate::span::Span,
        op: ast::CastOp,
        expr: &hir::Expr,
        target_ty: TypeId,
        out_ty: TypeId,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        match op {
            ast::CastOp::As => self.codegen_cast_as_expr(span, expr, target_ty),
            ast::CastOp::AsQ => self.codegen_cast_asq_expr(span, expr, target_ty, out_ty),
        }
    }

    fn codegen_cast_as_expr(
        &mut self,
        span: crate::span::Span,
        expr: &hir::Expr,
        target_ty: TypeId,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let target_cg = self
            .cg_ty_of(target_ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "cast target type",
                at: span.into(),
            })?;
        let target_ptr_ty = match target_cg {
            CgTy::Ref => self.llvm_gc_i8_ptr_type(),
            CgTy::String => self.llvm_scoop_string_ptr_type(),
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "cast target (ref)",
                    at: span.into(),
                });
            }
        };

        let v = self.codegen_expr(expr)?;
        let v = match v.ty {
            CgTy::Ref => v,
            CgTy::String => self.coerce_value(expr.span, v, CgTy::Ref)?,
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "cast operand (ref)",
                    at: span.into(),
                });
            }
        };
        let Some(raw) = v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "cast operand value",
                at: span.into(),
            });
        };
        let BasicValueEnum::PointerValue(obj_ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "cast operand type",
                at: span.into(),
            });
        };

        // 运行期检查：为避免在 obj=NULL 时解引用对象头，先对 NULL 做 fail 处理。
        let is_ok = self.codegen_ref_is_instance_of(span, obj_ptr, target_ty)?;

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

        let ok_bb = self.context.append_basic_block(func, "cast_ok");
        let fail_bb = self.context.append_basic_block(func, "cast_fail");
        let merge_bb = self.context.append_basic_block(func, "cast_merge");
        self.builder
            .build_conditional_branch(is_ok, ok_bb, fail_bb)?;

        // --- ok ---
        self.builder.position_at_end(ok_bb);
        let casted_ptr = self
            .builder
            .build_pointer_cast(obj_ptr, target_ptr_ty, "cast_ptr")?;
        self.builder.build_unconditional_branch(merge_bb)?;

        // --- fail ---
        self.builder.position_at_end(fail_bb);
        self.emit_raise_runtime_error_variant(span, "ClassCastFailed")?;
        let fail_incoming = if self.ordinary_effect_propagation_enabled() {
            self.emit_ordinary_non_resuming_effect_exit(span, "cast_raise_effect")?;
            None
        } else {
            let dead_bb =
                self.builder
                    .get_insert_block()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "builder has no insert block",
                        at: span.into(),
                    })?;
            let default_ptr = target_ptr_ty.const_null();
            self.builder.build_unconditional_branch(merge_bb)?;
            Some((default_ptr, dead_bb))
        };

        // --- merge ---
        self.builder.position_at_end(merge_bb);
        let phi = self.builder.build_phi(target_ptr_ty, "cast_value")?;
        if let Some((default_ptr, dead_bb)) = fail_incoming {
            phi.add_incoming(&[(&casted_ptr, ok_bb), (&default_ptr, dead_bb)]);
        } else {
            phi.add_incoming(&[(&casted_ptr, ok_bb)]);
        }
        let out_ptr = phi.as_basic_value().into_pointer_value();

        Ok(CgValue {
            ty: target_cg,
            value: Some(out_ptr.into()),
        })
    }

    fn codegen_cast_asq_expr(
        &mut self,
        span: crate::span::Span,
        expr: &hir::Expr,
        target_ty: TypeId,
        out_ty: TypeId,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        // `as?` 的结果类型应为 `Option<target_ty>`（或等价 nullable sugar）。
        let out_cg = self
            .cg_ty_of(out_ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "cast result type",
                at: span.into(),
            })?;
        let CgTy::Enum(option_ty) = out_cg else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "as? result type (Option<T>)",
                at: span.into(),
            });
        };

        let target_cg = self
            .cg_ty_of(target_ty)
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "cast target type",
                at: span.into(),
            })?;
        let target_ptr_ty = match target_cg {
            CgTy::Ref => self.llvm_gc_i8_ptr_type(),
            CgTy::String => self.llvm_scoop_string_ptr_type(),
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "as? target (ref)",
                    at: span.into(),
                });
            }
        };

        let v = self.codegen_expr(expr)?;
        let v = match v.ty {
            CgTy::Ref => v,
            CgTy::String => self.coerce_value(expr.span, v, CgTy::Ref)?,
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "as? operand (ref)",
                    at: span.into(),
                });
            }
        };
        let Some(raw) = v.value else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "as? operand value",
                at: span.into(),
            });
        };
        let BasicValueEnum::PointerValue(obj_ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "as? operand type",
                at: span.into(),
            });
        };

        let is_ok = self.codegen_ref_is_instance_of(span, obj_ptr, target_ty)?;

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

        let ok_bb = self.context.append_basic_block(func, "asq_ok");
        let fail_bb = self.context.append_basic_block(func, "asq_fail");
        let merge_bb = self.context.append_basic_block(func, "asq_merge");
        self.builder
            .build_conditional_branch(is_ok, ok_bb, fail_bb)?;

        // --- ok：Some(casted) ---
        self.builder.position_at_end(ok_bb);
        let casted_ptr = self
            .builder
            .build_pointer_cast(obj_ptr, target_ptr_ty, "asq_cast_ptr")?;
        let casted = CgValue {
            ty: target_cg,
            value: Some(casted_ptr.into()),
        };
        let payload = self.coerce_enum_payload(span, casted, target_cg)?;
        let some_v = self.build_enum_value(span, option_ty, 0, payload)?;
        let some_raw = some_v.value.ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "as? Some value",
            at: span.into(),
        })?;
        self.builder.build_unconditional_branch(merge_bb)?;

        // --- fail：None ---
        self.builder.position_at_end(fail_bb);
        let none_v = self.build_enum_value(span, option_ty, 1, CgEnumPayload::default())?;
        let none_raw = none_v.value.ok_or(LlvmEmitError::UnsupportedMainBody {
            kind: "as? None value",
            at: span.into(),
        })?;
        self.builder.build_unconditional_branch(merge_bb)?;

        // --- merge ---
        self.builder.position_at_end(merge_bb);
        let llvm_option_ty = self.llvm_enum_value_type(span, option_ty)?;
        let phi = self.builder.build_phi(llvm_option_ty, "asq_value")?;
        phi.add_incoming(&[(&some_raw, ok_bb), (&none_raw, fail_bb)]);
        let out_raw = phi.as_basic_value();

        Ok(CgValue {
            ty: CgTy::Enum(option_ty),
            value: Some(out_raw),
        })
    }

    /// 运行期类型检查：判断 `obj` 是否为 `target_ty` 的实例。
    ///
    /// 约定（v0，T1509b）：
    /// - 若 `obj == NULL`：返回 false（避免解引用 NULL）；
    /// - `Any`：只要非 NULL 即为 true（不依赖 type_desc）；
    /// - class：沿 `type_desc.parent_type_desc` 向上查找；
    /// - interface：扫描 itable entries 是否包含 `interface_id`。
    fn codegen_ref_is_instance_of(
        &mut self,
        at: crate::span::Span,
        obj: PointerValue<'ctx>,
        target_ty: TypeId,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let obj_is_null = self.builder.build_is_null(obj, "isa_obj_is_null")?;

        // fast path：`x is Any` 只需要判空。
        if matches!(self.types.kind(target_ty), TypeKind::Ref(RefTypeKind::Any)) {
            return Ok(self.builder.build_not(obj_is_null, "isa_any_nonnull")?);
        }

        // 对其它 target：obj 为 NULL 时直接 false，避免解引用对象头。
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

        let null_bb = self.context.append_basic_block(func, "isa_obj_null");
        let nonnull_bb = self.context.append_basic_block(func, "isa_obj_nonnull");
        let done_bb = self.context.append_basic_block(func, "isa_done");
        self.builder
            .build_conditional_branch(obj_is_null, null_bb, nonnull_bb)?;

        // null -> done(false)
        self.builder.position_at_end(null_bb);
        self.builder.build_unconditional_branch(done_bb)?;

        // nonnull -> 计算真实检查 -> done
        self.builder.position_at_end(nonnull_bb);
        let inner_ok = self.codegen_ref_is_instance_of_nonnull(at, obj, target_ty)?;
        let after_check_bb =
            self.builder
                .get_insert_block()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "builder has no insert block",
                    at: at.into(),
                })?;
        self.builder.build_unconditional_branch(done_bb)?;

        // done：phi 合并
        self.builder.position_at_end(done_bb);
        let phi = self
            .builder
            .build_phi(self.context.bool_type(), "isa_result")?;
        phi.add_incoming(&[
            (&self.context.bool_type().const_int(0, false), null_bb),
            (&inner_ok, after_check_bb),
        ]);
        Ok(phi.as_basic_value().into_int_value())
    }

    fn codegen_ref_is_instance_of_nonnull(
        &mut self,
        at: crate::span::Span,
        obj: PointerValue<'ctx>,
        target_ty: TypeId,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        match self.types.kind(target_ty) {
            TypeKind::Ref(RefTypeKind::Any) => Ok(self.context.bool_type().const_int(1, false)),
            TypeKind::Ref(RefTypeKind::String) => {
                let desc = self.get_or_create_string_type_desc_global(at)?;
                let target_i8 = desc.as_pointer_value().const_cast(self.llvm_i8_ptr_type());
                self.codegen_type_desc_chain_contains_target(at, obj, target_i8)
            }
            TypeKind::Ref(RefTypeKind::Nominal(nominal)) => {
                // interface：用 itable 判断是否实现。
                if let Some(info) = self.interfaces.get(&nominal.fqn) {
                    return self.codegen_itable_contains_interface_id(at, obj, info.interface_id);
                }

                // class：沿 parent 链查找。
                if self.class_inits.contains_key(&nominal.fqn) {
                    let desc = self.get_or_create_class_type_desc_global(at, &nominal.fqn)?;
                    let target_i8 = desc.as_pointer_value().const_cast(self.llvm_i8_ptr_type());
                    return self.codegen_type_desc_chain_contains_target(at, obj, target_i8);
                }

                Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "type check target (nominal ref)",
                    at: at.into(),
                })
            }
            _ => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "type check target type",
                at: at.into(),
            }),
        }
    }

    /// `class` 类型判断：检查 `obj.header.type_desc` 的 parent 链是否包含 `target_desc_i8`。
    fn codegen_type_desc_chain_contains_target(
        &mut self,
        at: crate::span::Span,
        obj: PointerValue<'ctx>,
        target_desc_i8: PointerValue<'ctx>,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let i8_ptr_ty = self.llvm_i8_ptr_type();

        // 读取 `header.type_desc`（i8*）。
        let header_ty = self.llvm_gc_object_header_type();
        let header_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
        let header_ptr = self
            .builder
            .build_pointer_cast(obj, header_ptr_ty, "isa_hdr_ptr")?;
        let type_desc_ptr =
            self.builder
                .build_struct_gep(header_ty, header_ptr, 1, "isa_type_desc_gep")?;
        let type_desc_i8 = self
            .builder
            .build_load(i8_ptr_ty, type_desc_ptr, "isa_type_desc")?
            .into_pointer_value();

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

        // while (cur != NULL) { if (cur == target) return true; cur = cur.parent; } return false
        let loop_bb = self.context.append_basic_block(func, "isa_loop");
        let check_bb = self.context.append_basic_block(func, "isa_check");
        let advance_bb = self.context.append_basic_block(func, "isa_advance");
        let hit_bb = self.context.append_basic_block(func, "isa_hit");
        let done_bb = self.context.append_basic_block(func, "isa_done");

        self.builder.build_unconditional_branch(loop_bb)?;
        self.builder.position_at_end(loop_bb);

        let cur_phi = self.builder.build_phi(i8_ptr_ty, "isa_cur")?;
        cur_phi.add_incoming(&[(&type_desc_i8, insert_block)]);
        let cur_i8 = cur_phi.as_basic_value().into_pointer_value();

        let cur_is_null = self.builder.build_is_null(cur_i8, "isa_cur_is_null")?;
        self.builder
            .build_conditional_branch(cur_is_null, done_bb, check_bb)?;

        // check：cur == target ?
        self.builder.position_at_end(check_bb);
        let word_ty = self.int_type(IntTy {
            bits: self.host.word_bit_width(),
            signed: false,
        });
        let cur_int = self
            .builder
            .build_ptr_to_int(cur_i8, word_ty, "isa_cur_int")?;
        let target_int =
            self.builder
                .build_ptr_to_int(target_desc_i8, word_ty, "isa_target_int")?;
        let eq = self
            .builder
            .build_int_compare(IntPredicate::EQ, cur_int, target_int, "isa_eq")?;
        self.builder
            .build_conditional_branch(eq, hit_bb, advance_bb)?;

        // advance：cur = cur.parent
        self.builder.position_at_end(advance_bb);
        let desc_ty = self.llvm_scoop_type_descriptor_type();
        let desc_ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        let cur_desc = self
            .builder
            .build_pointer_cast(cur_i8, desc_ptr_ty, "isa_desc")?;
        let parent_ptr = self
            .builder
            .build_struct_gep(desc_ty, cur_desc, 11, "isa_parent_gep")?;
        let parent_desc = self
            .builder
            .build_load(desc_ptr_ty, parent_ptr, "isa_parent")?
            .into_pointer_value();
        let parent_i8 = self
            .builder
            .build_pointer_cast(parent_desc, i8_ptr_ty, "isa_parent_i8")?;
        cur_phi.add_incoming(&[(&parent_i8, advance_bb)]);
        self.builder.build_unconditional_branch(loop_bb)?;

        // hit：return true
        self.builder.position_at_end(hit_bb);
        self.builder.build_unconditional_branch(done_bb)?;

        // done：phi 合并 true/false
        self.builder.position_at_end(done_bb);
        let phi = self
            .builder
            .build_phi(self.context.bool_type(), "isa_found")?;
        phi.add_incoming(&[
            (&self.context.bool_type().const_int(0, false), loop_bb),
            (&self.context.bool_type().const_int(1, false), hit_bb),
        ]);
        Ok(phi.as_basic_value().into_int_value())
    }

    /// `interface` 类型判断：扫描 `obj.type_desc.itable` 是否包含 `interface_id`。
    fn codegen_itable_contains_interface_id(
        &mut self,
        at: crate::span::Span,
        obj: PointerValue<'ctx>,
        interface_id: u64,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let i32_ty = self.context.i32_type();
        let i64_ty = self.context.i64_type();
        let i8_ptr_ty = self.llvm_i8_ptr_type();

        // obj 指向对象头起始地址：先把它 cast 为 `ScoopGcObjectHeader*`。
        let header_ty = self.llvm_gc_object_header_type();
        let header_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
        let header_ptr =
            self.builder
                .build_pointer_cast(obj, header_ptr_ty, "isa_iface_hdr_ptr")?;

        // header.type_desc : i8*
        let type_desc_ptr =
            self.builder
                .build_struct_gep(header_ty, header_ptr, 1, "isa_iface_type_desc_gep")?;
        let type_desc_i8 = self
            .builder
            .build_load(i8_ptr_ty, type_desc_ptr, "isa_iface_load_type_desc")?
            .into_pointer_value();

        // type_desc.itable : i8*
        let desc_ty = self.llvm_scoop_type_descriptor_type();
        let desc_ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        let desc_ptr =
            self.builder
                .build_pointer_cast(type_desc_i8, desc_ptr_ty, "isa_iface_type_desc")?;
        let itable_field_ptr =
            self.builder
                .build_struct_gep(desc_ty, desc_ptr, 12, "isa_iface_itable_gep")?;
        let itable_i8 = self
            .builder
            .build_load(i8_ptr_ty, itable_field_ptr, "isa_iface_load_itable")?
            .into_pointer_value();

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

        // itable == NULL -> false
        let itable_is_null = self
            .builder
            .build_is_null(itable_i8, "isa_iface_itable_is_null")?;
        let null_bb = self
            .context
            .append_basic_block(func, "isa_iface_itable_null");
        let lookup_bb = self
            .context
            .append_basic_block(func, "isa_iface_itable_lookup");
        let done_bb = self
            .context
            .append_basic_block(func, "isa_iface_itable_done");
        self.builder
            .build_conditional_branch(itable_is_null, null_bb, lookup_bb)?;

        self.builder.position_at_end(null_bb);
        self.builder.build_unconditional_branch(done_bb)?;

        // lookup：扫描 entries[idx].interface_id
        self.builder.position_at_end(lookup_bb);
        let itable_ty = self.llvm_scoop_itable_type();
        let itable_ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        let itable_ptr =
            self.builder
                .build_pointer_cast(itable_i8, itable_ptr_ty, "isa_iface_itable_ptr")?;

        let len_ptr =
            self.builder
                .build_struct_gep(itable_ty, itable_ptr, 0, "isa_iface_len_gep")?;
        let len_i32 = self
            .builder
            .build_load(i32_ty, len_ptr, "isa_iface_len")?
            .into_int_value();

        let entry_ty = self.llvm_scoop_itable_entry_type();
        let entries_field_ptr =
            self.builder
                .build_struct_gep(itable_ty, itable_ptr, 2, "isa_iface_entries_gep")?;
        let entry_ptr_ty = self.llvm_ptr_type(AddressSpace::default());
        let entries_base = self.builder.build_pointer_cast(
            entries_field_ptr,
            entry_ptr_ty,
            "isa_iface_entries",
        )?;

        let loop_bb = self.context.append_basic_block(func, "isa_iface_loop");
        let body_bb = self.context.append_basic_block(func, "isa_iface_body");
        let hit_bb = self.context.append_basic_block(func, "isa_iface_hit");
        let miss_bb = self.context.append_basic_block(func, "isa_iface_miss");

        self.builder.build_unconditional_branch(loop_bb)?;
        self.builder.position_at_end(loop_bb);

        let idx_phi = self.builder.build_phi(i32_ty, "isa_iface_idx")?;
        idx_phi.add_incoming(&[(&i32_ty.const_zero(), lookup_bb)]);
        let idx_i32 = idx_phi.as_basic_value().into_int_value();

        let cond = self.builder.build_int_compare(
            IntPredicate::ULT,
            idx_i32,
            len_i32,
            "isa_iface_idx_lt_len",
        )?;
        self.builder
            .build_conditional_branch(cond, body_bb, done_bb)?;

        // body：比较 interface_id
        self.builder.position_at_end(body_bb);
        let entry_ptr = unsafe {
            self.builder.build_in_bounds_gep(
                entry_ty,
                entries_base,
                &[idx_i32],
                "isa_iface_entry_ptr",
            )?
        };
        let id_ptr = self
            .builder
            .build_struct_gep(entry_ty, entry_ptr, 0, "isa_iface_id_gep")?;
        let id_i64 = self
            .builder
            .build_load(i64_ty, id_ptr, "isa_iface_id")?
            .into_int_value();

        let target_id = i64_ty.const_int(interface_id, false);
        let ok = self.builder.build_int_compare(
            IntPredicate::EQ,
            id_i64,
            target_id,
            "isa_iface_id_eq",
        )?;
        self.builder.build_conditional_branch(ok, hit_bb, miss_bb)?;

        // miss：idx++ 继续 loop
        self.builder.position_at_end(miss_bb);
        let next = self.builder.build_int_add(
            idx_i32,
            i32_ty.const_int(1, false),
            "isa_iface_idx_next",
        )?;
        idx_phi.add_incoming(&[(&next, miss_bb)]);
        self.builder.build_unconditional_branch(loop_bb)?;

        // hit：直接 done
        self.builder.position_at_end(hit_bb);
        self.builder.build_unconditional_branch(done_bb)?;

        // done：phi 合并 false/true
        self.builder.position_at_end(done_bb);
        let phi = self
            .builder
            .build_phi(self.context.bool_type(), "isa_iface_found")?;
        phi.add_incoming(&[
            (&self.context.bool_type().const_int(0, false), null_bb),
            (&self.context.bool_type().const_int(0, false), loop_bb),
            (&self.context.bool_type().const_int(1, false), hit_bb),
        ]);
        Ok(phi.as_basic_value().into_int_value())
    }

    fn codegen_float_binary_same_type(
        &mut self,
        span: crate::span::Span,
        op: ast::BinaryOp,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let (l_raw, l_ty) =
            self.codegen_expr(lhs)?
                .as_float()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "float binary op lhs",
                    at: span.into(),
                })?;
        let (r_raw, r_ty) =
            self.codegen_expr(rhs)?
                .as_float()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "float binary op rhs",
                    at: span.into(),
                })?;

        let out_ty = self.unify_float_cg_types(lhs, l_ty, rhs, r_ty).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "float binary op type",
                at: span.into(),
            },
        )?;

        let l = self.cast_float(l_raw, l_ty, out_ty)?;
        let r = self.cast_float(r_raw, r_ty, out_ty)?;

        let out = match op {
            ast::BinaryOp::Add => self.builder.build_float_add(l, r, "fadd")?,
            ast::BinaryOp::Sub => self.builder.build_float_sub(l, r, "fsub")?,
            ast::BinaryOp::Mul => self.builder.build_float_mul(l, r, "fmul")?,
            ast::BinaryOp::Div => self.builder.build_float_div(l, r, "fdiv")?,
            ast::BinaryOp::Rem => self.builder.build_float_rem(l, r, "frem")?,
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "float binary op",
                    at: span.into(),
                });
            }
        };

        Ok(CgValue::float(out, out_ty))
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

        // 2) mask：shiftCount & (bitWidth - 1)，避免 LLVM 对"超范围 shift"的 UB。
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

    fn codegen_float_compare(
        &mut self,
        span: crate::span::Span,
        op: ast::BinaryOp,
        lhs: &hir::Expr,
        rhs: &hir::Expr,
    ) -> Result<CgValue<'ctx>, LlvmEmitError> {
        let (l_raw, l_ty) =
            self.codegen_expr(lhs)?
                .as_float()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "float comparison lhs",
                    at: span.into(),
                })?;
        let (r_raw, r_ty) =
            self.codegen_expr(rhs)?
                .as_float()
                .ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "float comparison rhs",
                    at: span.into(),
                })?;

        let float_ty = self.unify_float_cg_types(lhs, l_ty, rhs, r_ty).ok_or(
            LlvmEmitError::UnsupportedMainBody {
                kind: "float comparison operand type",
                at: span.into(),
            },
        )?;

        let l = self.cast_float(l_raw, l_ty, float_ty)?;
        let r = self.cast_float(r_raw, r_ty, float_ty)?;

        let pred = match op {
            ast::BinaryOp::Lt => FloatPredicate::OLT,
            ast::BinaryOp::Le => FloatPredicate::OLE,
            ast::BinaryOp::Gt => FloatPredicate::OGT,
            ast::BinaryOp::Ge => FloatPredicate::OGE,
            _ => {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "float comparison operator",
                    at: span.into(),
                });
            }
        };

        Ok(CgValue::bool(
            self.builder.build_float_compare(pred, l, r, "fcmp")?,
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

        // T0107: String == String — call scoop_string_equals(a, b) -> i64 (1=equal, 0=not)
        if matches!((lhs_v.ty, rhs_v.ty), (CgTy::String, CgTy::String)) {
            let BasicValueEnum::PointerValue(l) =
                lhs_v.value.ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "equality lhs string value",
                    at: span.into(),
                })?
            else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "equality lhs string type",
                    at: span.into(),
                });
            };
            let BasicValueEnum::PointerValue(r) =
                rhs_v.value.ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "equality rhs string value",
                    at: span.into(),
                })?
            else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "equality rhs string type",
                    at: span.into(),
                });
            };
            let fn_val = self.declare_runtime_string_equals();
            let call = self
                .builder
                .build_call(fn_val, &[l.into(), r.into()], "str_eq")?;
            let raw_result =
                call.try_as_basic_value()
                    .basic()
                    .ok_or(LlvmEmitError::UnsupportedMainBody {
                        kind: "String equals return value",
                        at: span.into(),
                    })?;
            let BasicValueEnum::IntValue(eq_i64) = raw_result else {
                return Err(LlvmEmitError::UnsupportedMainBody {
                    kind: "String equals return type",
                    at: span.into(),
                });
            };
            let is_eq = self.builder.build_int_compare(
                IntPredicate::NE,
                eq_i64,
                self.context.i64_type().const_zero(),
                "str_eq_bool",
            )?;
            let result = match op {
                ast::BinaryOp::Eq => is_eq,
                ast::BinaryOp::Ne => self.builder.build_not(is_eq, "str_ne_bool")?,
                _ => unreachable!("filtered by caller"),
            };
            return Ok(CgValue::bool(result));
        }

        if let (Some((l_raw, l_ty)), Some((r_raw, r_ty))) = (lhs_v.as_float(), rhs_v.as_float()) {
            let float_ty = self.unify_float_cg_types(lhs, l_ty, rhs, r_ty).ok_or(
                LlvmEmitError::UnsupportedMainBody {
                    kind: "equality float operand type",
                    at: span.into(),
                },
            )?;
            let l = self.cast_float(l_raw, l_ty, float_ty)?;
            let r = self.cast_float(r_raw, r_ty, float_ty)?;
            let pred = match op {
                ast::BinaryOp::Eq => FloatPredicate::OEQ,
                ast::BinaryOp::Ne => FloatPredicate::UNE,
                _ => unreachable!("filtered by caller"),
            };
            return Ok(CgValue::bool(
                self.builder.build_float_compare(pred, l, r, "fcmp_eq")?,
            ));
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
            // T1612: Nothing (bottom type) coerces to any target type.
            // This only occurs on unreachable paths (after Raise.raise, etc.),
            // so we return a phantom default value of the target type.
            (CgTy::Never, _) => self.default_value(at, target),
            (CgTy::Unit, CgTy::Unit) => Ok(CgValue::unit()),
            (CgTy::Unit, CgTy::Ref) => {
                // early stage：允许把 `Unit` 装箱到 `Any`。
                //
                // 说明：
                // - 当前阶段有一部分"语句位置"的表达式仍会被类型系统视为 `Any`（例如某些 `block/when`），
                //   因此后端需要支持 `Unit -> Any` 的值提升；
                // - v0 阶段 runtime type descriptor 仍是占位（NULL），这里只保证"可执行/可回归"。
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
                // - 当前阶段 runtime type descriptor 仍是占位（NULL），因此这里只保证"可执行/可回归"，
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
                let widened =
                    self.builder
                        .build_int_z_extend(v, self.int_type(word), "box_bool_to_word")?;
                let boxed = self.codegen_box_int_to_ref(at, widened, word)?;
                Ok(CgValue {
                    ty: CgTy::Ref,
                    value: Some(boxed.into()),
                })
            }
            (CgTy::Float64, CgTy::Float64) | (CgTy::Float32, CgTy::Float32) => Ok(value),
            (CgTy::Float64, CgTy::Float32) | (CgTy::Float32, CgTy::Float64) => {
                let (v, from) = value.as_float().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "float value",
                    at: at.into(),
                })?;
                let out = self.cast_float(v, from, target)?;
                Ok(CgValue::float(out, target))
            }
            (CgTy::Int(from), CgTy::Int(to)) => {
                if let Some(bits) = self.int_literal_bits_from_source_span_if_present(at, to)? {
                    return Ok(CgValue::int(self.int_type(to).const_int(bits, false), to));
                }
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

                let casted = self.builder.build_pointer_cast(
                    ptr,
                    self.llvm_gc_i8_ptr_type(),
                    "str_to_ref",
                )?;
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
        let boxed_ty = self.llvm_boxed_unit_type();
        let obj_size_bytes = self.target_data.get_store_size(&boxed_ty);

        let size_v = self.context.i64_type().const_int(obj_size_bytes, false);

        let desc = self.get_or_create_boxed_unit_type_desc_global(at)?;
        let desc_i8 = self.builder.build_pointer_cast(
            desc.as_pointer_value(),
            self.llvm_i8_ptr_type(),
            "boxed_unit_type_desc_i8",
        )?;
        let rt_alloc = self.declare_runtime_alloc_typed();
        let call = self.builder.build_call(
            rt_alloc,
            &[desc_i8.into(), size_v.into()],
            "rt_alloc_box_unit",
        )?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc_typed return value",
                at: at.into(),
            })?;

        let BasicValueEnum::PointerValue(raw_ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc_typed return type",
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
        // - 当前阶段由 runtime 的 `scoop_alloc_typed` 初始化对象头字段：
        //   - `next = NULL`
        //   - `type_desc = <boxed-int type desc>`
        //   - `size_bytes = alloc_size`
        //   - `flags/mark = 0`
        //
        // 注意：这里不尝试做"复用 box 类型"或 cache；LLVM named struct 会在 module 内复用。
        let target = self.target_layout();
        let payload_size = u64::from(value_ty.bits).div_ceil(8);
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

        let size_v = self.context.i64_type().const_int(total_size, false);

        let desc = self.get_or_create_boxed_int_type_desc_global(at, value_ty)?;
        let desc_i8 = self.builder.build_pointer_cast(
            desc.as_pointer_value(),
            self.llvm_i8_ptr_type(),
            "boxed_int_type_desc_i8",
        )?;
        let rt_alloc = self.declare_runtime_alloc_typed();
        let call =
            self.builder
                .build_call(rt_alloc, &[desc_i8.into(), size_v.into()], "rt_alloc_box")?;
        let raw = call
            .try_as_basic_value()
            .basic()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc_typed return value",
                at: at.into(),
            })?;

        let BasicValueEnum::PointerValue(raw_ptr) = raw else {
            return Err(LlvmEmitError::UnsupportedMainBody {
                kind: "scoop_alloc_typed return type",
                at: at.into(),
            });
        };

        // 写入 payload（对象头由 runtime 初始化）。
        let boxed_ty = self.llvm_boxed_int_type(value_ty);
        let boxed_ptr_ty = self.llvm_ptr_type(self.gc_address_space());
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

    fn cast_float(
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

    fn coerce_exit_code(
        &mut self,
        at: crate::span::Span,
        value: CgValue<'ctx>,
    ) -> Result<IntValue<'ctx>, LlvmEmitError> {
        let i32_ty = self.context.i32_type();

        match value.ty {
            CgTy::Unit | CgTy::Never => Ok(i32_ty.const_int(0, false)),
            CgTy::Bool => {
                let b = value.as_bool().ok_or(LlvmEmitError::UnsupportedMainBody {
                    kind: "exit bool",
                    at: at.into(),
                })?;
                Ok(self.builder.build_int_z_extend(b, i32_ty, "exit_bool")?)
            }
            CgTy::Float64 | CgTy::Float32 => Err(LlvmEmitError::UnsupportedMainBody {
                kind: "float exit code",
                at: at.into(),
            }),
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

    /// 在当前 compilation unit 的 `TypeStore` 中查找 `() -> Unit / Pure` 的函数类型。
    ///
    /// 用途：
    /// - 一些 sysroot API（例如 `scoop.sync.Once.run`）在 early stage 是"只有声明没有 body 的外部落点"，
    ///   因此不在 `fun_index` 中；但 closure codegen 仍需要一个 expected function type 来确定参数绑定。
    fn lookup_pure_unit_closure_type(&self) -> Option<TypeId> {
        let unit = self
            .types
            .iter_ids()
            .find(|id| matches!(self.types.kind(*id), TypeKind::Value(ValueTypeKind::Unit)))?;

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

    fn int_type(&self, ty: IntTy) -> IntType<'ctx> {
        self.context.custom_width_int_type(ty.bits)
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

        let inst = ptr
            .as_instruction_value()
            .ok_or(LlvmEmitError::UnsupportedMainBody {
                kind: "alloca instruction value",
                at: at.into(),
            })?;
        inst.set_alignment(aligned)?;
        Ok(())
    }
}

fn string_literal_parse_reason(err: StringLiteralParseError) -> &'static str {
    match err {
        StringLiteralParseError::Invalid => "包含无效引号、转义或 Unicode 码点",
        StringLiteralParseError::Interpolated => "插值字符串当前阶段不能直接按普通字符串解析",
        StringLiteralParseError::InvalidUtf8 => "解码后的字节不是有效 UTF-8",
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

fn stable_hash64(text: &str) -> u64 {
    let digest = Sha256::digest(text.as_bytes());
    let bytes: [u8; 8] = digest[0..8].try_into().expect("sha256 output is 32 bytes");
    u64::from_le_bytes(bytes)
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

fn checked_positive_int_literal_bits(value: u128, int_ty: IntTy) -> Option<u128> {
    let max = if int_ty.signed {
        signed_int_max(int_ty.bits)
    } else {
        unsigned_int_max(int_ty.bits)
    };
    (value <= max).then_some(value)
}

fn checked_negated_int_literal_bits(value: u128, int_ty: IntTy) -> Option<u128> {
    if !int_ty.signed {
        return None;
    }

    let min_abs = signed_int_min_abs(int_ty.bits);
    if value > min_abs {
        return None;
    }

    Some(mask_to_bits(0u128.wrapping_sub(value), int_ty.bits))
}

fn unsigned_int_max(bits: u32) -> u128 {
    if bits == 0 {
        return 0;
    }
    if bits >= 128 {
        return u128::MAX;
    }
    (1u128 << bits) - 1
}

fn signed_int_max(bits: u32) -> u128 {
    if bits <= 1 {
        return 0;
    }
    if bits >= 128 {
        return i128::MAX as u128;
    }
    (1u128 << (bits - 1)) - 1
}

fn signed_int_min_abs(bits: u32) -> u128 {
    if bits == 0 {
        return 0;
    }
    if bits >= 128 {
        return 1u128 << 127;
    }
    1u128 << (bits - 1)
}

fn source_text_int_literal_body(text: &str) -> Option<(bool, &str)> {
    let (negative, body) = if let Some(rest) = text.strip_prefix('-') {
        (true, rest)
    } else {
        (false, text)
    };

    if body.is_empty() || !body.as_bytes().first().is_some_and(u8::is_ascii_digit) {
        return None;
    }

    if let Some(rest) = body.strip_prefix("0x").or_else(|| body.strip_prefix("0X")) {
        if rest
            .bytes()
            .all(|b| char::from(b).is_ascii_hexdigit() || b == b'_')
        {
            return Some((negative, body));
        }
        return None;
    }

    if let Some(rest) = body.strip_prefix("0b").or_else(|| body.strip_prefix("0B")) {
        if rest.bytes().all(|b| matches!(b, b'0' | b'1' | b'_')) {
            return Some((negative, body));
        }
        return None;
    }

    if body
        .bytes()
        .all(|b| char::from(b).is_ascii_digit() || b == b'_')
    {
        return Some((negative, body));
    }

    None
}

fn parse_f_string_text_bytes(raw: bool, text: &str) -> Result<Vec<u8>, StringLiteralParseError> {
    // f-string 的 Text 片段来自 parser 拆分后的"内容区间 slice"，不包含包裹引号。
    // 这里需要补齐两类语义：
    // - `{{` / `}}`：字面量大括号（spec §8.2）；
    // - 非 raw f-string：支持最小转义（与普通字符串一致）。
    if raw {
        let undoubled = undouble_braces(text);
        return Ok(undoubled.into_bytes());
    }

    // 非 raw：先在源码层"去双大括号"，并避免把 `\u{...}` 的 `{}` 当作候选；
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
                for c in chars.by_ref() {
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
