//! typed/lowered HIR → generic early MIR / ANF template lowering。
//!
//! 说明：
//! - 当前入口仍主要服务 `scoop dump-mir` 与 `tests/fixtures/mir/**` 的回归；
//! - lowering 会显式消费 typed/shared HIR side tables，把 dispatch / resume / perform / pattern
//!   等语言级事实收口到 MIR；
//! - 这里不负责 materialize monomorphic instance，也不编码 LLVM/backend-specific 细节；
//! - 未覆盖的表达式/语句继续以 `Todo(...)` 占位，优先保证边界清晰、输出稳定、不 panic。

use std::collections::{HashMap, HashSet};

use miette::Diagnostic;
use thiserror::Error;

use crate::ast;
use crate::effect_refactor_pipeline::{HandleArmContractKind, TypedHirEffectContracts};
use crate::hir;
use crate::session::Session;
use crate::source::SourceFile;
use crate::span::Span;
use crate::ty::{BuiltinTypes, EffectRow, NominalType, RefTypeKind, TypeId, TypeKind, TypeStore};

use super::{
    BasicBlock, BasicBlockId, Body, CallArg, CallKind, ConstValue, DispatchMetadata, File, FunDecl,
    HandleMetadata, HandlerArm, HandlerArmKind, Item, LocalDecl, LocalId, LocalSourceKind,
    MemberAccessMetadata, MemberTarget, MirValidationError, Operand, Param, Pattern,
    PatternBindingStep, PerformArg, PerformMetadata, ResumeMetadata, Rvalue, SiteId, Statement,
    StatementKind, StoredContinuationRoutePublication, StoredContinuationValueRoute, Terminator,
    TerminatorKind, TopLevelRef, UnwindAction,
};

/// MIR lowering 需要消费的最小共享事实。
///
/// 目标：
/// - 把 HIR/typecheck 已确认的调用语义收口成 MIR lowering 可直接查询的 backend-agnostic 输入；
/// - 避免 MIR 阶段重新回到 LLVM vtable/itable 细节或 `Continuation.resume` 名字推断。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MirSiteContractSource {
    LegacyFallbacks,
    RefactorTyped,
}

#[derive(Debug, Clone)]
pub(crate) struct MirLoweringFacts {
    site_contract_source: MirSiteContractSource,
    dispatch_call_sites: HashMap<hir::DispatchCallSite, DispatchTargetKind>,
    legacy_resume_site_spans: HashSet<Span>,
    legacy_outward_resume_site_spans: HashSet<Span>,
    legacy_perform_sites: HashMap<Span, PerformCallSiteInfo>,
    refactor_resume_sites: HashMap<hir::CallSite, ResumeMetadata>,
    refactor_perform_sites: HashMap<hir::CallSite, PerformMetadata>,
    refactor_handle_sites: HashMap<hir::CallSite, RefactorHandleSiteInfo>,
    when_pat_binding_tys: HashMap<Span, TypeId>,
    top_level_fun_call_sites: HashMap<hir::CallSite, ast::TopLevelFunCallBinding>,
    member_value_tys: HashMap<String, TypeId>,
}

impl Default for MirLoweringFacts {
    fn default() -> Self {
        Self {
            site_contract_source: MirSiteContractSource::LegacyFallbacks,
            dispatch_call_sites: HashMap::new(),
            legacy_resume_site_spans: HashSet::new(),
            legacy_outward_resume_site_spans: HashSet::new(),
            legacy_perform_sites: HashMap::new(),
            refactor_resume_sites: HashMap::new(),
            refactor_perform_sites: HashMap::new(),
            refactor_handle_sites: HashMap::new(),
            when_pat_binding_tys: HashMap::new(),
            top_level_fun_call_sites: HashMap::new(),
            member_value_tys: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DispatchTargetKind {
    Virtual,
    Interface,
}

#[derive(Debug, Clone)]
struct PerformCallSiteInfo {
    arg_mapping: Vec<usize>,
    payload_tuple_ty: Option<TypeId>,
}

#[derive(Debug, Clone)]
struct RefactorHandleSiteInfo {
    metadata: HandleMetadata,
    arms: Vec<HandlerArm>,
}

impl MirLoweringFacts {
    pub(crate) fn from_lowered_hir(lowered: &hir::LoweredHir) -> Self {
        Self::from_hir_side_tables_and_resume_spans(
            &lowered.dispatch_call_sites,
            lowered
                .continuation_resume_call_sites
                .iter()
                .map(|site| site.span),
            lowered
                .non_pure_continuation_resume_call_sites
                .iter()
                .map(|site| site.span),
            &lowered.effect_op_call_sites,
            &lowered.when_pat_binding_tys,
            &lowered.top_level_fun_call_sites,
        )
        .with_member_value_types(lowered)
    }

    pub(crate) fn from_refactor_typed_handoff(
        lowered: &hir::LoweredHir,
        contracts: &TypedHirEffectContracts,
    ) -> Self {
        let mut facts = Self::default();

        for (site, kind) in &lowered.dispatch_call_sites {
            facts.dispatch_call_sites.insert(
                site.clone(),
                match kind {
                    hir::DispatchCallKind::Virtual => DispatchTargetKind::Virtual,
                    hir::DispatchCallKind::Interface => DispatchTargetKind::Interface,
                },
            );
        }

        for (site, ty) in &lowered.when_pat_binding_tys {
            facts.when_pat_binding_tys.insert(site.decl_span, *ty);
        }

        facts
            .top_level_fun_call_sites
            .extend(lowered.top_level_fun_call_sites.clone());
        facts = facts.with_member_value_types(lowered);

        facts.with_refactor_typed_contracts(contracts)
    }

    pub(crate) fn from_hir_side_tables_and_resume_spans(
        dispatch_call_sites: &hir::DispatchCallSiteIndex,
        legacy_resume_site_spans: impl IntoIterator<Item = Span>,
        legacy_outward_resume_site_spans: impl IntoIterator<Item = Span>,
        effect_op_call_sites: &hir::EffectOpCallSiteIndex,
        when_pat_binding_tys: &hir::WhenPatBindingTypeIndex,
        top_level_fun_call_sites: &hir::TopLevelFunCallSiteIndex,
    ) -> Self {
        let mut facts = Self::default();

        for (site, kind) in dispatch_call_sites {
            facts.dispatch_call_sites.insert(
                site.clone(),
                match kind {
                    hir::DispatchCallKind::Virtual => DispatchTargetKind::Virtual,
                    hir::DispatchCallKind::Interface => DispatchTargetKind::Interface,
                },
            );
        }

        facts.legacy_resume_site_spans = legacy_resume_site_spans.into_iter().collect();
        facts.legacy_outward_resume_site_spans =
            legacy_outward_resume_site_spans.into_iter().collect();
        facts.with_hir_side_tables(
            effect_op_call_sites,
            when_pat_binding_tys,
            top_level_fun_call_sites,
        )
    }

    pub(crate) fn with_hir_side_tables(
        mut self,
        effect_op_call_sites: &hir::EffectOpCallSiteIndex,
        when_pat_binding_tys: &hir::WhenPatBindingTypeIndex,
        top_level_fun_call_sites: &hir::TopLevelFunCallSiteIndex,
    ) -> Self {
        for (site, info) in effect_op_call_sites {
            self.legacy_perform_sites.insert(
                site.span,
                PerformCallSiteInfo {
                    arg_mapping: info.arg_mapping.clone(),
                    payload_tuple_ty: info.payload_tuple_ty,
                },
            );
        }

        for (site, ty) in when_pat_binding_tys {
            self.when_pat_binding_tys.insert(site.decl_span, *ty);
        }

        self.top_level_fun_call_sites
            .extend(top_level_fun_call_sites.clone());

        self
    }

    fn with_member_value_types(mut self, lowered: &hir::LoweredHir) -> Self {
        for class in lowered.class_inits.values() {
            for field in &class.fields {
                self.member_value_tys.insert(field.fqn.clone(), field.ty);
            }
        }

        for layout in lowered.struct_layouts.values() {
            for field in &layout.fields {
                if let Some(ty) = field.ty {
                    self.member_value_tys.insert(field.fqn.clone(), ty);
                }
            }
        }

        for object in lowered.object_inits.values() {
            for property in object.properties.values() {
                self.member_value_tys
                    .insert(format!("{}.{}", object.fqn, property.name), property.ty);
            }
        }

        self
    }

    pub(crate) fn with_refactor_typed_contracts(
        mut self,
        contracts: &TypedHirEffectContracts,
    ) -> Self {
        self.site_contract_source = MirSiteContractSource::RefactorTyped;
        self.legacy_resume_site_spans.clear();
        self.legacy_outward_resume_site_spans.clear();
        self.legacy_perform_sites.clear();
        self.refactor_resume_sites.clear();
        self.refactor_perform_sites.clear();
        self.refactor_handle_sites.clear();

        for (call_site, contract) in contracts.continuation_resume_sites() {
            self.refactor_resume_sites.insert(
                call_site.clone(),
                ResumeMetadata {
                    continuation_ty: contract.receiver_ty(),
                    resume_ty: contract.resume_ty(),
                    answer_ty: contract.answer_ty(),
                    return_ty: contract.return_ty(),
                    out_effects: contract.out_effects().clone(),
                    runtime_error_effect_ty: contract.runtime_error_effect_ty(),
                    suspends_outward: !contract.out_effects().is_pure(),
                },
            );
        }

        for (call_site, contract) in contracts.perform_sites() {
            self.refactor_perform_sites.insert(
                call_site.clone(),
                PerformMetadata {
                    effect_ty: contract.effect_ty(),
                    payload_tuple_ty: contract.payload().ty(),
                    payload_component_tys: contract.payload().components().to_vec(),
                    arg_mapping: contract.arg_mapping().to_vec(),
                },
            );
        }

        for (call_site, contract) in contracts.handle_sites() {
            let arms = contract
                .arm_contracts()
                .iter()
                .map(|arm| HandlerArm {
                    op_fqn: arm.op_fqn().to_string(),
                    binder_count: arm.payload().components().len(),
                    binder_locals: Vec::new(),
                    continuation_local: None,
                    handled_effect_ty: arm.handled_effect_ty(),
                    payload_tuple_ty: arm.payload().ty(),
                    payload_component_tys: arm.payload().components().to_vec(),
                    body_ty: arm.body_ty(),
                    kind: match arm.kind() {
                        HandleArmContractKind::NonResuming => HandlerArmKind::NonResuming,
                        HandleArmContractKind::EscapeContinuation => {
                            HandlerArmKind::EscapeContinuation
                        }
                    },
                })
                .collect();
            self.refactor_handle_sites.insert(
                call_site.clone(),
                RefactorHandleSiteInfo {
                    metadata: HandleMetadata {
                        result_ty: contract.result_ty(),
                        body_result_ty: contract.body_result_ty(),
                        finally_result_ty: contract.finally_result_ty(),
                    },
                    arms,
                },
            );
        }

        self
    }

    fn uses_refactor_typed_contracts(&self) -> bool {
        self.site_contract_source == MirSiteContractSource::RefactorTyped
    }

    fn dispatch_target_kind(
        &self,
        source_path: &std::path::Path,
        call_span: Span,
        receiver_ty: TypeId,
    ) -> Option<DispatchTargetKind> {
        self.dispatch_call_sites
            .get(&hir::DispatchCallSite::new(
                source_path.to_path_buf(),
                call_span,
                receiver_ty,
            ))
            .copied()
    }

    fn top_level_fun_call_binding(
        &self,
        source_path: &std::path::Path,
        call_span: Span,
    ) -> Option<&ast::TopLevelFunCallBinding> {
        self.top_level_fun_call_sites
            .get(&hir::CallSite::new(source_path.to_path_buf(), call_span))
    }

    fn legacy_resume_site_matches(&self, span: Span) -> bool {
        self.legacy_resume_site_spans.contains(&span)
    }

    fn legacy_resume_site_suspends_outward(&self, span: Span) -> bool {
        self.legacy_outward_resume_site_spans.contains(&span)
    }

    fn legacy_perform_site_info(&self, span: Span) -> Option<&PerformCallSiteInfo> {
        self.legacy_perform_sites.get(&span)
    }

    fn refactor_resume_metadata(
        &self,
        source_path: &std::path::Path,
        span: Span,
    ) -> Option<&ResumeMetadata> {
        self.refactor_resume_sites
            .get(&hir::CallSite::new(source_path.to_path_buf(), span))
    }

    fn refactor_perform_metadata(
        &self,
        source_path: &std::path::Path,
        span: Span,
    ) -> Option<&PerformMetadata> {
        self.refactor_perform_sites
            .get(&hir::CallSite::new(source_path.to_path_buf(), span))
    }

    fn refactor_handle_site_info(
        &self,
        source_path: &std::path::Path,
        span: Span,
    ) -> Option<&RefactorHandleSiteInfo> {
        self.refactor_handle_sites
            .get(&hir::CallSite::new(source_path.to_path_buf(), span))
    }

    fn when_pat_binding_ty(&self, span: Span) -> Option<TypeId> {
        self.when_pat_binding_tys.get(&span).copied()
    }
}

/// MIR lowering 错误（当前阶段仅包装 HIR lowering 错误）。
#[derive(Debug, Error, Diagnostic)]
pub enum MirLowerError {
    #[error(transparent)]
    #[diagnostic(transparent)]
    Hir(#[from] hir::HirLowerError),
    #[error("refactor direct-style MIR validation failed for `{fqn}`: {error}")]
    InvalidRefactorMir {
        fqn: String,
        #[source]
        error: MirValidationError,
    },
}

/// 一次 lowering 的产物：MIR + 对应的 `TypeStore`。
///
/// 说明：MIR 节点里的 `TypeId` 仅在同一个 `TypeStore` 里可解码/展示。
#[derive(Debug)]
pub struct LoweredMir {
    pub file: File,
    pub types: TypeStore,
}

/// 新建 basic block 时使用的默认 terminator 标记。
///
/// 说明：builder 在 block 完成后应当覆盖该 terminator；若最终仍保留该值，说明 lowering 未覆盖到
/// 某条控制流路径（对 dump/fixtures 来说仍可接受，但在后续阶段应当更严格约束）。
const UNTERMINATED: &str = "unterminated";
/// `var` 可变捕获在 MIR dump 阶段使用的内部 box 类型名（T0714）。
const CAPTURE_BOX_FQN: &str = "scoop.__CaptureBox";

/// 为 `scoop dump-mir` / mir fixtures 生成 MIR（最小实现）。
///
/// 当前阶段 pipeline：
/// 1) parse/resolve 源文件并降到 HIR（复用 `hir::lower_for_dump`）；
/// 2) 把 HIR 再降到 MIR（本文件实现），并生成显式 CFG。
pub fn lower_for_dump(session: &Session, source: &SourceFile) -> Result<LoweredMir, MirLowerError> {
    let mut lowered_hir = hir::lower_typed_for_dump(session, source)?;
    let builtins = lowered_hir.types.intern_builtins();
    let facts = MirLoweringFacts::from_lowered_hir(&lowered_hir);

    let file = lower_hir_file_for_dump_with_facts(
        builtins,
        &mut lowered_hir.types,
        &lowered_hir.file,
        &lowered_hir.member_funs,
        &facts,
    );
    Ok(LoweredMir {
        file,
        types: lowered_hir.types,
    })
}

/// 将一份已构造的 HIR 文件降低为 MIR，并显式接入 typed/shared facts。
///
/// 说明：
/// - 调用方需要确保 `hir_file` 中的 `TypeId` 与 `types` 来自同一个 `TypeStore`；
/// - `facts` 负责把 `Continuation.resume`、virtual/interface dispatch 等已确认语义
///   从 HIR/typecheck side table 收口为 MIR lowering 可直接消费的最小输入。
pub(crate) fn lower_hir_file_for_dump_with_facts(
    builtins: BuiltinTypes,
    types: &mut TypeStore,
    hir_file: &hir::File,
    member_funs: &[hir::FunDecl],
    facts: &MirLoweringFacts,
) -> File {
    let mut lowering = MirLowering::new(builtins, types, facts);
    lowering.lower_file(hir_file, member_funs)
}

/// 文件级 lowering：负责遍历顶层 item 并为每个函数构造 MIR body。
struct MirLowering<'a> {
    builtins: BuiltinTypes,
    types: &'a mut TypeStore,
    facts: &'a MirLoweringFacts,
}

impl<'a> MirLowering<'a> {
    /// 创建一个 MIR lowering 上下文（仅保存 builtin type ids）。
    fn new(builtins: BuiltinTypes, types: &'a mut TypeStore, facts: &'a MirLoweringFacts) -> Self {
        Self {
            builtins,
            types,
            facts,
        }
    }

    /// 把 HIR 文件降到 MIR 文件。
    fn lower_file(&mut self, file: &hir::File, member_funs: &[hir::FunDecl]) -> File {
        let mut items = Vec::with_capacity(file.items.len() + member_funs.len());
        for item in &file.items {
            match item {
                hir::Item::Fun(fun) => {
                    let (primary, nested) = self.lower_fun(fun);
                    items.push(Item::Fun(primary));
                    items.extend(nested.into_iter().map(Item::Fun));
                }
                hir::Item::Val(decl) => items.push(Item::Todo {
                    span: decl.span,
                    kind: "top-level val",
                }),
                hir::Item::Todo { span, kind } => items.push(Item::Todo { span: *span, kind }),
            }
        }

        // type/object body 中可 codegen 的 member fun 在 HIR 中以 side table 形式保存；
        // dump-mir / dump-ir 需要把它们也作为真正的 generic MIR root 发射出来。
        for fun in member_funs {
            let (primary, nested) = self.lower_fun(fun);
            items.push(Item::Fun(primary));
            items.extend(nested.into_iter().map(Item::Fun));
        }

        File { items }
    }

    /// 把一个函数降到 MIR。
    fn lower_fun(&mut self, fun: &hir::FunDecl) -> (FunDecl, Vec<FunDecl>) {
        FnLowering::new(
            self.builtins,
            self.types,
            self.facts,
            fun.fqn.clone(),
            fun.source_path.clone(),
        )
        .lower_fun(fun)
    }
}

/// 函数体 lowering：负责为单个函数构造 `Body`、管理 locals、并生成显式 CFG。
#[derive(Debug)]
struct FnLowering<'a> {
    builtins: BuiltinTypes,
    types: &'a mut TypeStore,
    facts: &'a MirLoweringFacts,
    owner_fqn: String,
    source_path: std::path::PathBuf,
    body: Body,
    current_bb: BasicBlockId,
    next_temp: u32,
    next_site_id: u32,
    symbol_locals: HashMap<hir::SymbolId, LocalId>,
    /// 值 local 的最小 provenance。
    ///
    /// 当前阶段主要为 call / member / unresolved callee / pattern canonicalization 保留最小来源信息；
    /// 一旦出现多路径/多来源冲突，就保守退化为 `UnknownCallable`。
    value_origins: HashMap<LocalId, ValueOrigin>,
    /// 当前函数内哪些 `SymbolId` 以 box 形式存储（用于 `var` 被 closure 捕获时的别名语义，T0714）。
    boxed_symbols: HashSet<hir::SymbolId>,
    cleanup_scopes: Vec<CleanupScope>,
    loop_stack: Vec<LoopContext>,
    nested_funs: Vec<FunDecl>,
}

/// 当前函数内的一个 loop 语境（用于 `break/continue` lowering）。
#[derive(Debug, Clone, Copy)]
struct LoopContext {
    break_target: BasicBlockId,
    continue_target: BasicBlockId,
    cleanup_depth: usize,
}

/// 当前 lowering 语境里“离开该作用域前必须执行”的 finally/cleanup 块。
#[derive(Debug, Clone)]
struct CleanupScope {
    finally: hir::Block,
}

/// 一个 local 当前可观察到的最小 provenance。
#[derive(Debug, Clone, PartialEq, Eq)]
enum ValueOrigin {
    Closure { fn_ptr: String },
    TopLevelRef { fqn: String },
    MemberAccess { member: MemberAccessMetadata },
    UnresolvedName { name: String },
    UnknownCallable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StoredContinuationRouteError {
    MissingSourceLocal,
    Ambiguous,
}

/// 一个 closure 捕获的外部局部变量在 env tuple 中的布局信息（T0711）。
#[derive(Debug, Clone)]
struct ClosureCaptureLayout {
    id: hir::SymbolId,
    name: String,
    decl_span: Span,
    ty: TypeId,
    mutable: bool,
    /// 在“创建该 closure 的函数”中，对应被捕获值的 local。
    source_local: LocalId,
}

#[derive(Debug, Clone)]
struct WhenPatternBinding {
    id: hir::SymbolId,
    span: Span,
    name: String,
    ty: TypeId,
    path: Vec<PatternBindingStep>,
}

impl<'a> FnLowering<'a> {
    /// 创建一个新的函数 lowering builder。
    fn new(
        builtins: BuiltinTypes,
        types: &'a mut TypeStore,
        facts: &'a MirLoweringFacts,
        owner_fqn: String,
        source_path: std::path::PathBuf,
    ) -> Self {
        Self {
            builtins,
            types,
            facts,
            owner_fqn,
            source_path,
            body: Body::new_empty(),
            current_bb: BasicBlockId(0),
            next_temp: 0,
            next_site_id: 0,
            symbol_locals: HashMap::new(),
            value_origins: HashMap::new(),
            boxed_symbols: HashSet::new(),
            cleanup_scopes: Vec::new(),
            loop_stack: Vec::new(),
            nested_funs: Vec::new(),
        }
    }

    /// 把一个 HIR 函数声明降到 MIR（当前阶段仅关注 body 的 CFG 形态）。
    fn lower_fun(mut self, fun: &hir::FunDecl) -> (FunDecl, Vec<FunDecl>) {
        // 1) 创建入口块。
        let entry = self.push_block(fun.span);
        self.body.start = entry;
        self.current_bb = entry;

        // 2) 参数变为 locals，并建立 SymbolId → LocalId 映射。
        let mut params = Vec::with_capacity(fun.params.len());
        for p in &fun.params {
            let local = self.push_named_local(p.span, &p.name, p.ty);
            self.symbol_locals.insert(p.id, local);
            params.push(Param {
                span: p.span,
                name: p.name.clone(),
                ty: p.ty,
                local,
            });
        }

        // 3) lower 函数体。
        let mir_body = if let Some(block) = fun.body.as_ref() {
            // 先扫描函数体：若某个 `var` 被任意深度的嵌套 closure 捕获，则该 `var` 在本函数内需要 box 存储。
            self.boxed_symbols = boxed_symbols_in_block(block);
            if fun.return_ty == self.builtins.unit {
                self.lower_block_as_stmt(block);
                self.finish_function(fun.span);
            } else {
                let body_result = self.lower_block_as_expr(block);
                if !self.current_is_terminated() {
                    self.set_terminator(
                        self.current_bb,
                        fun.span,
                        TerminatorKind::Return {
                            value: Some(Operand::Local(body_result)),
                        },
                    );
                }
            }
            Some(std::mem::replace(&mut self.body, Body::new_empty()))
        } else {
            None
        };

        let out = FunDecl {
            span: fun.span,
            fqn: fun.fqn.clone(),
            name: fun.name.clone(),
            ty: fun.ty,
            params,
            return_ty: fun.return_ty,
            body: mir_body,
        };

        (out, self.nested_funs)
    }

    /// 创建一个新的 basic block，并返回其 id。
    fn push_block(&mut self, span: Span) -> BasicBlockId {
        self.body.push_block(BasicBlock {
            is_cleanup: false,
            stmts: Vec::new(),
            terminator: Terminator {
                span,
                kind: TerminatorKind::Todo(UNTERMINATED),
                unwind: UnwindAction::NoUnwind,
            },
        })
    }

    fn push_cleanup_block(&mut self, span: Span) -> BasicBlockId {
        let bb = self.push_block(span);
        self.body.blocks[bb.as_usize()].is_cleanup = true;
        bb
    }

    fn fresh_site_id(&mut self) -> SiteId {
        let site_id = SiteId::from_raw(self.next_site_id);
        self.next_site_id = self
            .next_site_id
            .checked_add(1)
            .expect("too many MIR site ids in one body");
        site_id
    }

    /// 在当前 basic block 末尾追加一条语句。
    fn push_stmt(&mut self, span: Span, kind: StatementKind) {
        let bb = self.current_bb;
        self.body.blocks[bb.as_usize()]
            .stmts
            .push(Statement { span, kind });
    }

    /// 覆盖指定 basic block 的 terminator。
    fn set_terminator_with_unwind(
        &mut self,
        bb: BasicBlockId,
        span: Span,
        kind: TerminatorKind,
        unwind: UnwindAction,
    ) {
        self.body.blocks[bb.as_usize()].terminator = Terminator { span, kind, unwind };
    }

    /// 覆盖指定 basic block 的 terminator（默认 `NoUnwind`）。
    fn set_terminator(&mut self, bb: BasicBlockId, span: Span, kind: TerminatorKind) {
        self.set_terminator_with_unwind(bb, span, kind, UnwindAction::NoUnwind);
    }

    /// 当前 basic block 是否已经被 terminator 结束。
    fn current_is_terminated(&self) -> bool {
        let bb = self.current_bb;
        !matches!(
            self.body.blocks[bb.as_usize()].terminator.kind,
            TerminatorKind::Todo(msg) if msg == UNTERMINATED
        )
    }

    /// 当前 block 若只是被占位式 effect terminator 截断，则为后续语句分配一个新的 continuation block。
    ///
    /// 说明：
    /// - 现阶段 `TerminatorKind::Handle` / `TerminatorKind::Perform` 仍未展开成真实 CFG；
    /// - 但像 async task body 这类形状会在 `handle { ... }` 之后继续出现普通 direct call
    ///   （例如 `__task_step_ready(...)`），并且 `await` 之后的恢复路径也仍需要在 generic MIR 中保形；
    /// - 若这里直接停止，generic MIR materializer 将看不到这些后续 call-site；
    /// - 因此仅当终止原因是占位式 `Handle` / `Perform` 时，允许把后续语句接到一个新的孤立 block 中继续保形。
    fn continue_after_placeholder_effect_terminator_if_needed(&mut self, next_span: Span) -> bool {
        if self.facts.uses_refactor_typed_contracts() {
            return !self.current_is_terminated();
        }
        if !self.current_is_terminated() {
            return true;
        }
        if !matches!(
            self.body.blocks[self.current_bb.as_usize()].terminator.kind,
            TerminatorKind::Handle { .. } | TerminatorKind::Perform { .. }
        ) {
            return false;
        }
        self.current_bb = self.push_block(next_span);
        true
    }

    fn with_cleanup_scope_len<T>(&mut self, len: usize, f: impl FnOnce(&mut Self) -> T) -> T {
        let mut tail = self.cleanup_scopes.split_off(len);
        let result = f(self);
        self.cleanup_scopes.append(&mut tail);
        result
    }

    fn lower_cleanup_block_to_target(
        &mut self,
        cleanup_bb: BasicBlockId,
        cleanup: &hir::Block,
        target: BasicBlockId,
        outer_cleanup_len: usize,
    ) {
        let saved_bb = self.current_bb;
        self.current_bb = cleanup_bb;
        self.with_cleanup_scope_len(outer_cleanup_len, |this| {
            this.lower_block_as_stmt(cleanup);
        });
        if !self.current_is_terminated() {
            self.set_terminator(
                self.current_bb,
                cleanup.span,
                TerminatorKind::Goto { target },
            );
        }
        self.current_bb = saved_bb;
    }

    fn build_cleanup_route(
        &mut self,
        target: BasicBlockId,
        min_cleanup_depth: usize,
    ) -> BasicBlockId {
        let mut next_target = target;
        for scope_index in (min_cleanup_depth..self.cleanup_scopes.len()).rev() {
            let cleanup = self.cleanup_scopes[scope_index].finally.clone();
            let cleanup_bb = self.push_cleanup_block(cleanup.span);
            self.lower_cleanup_block_to_target(cleanup_bb, &cleanup, next_target, scope_index);
            next_target = cleanup_bb;
        }
        next_target
    }

    fn build_perform_unwind_action(&mut self, span: Span) -> UnwindAction {
        let Some(scope) = self.cleanup_scopes.last().cloned() else {
            return UnwindAction::Propagate;
        };

        let resume_unwind_bb = self.push_cleanup_block(span);
        self.set_terminator(resume_unwind_bb, span, TerminatorKind::ResumeUnwind);

        let cleanup_bb = self.push_cleanup_block(scope.finally.span);
        self.lower_cleanup_block_to_target(
            cleanup_bb,
            &scope.finally,
            resume_unwind_bb,
            self.cleanup_scopes.len() - 1,
        );
        UnwindAction::Cleanup { target: cleanup_bb }
    }

    /// 若函数尾部没有显式 terminator，则默认补一个 `return`（保持 body 可验证/可 dump）。
    fn finish_function(&mut self, span: Span) {
        if !self.current_is_terminated() {
            self.set_terminator(
                self.current_bb,
                span,
                TerminatorKind::Return { value: None },
            );
        }
    }

    /// 分配一个具名 local（用于参数与 `val/var` 声明）。
    fn push_named_local(&mut self, span: Span, name: &str, ty: TypeId) -> LocalId {
        self.body.push_local(LocalDecl {
            span,
            name: Some(name.to_string()),
            ty,
            source: LocalSourceKind::SourceLocal,
        })
    }

    /// 分配一个临时 local（用于表达式求值与 if/when merge）。
    fn push_temp_local(&mut self, span: Span, ty: TypeId) -> LocalId {
        let name = format!("tmp{}", self.next_temp);
        self.next_temp += 1;
        self.body.push_local(LocalDecl {
            span,
            name: Some(name),
            ty,
            source: LocalSourceKind::CompilerTemporary,
        })
    }

    /// 生成 `target = value` 赋值语句。
    fn assign(&mut self, span: Span, target: LocalId, value: Rvalue) {
        self.record_value_origin(target, &value);
        self.push_stmt(span, StatementKind::Assign { target, value });
    }

    fn is_function_value_ty(&self, ty: TypeId) -> bool {
        matches!(self.types.kind(ty), TypeKind::Ref(RefTypeKind::Function(_)))
    }

    fn value_origin_from_operand(&self, operand: &Operand) -> Option<ValueOrigin> {
        match operand {
            Operand::Local(local) => self.value_origins.get(local).cloned(),
            Operand::Const(_) => None,
        }
    }

    fn classify_value_assignment(&self, target: LocalId, value: &Rvalue) -> Option<ValueOrigin> {
        let target_ty = self.body.locals[target.as_u32() as usize].ty;
        match value {
            Rvalue::MakeClosure { fn_ptr, .. } => Some(ValueOrigin::Closure {
                fn_ptr: fn_ptr.clone(),
            }),
            Rvalue::TopLevelRef(TopLevelRef { fqn }) => {
                Some(ValueOrigin::TopLevelRef { fqn: fqn.clone() })
            }
            Rvalue::MemberAccess { member, .. } => Some(ValueOrigin::MemberAccess {
                member: member.clone(),
            }),
            Rvalue::UnresolvedName { name } => {
                Some(ValueOrigin::UnresolvedName { name: name.clone() })
            }
            Rvalue::Use(operand) => self.value_origin_from_operand(operand).or_else(|| {
                self.is_function_value_ty(target_ty)
                    .then_some(ValueOrigin::UnknownCallable)
            }),
            _ => self
                .is_function_value_ty(target_ty)
                .then_some(ValueOrigin::UnknownCallable),
        }
    }

    fn merge_value_origin(
        current: Option<ValueOrigin>,
        next: Option<ValueOrigin>,
    ) -> Option<ValueOrigin> {
        match (current, next) {
            (None, None) => None,
            (_, None) => None,
            (None, Some(origin)) => Some(origin),
            (Some(left), Some(right)) if left == right => Some(left),
            (Some(_), Some(_)) => Some(ValueOrigin::UnknownCallable),
        }
    }

    fn record_value_origin(&mut self, target: LocalId, value: &Rvalue) {
        let next = self.classify_value_assignment(target, value);
        let merged = Self::merge_value_origin(self.value_origins.get(&target).cloned(), next);
        match merged {
            Some(origin) => {
                self.value_origins.insert(target, origin);
            }
            None => {
                self.value_origins.remove(&target);
            }
        }
    }

    /// 把一个 block 作为“语句块”来 lower（顺序执行；最后表达式结果被丢弃）。
    fn lower_block_as_stmt(&mut self, block: &hir::Block) {
        for stmt in &block.stmts {
            if !self.continue_after_placeholder_effect_terminator_if_needed(stmt.span) {
                break;
            }
            self.lower_stmt(stmt);
        }
    }

    /// 把一个 block 作为“表达式块”来 lower，并返回 block 的结果 local。
    fn lower_block_as_expr(&mut self, block: &hir::Block) -> LocalId {
        let mut result: Option<LocalId> = None;
        for (idx, stmt) in block.stmts.iter().enumerate() {
            if !self.continue_after_placeholder_effect_terminator_if_needed(stmt.span) {
                break;
            }
            let is_last = idx + 1 == block.stmts.len();
            match (&stmt.kind, is_last) {
                (hir::StmtKind::Expr(expr), true) => result = Some(self.lower_expr_to_local(expr)),
                _ => self.lower_stmt(stmt),
            }
        }

        if self.current_is_terminated() {
            // block 由于 `return/break/continue` 等提前终止：结果永远不会被使用。
            // 为保持接口一致，仍返回一个临时 local，但不额外发射赋值语句（避免“终止后又生成语句”）。
            return self.push_temp_local(block.span, block.ty);
        }

        result.unwrap_or_else(|| self.emit_unit(block.span))
    }

    /// 把一条 HIR 语句降到 MIR（当前阶段只覆盖必要子集；未覆盖节点以 `Todo` 占位）。
    fn lower_stmt(&mut self, stmt: &hir::Stmt) {
        match &stmt.kind {
            hir::StmtKind::Empty => {}
            hir::StmtKind::Expr(expr) => {
                let _ = self.lower_expr_to_local(expr);
            }
            hir::StmtKind::Val(decl) => self.lower_val_decl(decl),
            hir::StmtKind::Assign { lhs, rhs, .. } => self.lower_assign_stmt(stmt.span, lhs, rhs),
            hir::StmtKind::While { cond, body } => self.lower_while_stmt(stmt.span, cond, body),
            hir::StmtKind::Break { .. } => self.lower_break_stmt(stmt.span),
            hir::StmtKind::Continue { .. } => self.lower_continue_stmt(stmt.span),
            hir::StmtKind::Return { value } => {
                let return_value = if let Some(expr) = value {
                    let result = self.lower_expr_to_local(expr);
                    if self.current_is_terminated() {
                        return;
                    }
                    Some(Operand::Local(result))
                } else {
                    None
                };

                if self.cleanup_scopes.is_empty() {
                    self.set_terminator(
                        self.current_bb,
                        stmt.span,
                        TerminatorKind::Return {
                            value: return_value,
                        },
                    );
                    return;
                }

                let return_bb = self.push_block(stmt.span);
                self.set_terminator(
                    return_bb,
                    stmt.span,
                    TerminatorKind::Return {
                        value: return_value,
                    },
                );
                let cleanup_target = self.build_cleanup_route(return_bb, 0);
                self.set_terminator(
                    self.current_bb,
                    stmt.span,
                    TerminatorKind::Goto {
                        target: cleanup_target,
                    },
                );
            }
            hir::StmtKind::Todo(kind) => self.push_stmt(stmt.span, StatementKind::Todo(kind)),
        }
    }

    /// 降低一个 `while` 语句：构造 loop CFG，并为 `break/continue` 建立跳转目标。
    fn lower_while_stmt(&mut self, span: Span, cond: &hir::Expr, body: &hir::Block) {
        // CFG 形态（无 label）：
        //
        //   parent ──goto──▶ cond_bb ──condbr──▶ body_bb ──goto──▶ cond_bb
        //                 └───────────────▶ exit_bb
        //
        // `break`    → exit_bb
        // `continue` → cond_bb

        let parent = self.current_bb;
        let cond_bb = self.push_block(cond.span);
        let body_bb = self.push_block(body.span);
        let exit_bb = self.push_block(span);

        self.set_terminator(parent, span, TerminatorKind::Goto { target: cond_bb });

        // 1) condition：在 cond_bb 中求值条件，并用 CondBr 结束。
        self.current_bb = cond_bb;
        let cond_local = self.lower_expr_to_local(cond);
        if !self.current_is_terminated() {
            self.set_terminator(
                self.current_bb,
                span,
                TerminatorKind::CondBr {
                    cond: Operand::Local(cond_local),
                    then_target: body_bb,
                    else_target: exit_bb,
                },
            );
        }

        // 2) body：在 loop context 下 lower body；若 body 自然结束则回跳 cond_bb。
        self.current_bb = body_bb;
        self.loop_stack.push(LoopContext {
            break_target: exit_bb,
            continue_target: cond_bb,
            cleanup_depth: self.cleanup_scopes.len(),
        });
        self.lower_block_as_stmt(body);
        let _ = self.loop_stack.pop();

        if !self.current_is_terminated() {
            self.set_terminator(
                self.current_bb,
                body.span,
                TerminatorKind::Goto { target: cond_bb },
            );
        }

        // 3) 后续语句继续在 exit_bb 生成。
        self.current_bb = exit_bb;
    }

    /// 降低 `break`：跳转到当前 loop 的 exit block。
    fn lower_break_stmt(&mut self, span: Span) {
        let Some(ctx) = self.loop_stack.last().copied() else {
            self.set_terminator(
                self.current_bb,
                span,
                TerminatorKind::Todo("break not in loop"),
            );
            return;
        };
        if self.cleanup_scopes.len() == ctx.cleanup_depth {
            self.set_terminator(
                self.current_bb,
                span,
                TerminatorKind::Goto {
                    target: ctx.break_target,
                },
            );
            return;
        }
        let cleanup_target = self.build_cleanup_route(ctx.break_target, ctx.cleanup_depth);
        self.set_terminator(
            self.current_bb,
            span,
            TerminatorKind::Goto {
                target: cleanup_target,
            },
        );
    }

    /// 降低 `continue`：跳转到当前 loop 的 cond block。
    fn lower_continue_stmt(&mut self, span: Span) {
        let Some(ctx) = self.loop_stack.last().copied() else {
            self.set_terminator(
                self.current_bb,
                span,
                TerminatorKind::Todo("continue not in loop"),
            );
            return;
        };
        if self.cleanup_scopes.len() == ctx.cleanup_depth {
            self.set_terminator(
                self.current_bb,
                span,
                TerminatorKind::Goto {
                    target: ctx.continue_target,
                },
            );
            return;
        }
        let cleanup_target = self.build_cleanup_route(ctx.continue_target, ctx.cleanup_depth);
        self.set_terminator(
            self.current_bb,
            span,
            TerminatorKind::Goto {
                target: cleanup_target,
            },
        );
    }

    /// 降低一个 `val/var` 声明：分配 local，并 lower initializer（若存在）。
    fn lower_val_decl(&mut self, decl: &hir::ValDecl) {
        let Some(id) = decl.id else {
            self.push_stmt(decl.span, StatementKind::Todo("val decl missing symbol id"));
            return;
        };

        let name = decl.name.as_deref().unwrap_or("<anon>");
        // `var` 若被 closure 捕获，需要在本函数内以 box 形式存储，保证后续读写别名一致（T0714）。
        if decl.mutable && self.boxed_symbols.contains(&id) {
            let box_ty = self.capture_box_ty(decl.ty);
            let local = self.push_named_local(decl.span, name, box_ty);
            self.symbol_locals.insert(id, local);

            if let Some(init) = &decl.init {
                let value = self.lower_expr_to_local(init);
                if self.current_is_terminated() {
                    return;
                }
                self.assign(
                    decl.span,
                    local,
                    Rvalue::CaptureBoxNew {
                        value: Operand::Local(value),
                    },
                );
            } else {
                self.assign(
                    decl.span,
                    local,
                    Rvalue::Todo("boxed var decl init pending"),
                );
            }
            return;
        }

        let local = self.push_named_local(decl.span, name, decl.ty);
        self.symbol_locals.insert(id, local);

        if let Some(init) = &decl.init {
            let value = self.lower_expr_to_local(init);
            if self.current_is_terminated() {
                return;
            }
            self.assign(decl.span, local, Rvalue::Use(Operand::Local(value)));
        }
    }

    /// 降低一个赋值语句。
    fn lower_assign_stmt(&mut self, span: Span, lhs: &hir::Expr, rhs: &hir::Expr) {
        match &lhs.kind {
            hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) => {
                let Some(target) = self.symbol_locals.get(id).copied() else {
                    self.push_stmt(span, StatementKind::Todo("assign lhs missing local"));
                    return;
                };

                let value = self.lower_expr_to_local(rhs);
                if self.current_is_terminated() {
                    return;
                }
                if self.boxed_symbols.contains(id) {
                    let tmp = self.push_temp_local(span, self.builtins.unit);
                    self.assign(
                        span,
                        tmp,
                        Rvalue::CaptureBoxSet {
                            box_operand: Operand::Local(target),
                            value: Operand::Local(value),
                        },
                    );
                } else {
                    self.assign(span, target, Rvalue::Use(Operand::Local(value)));
                }
            }
            hir::ExprKind::MemberAccess { receiver, member } => {
                let receiver_local = self.lower_expr_to_local(receiver);
                if self.current_is_terminated() {
                    return;
                }
                let value_local = self.lower_expr_to_local(rhs);
                if self.current_is_terminated() {
                    return;
                }
                let receiver_ty = self.body.locals[receiver_local.as_u32() as usize].ty;
                let value_ty = self.body.locals[value_local.as_u32() as usize].ty;
                self.push_stmt(
                    span,
                    StatementKind::StoreMember {
                        receiver: Operand::Local(receiver_local),
                        member: self.lower_member_access_metadata(member, receiver_ty),
                        value: Operand::Local(value_local),
                        value_ty,
                        continuation_route: self.extract_stored_continuation_route(rhs),
                    },
                );
            }
            _ => {
                self.push_stmt(span, StatementKind::Todo("assign lhs lowering pending"));
            }
        }
    }

    fn extract_stored_continuation_route(
        &self,
        expr: &hir::Expr,
    ) -> StoredContinuationRoutePublication {
        match self.try_extract_stored_continuation_route(expr) {
            Ok(Some(route)) => StoredContinuationRoutePublication::Unique(route),
            Ok(None) => StoredContinuationRoutePublication::None,
            Err(StoredContinuationRouteError::Ambiguous) => {
                StoredContinuationRoutePublication::Ambiguous
            }
            Err(StoredContinuationRouteError::MissingSourceLocal) => {
                StoredContinuationRoutePublication::None
            }
        }
    }

    fn try_extract_stored_continuation_route(
        &self,
        expr: &hir::Expr,
    ) -> Result<Option<StoredContinuationValueRoute>, StoredContinuationRouteError> {
        if continuation_contract_from_type(self.types, expr.ty).is_some() {
            let hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) = &expr.kind else {
                return Ok(None);
            };
            let Some(local) = self.symbol_locals.get(id).copied() else {
                return Err(StoredContinuationRouteError::MissingSourceLocal);
            };
            let source_ty = self.body.locals[local.as_u32() as usize].ty;
            return Ok(Some(StoredContinuationValueRoute {
                source_local: local,
                source_ty,
                path: Vec::new(),
            }));
        }

        let hir::ExprKind::Call { callee, args } = &expr.kind else {
            return Ok(None);
        };
        let hir::ExprKind::UnresolvedIdent { name } = &callee.kind else {
            return Ok(None);
        };

        let mut found: Option<(usize, StoredContinuationValueRoute)> = None;
        for (field_index, arg) in args.iter().enumerate() {
            let arg_expr = match arg {
                hir::CallArg::Positional(expr) => expr,
                hir::CallArg::Named { value, .. } => value,
            };
            let Some(mut route) = self.try_extract_stored_continuation_route(arg_expr)? else {
                continue;
            };
            if found.is_some() {
                return Err(StoredContinuationRouteError::Ambiguous);
            }
            route.path.insert(
                0,
                PatternBindingStep::VariantField {
                    variant: name.clone(),
                    field_index,
                },
            );
            found = Some((field_index, route));
        }

        Ok(found.map(|(_, route)| route))
    }

    /// 把一个 HIR 表达式降为“产生值的 local”，并返回该 local。
    ///
    /// 说明：当前阶段优先保证 CFG 形态正确，因此表达式求值本身常以 `Todo` 占位。
    fn lower_expr_to_local(&mut self, expr: &hir::Expr) -> LocalId {
        match &expr.kind {
            hir::ExprKind::Missing => {
                let tmp = self.push_temp_local(expr.span, expr.ty);
                self.assign(expr.span, tmp, Rvalue::Todo("missing expr"));
                tmp
            }
            hir::ExprKind::UnresolvedIdent { name } => {
                self.lower_unresolved_ident(expr.span, expr.ty, name)
            }
            hir::ExprKind::Todo(kind) => {
                let tmp = self.push_temp_local(expr.span, expr.ty);
                self.assign(expr.span, tmp, Rvalue::Todo(kind));
                tmp
            }
            hir::ExprKind::Literal(lit) => self.lower_literal(expr.span, expr.ty, lit),
            hir::ExprKind::VarRef(v) => self.lower_var_ref(expr.span, expr.ty, v),
            hir::ExprKind::StructLit { .. } => {
                self.emit_todo_value(expr.span, expr.ty, "struct literal lowering pending")
            }
            hir::ExprKind::TupleLit { .. } => {
                self.emit_todo_value(expr.span, expr.ty, "tuple literal lowering pending")
            }
            hir::ExprKind::InterpolatedString { .. } => {
                self.emit_todo_value(expr.span, expr.ty, "interpolated string lowering pending")
            }
            hir::ExprKind::Unary {
                op, expr: operand, ..
            } => self.lower_unary_expr(expr.span, expr.ty, *op, operand),
            hir::ExprKind::Binary { lhs, op, rhs, .. } => {
                self.lower_binary_expr(expr.span, expr.ty, lhs, *op, rhs)
            }
            hir::ExprKind::TypeCheck {
                expr: value,
                op,
                target_ty: test_ty,
                ..
            } => self.lower_type_check_expr(expr.span, expr.ty, value, *op, *test_ty),
            hir::ExprKind::Cast {
                expr: value,
                op,
                target_ty,
                ..
            } => self.lower_cast_expr(expr.span, expr.ty, value, *op, *target_ty),
            hir::ExprKind::Block(block) => self.lower_block_as_expr(block),
            hir::ExprKind::Closure(closure) => self.lower_closure_expr(expr.span, expr.ty, closure),
            hir::ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => self.lower_if_expr(
                expr.span,
                expr.ty,
                cond,
                then_branch,
                else_branch.as_deref(),
            ),
            hir::ExprKind::When { subject, arms } => {
                self.lower_when_expr(expr.span, expr.ty, subject, arms)
            }
            hir::ExprKind::MemberAccess { receiver, member } => {
                self.lower_member_access_expr(expr.span, expr.ty, receiver, member)
            }
            hir::ExprKind::Call { callee, args } => {
                self.lower_call_expr(expr.span, expr.ty, callee, args)
            }
            hir::ExprKind::Perform {
                effect_ty,
                op,
                args,
            } => self.lower_perform_expr(expr.span, expr.ty, *effect_ty, op, args),
            hir::ExprKind::Handle(handle) => self.lower_handle_expr(expr.span, expr.ty, handle),
        }
    }

    fn lower_unresolved_ident(&mut self, span: Span, ty: TypeId, name: &str) -> LocalId {
        let tmp = self.push_temp_local(span, ty);
        self.assign(
            span,
            tmp,
            Rvalue::UnresolvedName {
                name: name.to_string(),
            },
        );
        tmp
    }

    /// 生成一个 `Unit` 值，并返回其 local。
    fn emit_unit(&mut self, span: Span) -> LocalId {
        let tmp = self.push_temp_local(span, self.builtins.unit);
        self.assign(span, tmp, Rvalue::Use(Operand::Const(ConstValue::Unit)));
        tmp
    }

    /// 生成一个“未实现的值”，并返回其 local。
    fn emit_todo_value(&mut self, span: Span, ty: TypeId, msg: &'static str) -> LocalId {
        let tmp = self.push_temp_local(span, ty);
        self.assign(span, tmp, Rvalue::Todo(msg));
        tmp
    }

    fn lower_unary_expr(
        &mut self,
        span: Span,
        ty: TypeId,
        op: ast::UnaryOp,
        operand: &hir::Expr,
    ) -> LocalId {
        let result = self.push_temp_local(span, ty);
        let operand_local = self.lower_expr_to_local(operand);
        if self.current_is_terminated() {
            return result;
        }
        self.assign(
            span,
            result,
            Rvalue::Unary {
                op,
                operand: Operand::Local(operand_local),
            },
        );
        result
    }

    fn lower_binary_expr(
        &mut self,
        span: Span,
        ty: TypeId,
        lhs: &hir::Expr,
        op: ast::BinaryOp,
        rhs: &hir::Expr,
    ) -> LocalId {
        let result_ty = self.binary_result_ty(ty, op);
        match op {
            ast::BinaryOp::LogAnd | ast::BinaryOp::LogOr => {
                self.lower_short_circuit_binary_expr(span, result_ty, lhs, op, rhs)
            }
            ast::BinaryOp::Lt | ast::BinaryOp::Le | ast::BinaryOp::Gt | ast::BinaryOp::Ge => {
                if let Some(result) =
                    self.try_lower_compare_to_binary_expr(span, result_ty, lhs, op, rhs)
                {
                    return result;
                }

                let result = self.push_temp_local(span, result_ty);
                let lhs_local = self.lower_expr_to_local(lhs);
                if self.current_is_terminated() {
                    return result;
                }
                let rhs_local = self.lower_expr_to_local(rhs);
                if self.current_is_terminated() {
                    return result;
                }
                self.assign(
                    span,
                    result,
                    Rvalue::Binary {
                        lhs: Operand::Local(lhs_local),
                        op,
                        rhs: Operand::Local(rhs_local),
                    },
                );
                result
            }
            _ => {
                let result = self.push_temp_local(span, result_ty);
                let lhs_local = self.lower_expr_to_local(lhs);
                if self.current_is_terminated() {
                    return result;
                }
                let rhs_local = self.lower_expr_to_local(rhs);
                if self.current_is_terminated() {
                    return result;
                }
                self.assign(
                    span,
                    result,
                    Rvalue::Binary {
                        lhs: Operand::Local(lhs_local),
                        op,
                        rhs: Operand::Local(rhs_local),
                    },
                );
                result
            }
        }
    }

    fn binary_result_ty(&self, fallback_ty: TypeId, op: ast::BinaryOp) -> TypeId {
        match op {
            ast::BinaryOp::Lt
            | ast::BinaryOp::Le
            | ast::BinaryOp::Gt
            | ast::BinaryOp::Ge
            | ast::BinaryOp::Eq
            | ast::BinaryOp::Ne
            | ast::BinaryOp::LogAnd
            | ast::BinaryOp::LogOr => self.builtins.bool_,
            _ => fallback_ty,
        }
    }

    fn lower_short_circuit_binary_expr(
        &mut self,
        span: Span,
        ty: TypeId,
        lhs: &hir::Expr,
        op: ast::BinaryOp,
        rhs: &hir::Expr,
    ) -> LocalId {
        let result = self.push_temp_local(span, ty);
        let lhs_local = self.lower_expr_to_local(lhs);
        if self.current_is_terminated() {
            return result;
        }

        let rhs_bb = self.push_block(rhs.span);
        let short_bb = self.push_block(span);
        let merge_bb = self.push_block(span);
        let parent = self.current_bb;

        let (then_target, else_target, short_value) = match op {
            ast::BinaryOp::LogAnd => (rhs_bb, short_bb, false),
            ast::BinaryOp::LogOr => (short_bb, rhs_bb, true),
            _ => unreachable!("caller guarantees short-circuit op"),
        };

        self.set_terminator(
            parent,
            span,
            TerminatorKind::CondBr {
                cond: Operand::Local(lhs_local),
                then_target,
                else_target,
            },
        );

        self.current_bb = short_bb;
        self.assign(
            span,
            result,
            Rvalue::Use(Operand::Const(ConstValue::Bool(short_value))),
        );
        self.set_terminator(short_bb, span, TerminatorKind::Goto { target: merge_bb });

        self.current_bb = rhs_bb;
        let rhs_local = self.lower_expr_to_local(rhs);
        if !self.current_is_terminated() {
            self.assign(span, result, Rvalue::Use(Operand::Local(rhs_local)));
            self.set_terminator(rhs_bb, span, TerminatorKind::Goto { target: merge_bb });
        }

        self.current_bb = merge_bb;
        result
    }

    fn lower_type_check_expr(
        &mut self,
        span: Span,
        ty: TypeId,
        value: &hir::Expr,
        op: ast::TypeCheckOp,
        test_ty: TypeId,
    ) -> LocalId {
        let result = self.push_temp_local(span, ty);
        let value_local = self.lower_expr_to_local(value);
        if self.current_is_terminated() {
            return result;
        }
        self.assign(
            span,
            result,
            Rvalue::TypeCheck {
                value: Operand::Local(value_local),
                op,
                test_ty,
            },
        );
        result
    }

    fn lower_cast_expr(
        &mut self,
        span: Span,
        ty: TypeId,
        value: &hir::Expr,
        op: ast::CastOp,
        target_ty: TypeId,
    ) -> LocalId {
        let result = self.push_temp_local(span, ty);
        let value_local = self.lower_expr_to_local(value);
        if self.current_is_terminated() {
            return result;
        }
        self.assign(
            span,
            result,
            Rvalue::Cast {
                value: Operand::Local(value_local),
                op,
                target_ty,
            },
        );
        result
    }

    fn lower_member_access_expr(
        &mut self,
        span: Span,
        ty: TypeId,
        receiver: &hir::Expr,
        member: &hir::MemberAccess,
    ) -> LocalId {
        let result_ty = self.member_value_ty(member).unwrap_or(ty);
        let result = self.push_temp_local(span, result_ty);
        let receiver_local = self.lower_expr_to_local(receiver);
        if self.current_is_terminated() {
            return result;
        }
        let receiver_ty = self.body.locals[receiver_local.as_u32() as usize].ty;
        self.assign(
            span,
            result,
            Rvalue::MemberAccess {
                receiver: Operand::Local(receiver_local),
                member: self.lower_member_access_metadata(member, receiver_ty),
            },
        );
        result
    }

    fn member_value_ty(&self, member: &hir::MemberAccess) -> Option<TypeId> {
        let Some(hir::MemberRef::Value { fqn, .. }) = member.resolved.as_ref() else {
            return None;
        };
        self.facts.member_value_tys.get(fqn).copied()
    }

    fn lower_member_access_metadata(
        &self,
        member: &hir::MemberAccess,
        receiver_ty: TypeId,
    ) -> MemberAccessMetadata {
        let resolved = member.resolved.as_ref().map(|resolved| match resolved {
            hir::MemberRef::Value { fqn, .. } => MemberTarget::Value { fqn: fqn.clone() },
            hir::MemberRef::Fun { fqn, .. } => MemberTarget::Fun { fqn: fqn.clone() },
            hir::MemberRef::ExtensionValue { fqn, .. } => {
                MemberTarget::ExtensionValue { fqn: fqn.clone() }
            }
            hir::MemberRef::ExtensionFun { fqn, .. } => {
                MemberTarget::ExtensionFun { fqn: fqn.clone() }
            }
        });
        MemberAccessMetadata {
            name: member.name.clone(),
            receiver_ty,
            resolved,
        }
    }

    fn lower_call_args(&mut self, args: &[hir::CallArg]) -> Option<Vec<CallArg>> {
        let mut out = Vec::with_capacity(args.len());
        for arg in args {
            if self.current_is_terminated() {
                return None;
            }
            match arg {
                hir::CallArg::Positional(expr) => {
                    let value = self.lower_expr_to_local(expr);
                    if self.current_is_terminated() {
                        return None;
                    }
                    out.push(CallArg {
                        span: expr.span,
                        name: None,
                        value: Operand::Local(value),
                    });
                }
                hir::CallArg::Named { name, value, .. } => {
                    let operand_local = self.lower_expr_to_local(value);
                    if self.current_is_terminated() {
                        return None;
                    }
                    out.push(CallArg {
                        span: value.span,
                        name: Some(name.clone()),
                        value: Operand::Local(operand_local),
                    });
                }
            }
        }
        Some(out)
    }

    fn operand_ty(&self, operand: &Operand) -> TypeId {
        match operand {
            Operand::Local(local) => self.body.locals[local.as_u32() as usize].ty,
            Operand::Const(ConstValue::Bool(_)) => self.builtins.bool_,
            Operand::Const(ConstValue::Char) => self.builtins.char_,
            Operand::Const(ConstValue::Unit) => self.builtins.unit,
            Operand::Const(ConstValue::Int) => self.builtins.int,
            Operand::Const(ConstValue::SynthInt(_)) => self.builtins.int,
            Operand::Const(ConstValue::Float64) => self.builtins.float64,
            Operand::Const(ConstValue::Float32) => self.builtins.float32,
            Operand::Const(ConstValue::String) => self.builtins.string,
        }
    }

    fn canonicalize_perform_args(
        &mut self,
        span: Span,
        lowered_args: Vec<CallArg>,
    ) -> Option<(Vec<PerformArg>, PerformMetadata)> {
        if let Some(metadata) = self
            .facts
            .refactor_perform_metadata(self.source_path.as_path(), span)
            .filter(|metadata| {
                metadata
                    .arg_mapping
                    .iter()
                    .all(|idx| *idx < lowered_args.len())
            })
            .cloned()
        {
            let perform_args = metadata
                .arg_mapping
                .iter()
                .copied()
                .filter_map(|arg_idx| lowered_args.get(arg_idx).map(|arg| (arg_idx, arg)))
                .map(|(source_arg_index, arg)| PerformArg {
                    span: arg.span,
                    source_arg_index,
                    name: arg.name.clone(),
                    value: arg.value.clone(),
                })
                .collect::<Vec<_>>();
            return Some((perform_args, metadata));
        }

        if self.facts.uses_refactor_typed_contracts() {
            return None;
        }

        let info = self.facts.legacy_perform_site_info(span);
        let arg_mapping = info
            .map(|site| site.arg_mapping.as_slice())
            .filter(|mapping| mapping.iter().all(|idx| *idx < lowered_args.len()))
            .map(|mapping| mapping.to_vec())
            .unwrap_or_else(|| (0..lowered_args.len()).collect());

        let perform_args = arg_mapping
            .iter()
            .copied()
            .filter_map(|arg_idx| lowered_args.get(arg_idx).map(|arg| (arg_idx, arg)))
            .map(|(source_arg_index, arg)| PerformArg {
                span: arg.span,
                source_arg_index,
                name: arg.name.clone(),
                value: arg.value.clone(),
            })
            .collect::<Vec<_>>();

        let payload_tuple_ty = info.and_then(|site| site.payload_tuple_ty).or_else(|| {
            (perform_args.len() > 1).then(|| {
                self.types.ty_tuple(
                    perform_args
                        .iter()
                        .map(|arg| self.operand_ty(&arg.value))
                        .collect(),
                )
            })
        });

        let payload_component_tys = perform_args
            .iter()
            .map(|arg| self.operand_ty(&arg.value))
            .collect();

        Some((
            perform_args,
            PerformMetadata {
                effect_ty: self.builtins.any,
                payload_tuple_ty,
                payload_component_tys,
                arg_mapping,
            },
        ))
    }

    fn lower_call_expr(
        &mut self,
        span: Span,
        ty: TypeId,
        callee: &hir::Expr,
        args: &[hir::CallArg],
    ) -> LocalId {
        let result = self.push_temp_local(span, ty);

        if let Some(resume_metadata) = self
            .facts
            .refactor_resume_metadata(self.source_path.as_path(), span)
            .cloned()
        {
            self.lower_resume_call_expr(span, result, callee, args, Some(resume_metadata));
            return result;
        }

        if !self.facts.uses_refactor_typed_contracts()
            && self.facts.legacy_resume_site_matches(span)
        {
            self.lower_resume_call_expr(span, result, callee, args, None);
            return result;
        }

        if self.lower_dispatch_call_expr(span, result, callee, args) {
            return result;
        }

        let callee_local = self.lower_expr_to_local(callee);
        if self.current_is_terminated() {
            return result;
        }
        let callee_ty = self.body.locals[callee_local.as_u32() as usize].ty;
        let callee_origin = self.value_origins.get(&callee_local).cloned();
        let callee_can_lower = self.is_function_value_ty(callee_ty)
            || matches!(
                callee_origin,
                Some(
                    ValueOrigin::Closure { .. }
                        | ValueOrigin::TopLevelRef { .. }
                        | ValueOrigin::MemberAccess { .. }
                        | ValueOrigin::UnknownCallable
                        | ValueOrigin::UnresolvedName { .. }
                )
            );
        if !callee_can_lower {
            self.assign(span, result, Rvalue::Todo("call callee lowering pending"));
            return result;
        }

        let Some(args) = self.lower_call_args(args) else {
            return result;
        };

        let kind = match callee_origin.as_ref() {
            Some(ValueOrigin::TopLevelRef { fqn }) => CallKind::Direct {
                callee_fqn: fqn.clone(),
            },
            Some(ValueOrigin::Closure { fn_ptr }) => CallKind::Closure {
                callee: Operand::Local(callee_local),
                fn_ptr: fn_ptr.clone(),
            },
            Some(ValueOrigin::UnresolvedName { .. }) => {
                self.assign(span, result, Rvalue::Todo("ctor call lowering pending"));
                return result;
            }
            Some(ValueOrigin::MemberAccess { .. }) | Some(ValueOrigin::UnknownCallable) | None => {
                CallKind::FunValue {
                    callee: Operand::Local(callee_local),
                }
            }
        };

        let site_id = self.fresh_site_id();
        self.assign(
            span,
            result,
            Rvalue::Call {
                site_id,
                kind,
                args,
            },
        );
        result
    }

    fn lower_resume_call_expr(
        &mut self,
        span: Span,
        result: LocalId,
        callee: &hir::Expr,
        args: &[hir::CallArg],
        resume_metadata: Option<ResumeMetadata>,
    ) {
        let (receiver, payload_args) = match &callee.kind {
            hir::ExprKind::MemberAccess { receiver, .. } => (receiver.as_ref(), args),
            hir::ExprKind::VarRef(hir::ValueRef::TopLevel { .. }) => {
                let Some((hir::CallArg::Positional(receiver), payload_args)) = args.split_first()
                else {
                    self.assign(
                        span,
                        result,
                        Rvalue::Todo("resume lowering requires canonical callee shape"),
                    );
                    return;
                };
                (receiver, payload_args)
            }
            _ => {
                self.assign(
                    span,
                    result,
                    Rvalue::Todo("resume lowering requires canonical callee shape"),
                );
                return;
            }
        };

        let continuation_local = self.lower_expr_to_local(receiver);
        if self.current_is_terminated() {
            return;
        }

        let Some(args) = self.lower_call_args(payload_args) else {
            return;
        };
        let continuation_ty = self.body.locals[continuation_local.as_u32() as usize].ty;
        let resume = resume_metadata.unwrap_or_else(|| {
            let (resume_ty, answer_ty, out_effects) = continuation_contract_from_type(
                self.types,
                continuation_ty,
            )
            .unwrap_or((self.builtins.any, self.builtins.any, EffectRow::pure()));
            ResumeMetadata {
                continuation_ty,
                resume_ty,
                answer_ty,
                return_ty: self.body.locals[result.as_u32() as usize].ty,
                out_effects: out_effects.clone(),
                runtime_error_effect_ty: find_raise_runtime_error_effect(self.types),
                suspends_outward: !out_effects.is_pure()
                    || self.facts.legacy_resume_site_suspends_outward(span),
            }
        });
        let site_id = self.fresh_site_id();
        self.assign(
            span,
            result,
            Rvalue::Call {
                site_id,
                kind: CallKind::Resume {
                    continuation: Operand::Local(continuation_local),
                    resume,
                },
                args,
            },
        );
    }

    fn lower_dispatch_call_expr(
        &mut self,
        span: Span,
        result: LocalId,
        callee: &hir::Expr,
        args: &[hir::CallArg],
    ) -> bool {
        let dispatch_target = match &callee.kind {
            hir::ExprKind::VarRef(hir::ValueRef::TopLevel { fqn, .. }) => {
                let Some((receiver_arg, remaining_args)) = args.split_first() else {
                    return false;
                };
                let receiver_expr = match receiver_arg {
                    hir::CallArg::Positional(expr) => expr,
                    hir::CallArg::Named { value, .. } => value,
                };
                let Some(kind) = self.facts.dispatch_target_kind(
                    self.source_path.as_path(),
                    span,
                    receiver_expr.ty,
                ) else {
                    return false;
                };
                (kind, fqn.as_str(), receiver_expr, remaining_args)
            }
            hir::ExprKind::MemberAccess { receiver, member } => {
                let Some(hir::MemberRef::Fun { fqn, .. }) = member.resolved.as_ref() else {
                    return false;
                };
                let Some(kind) =
                    self.facts
                        .dispatch_target_kind(self.source_path.as_path(), span, receiver.ty)
                else {
                    return false;
                };
                (kind, fqn.as_str(), receiver.as_ref(), args)
            }
            _ => return false,
        };

        let (dispatch_kind, callee_fqn, receiver_expr, call_args) = dispatch_target;
        let receiver_local = self.lower_expr_to_local(receiver_expr);
        if self.current_is_terminated() {
            return true;
        }
        let Some(args) = self.lower_call_args(call_args) else {
            return true;
        };
        let receiver_ty = self.body.locals[receiver_local.as_u32() as usize].ty;
        let Some((owner_fqn, member_name)) = callee_fqn.rsplit_once('.') else {
            self.assign(
                span,
                result,
                Rvalue::Todo("dispatch callee lowering pending"),
            );
            return true;
        };
        let dispatch = DispatchMetadata {
            owner_fqn: owner_fqn.to_string(),
            member_name: member_name.to_string(),
            receiver_ty,
        };
        let kind = match dispatch_kind {
            DispatchTargetKind::Virtual => CallKind::Virtual {
                receiver: Operand::Local(receiver_local),
                dispatch,
            },
            DispatchTargetKind::Interface => CallKind::Interface {
                receiver: Operand::Local(receiver_local),
                dispatch,
            },
        };
        let site_id = self.fresh_site_id();
        self.assign(
            span,
            result,
            Rvalue::Call {
                site_id,
                kind,
                args,
            },
        );
        true
    }

    fn capture_box_ty(&mut self, inner: TypeId) -> TypeId {
        self.types
            .intern(TypeKind::Ref(RefTypeKind::Nominal(NominalType {
                fqn: CAPTURE_BOX_FQN.to_string(),
                args: vec![inner],
                eff: None,
            })))
    }

    /// 降低一个 effect operation 调用（HIR `Perform`）到 MIR。
    ///
    /// 当前阶段会把 `perform` 同时显式化为：
    /// - 普通恢复后的 continuation block（`resume_target`）；
    /// - 若 outward propagation 需要先跑 cleanup，则通过 `UnwindAction::Cleanup` 连到 cleanup block；
    /// - 若当前无本地 cleanup，则用 `UnwindAction::Propagate` 明确表示“直接继续向外 unwind”。
    fn lower_perform_expr(
        &mut self,
        span: Span,
        ty: TypeId,
        effect_ty: TypeId,
        op: &hir::EffectOpRef,
        args: &[hir::CallArg],
    ) -> LocalId {
        let Some(lowered_args) = self.lower_call_args(args) else {
            return self.push_temp_local(span, ty);
        };

        if self.current_is_terminated() {
            // 实参 lowering 提前终止了 CFG：该 perform 永远不会发生。
            return self.push_temp_local(span, ty);
        }

        let Some((perform_args, mut metadata)) = self.canonicalize_perform_args(span, lowered_args)
        else {
            let result = self.push_temp_local(span, ty);
            self.assign(
                span,
                result,
                Rvalue::Todo("refactor perform contract missing"),
            );
            self.set_terminator(
                self.current_bb,
                span,
                TerminatorKind::Todo("refactor perform contract missing"),
            );
            return result;
        };
        metadata.effect_ty = effect_ty;

        let result = self.push_temp_local(span, ty);
        self.assign(
            span,
            result,
            Rvalue::PerformResult {
                op_fqn: op.fqn.clone(),
                effect_ty,
            },
        );

        let resume_target = self.push_block(span);
        let site_id = self.fresh_site_id();
        let unwind = self.build_perform_unwind_action(span);
        self.set_terminator_with_unwind(
            self.current_bb,
            span,
            TerminatorKind::Perform {
                site_id,
                op_fqn: op.fqn.clone(),
                metadata,
                args: perform_args,
                resume_target,
            },
            unwind,
        );
        self.current_bb = resume_target;

        result
    }

    /// 降低一个 effect handler 表达式（HIR `Handle`）到 MIR。
    ///
    /// 当前阶段会把 `handle` 显式展开为 direct-style CFG：
    /// - 入口 block 以 `TerminatorKind::Handle` 指向 body/arms/finally/exit；
    /// - body 与 arm 正常完成后显式写回结果并跳向 `finally`/`exit_target`；
    /// - `finally` 自身作为 cleanup block 存在，`return` / `break` / `continue` 通过 cleanup chain
    ///   穿过它，而不是把这些续点留成 `Todo(...)`。
    fn lower_handle_expr(&mut self, span: Span, ty: TypeId, handle: &hir::HandleExpr) -> LocalId {
        let outer_bb = self.current_bb;

        let result = self.push_temp_local(span, ty);
        let refactor_handle_site = self
            .facts
            .refactor_handle_site_info(self.source_path.as_path(), span)
            .cloned();
        let handle_contract = if let Some(site) = refactor_handle_site {
            Some((site.metadata, site.arms))
        } else if self.facts.uses_refactor_typed_contracts() {
            None
        } else {
            Some(self.lower_handle_contract_from_hir(ty, handle))
        };
        let Some((metadata, mut arms)) = handle_contract else {
            self.assign(
                span,
                result,
                Rvalue::Todo("refactor handle contract missing"),
            );
            self.set_terminator(
                outer_bb,
                span,
                TerminatorKind::Todo("refactor handle contract missing"),
            );
            return result;
        };
        for (hir_arm, lowered_arm) in handle.arms.iter().zip(arms.iter_mut()) {
            self.allocate_handle_arm_locals(hir_arm, lowered_arm);
        }

        let body_bb = self.push_block(handle.body.span);
        let arm_bbs = handle
            .arms
            .iter()
            .map(|arm| self.push_block(arm.span))
            .collect::<Vec<_>>();
        let finally_bb = handle
            .finally
            .as_ref()
            .map(|finally| self.push_cleanup_block(finally.span));
        let exit_bb = self.push_block(span);

        let site_id = self.fresh_site_id();
        self.set_terminator(
            outer_bb,
            span,
            TerminatorKind::Handle {
                site_id,
                metadata,
                arms: arms.clone(),
                has_finally: handle.finally.is_some(),
                body_target: body_bb,
                arm_targets: arm_bbs.clone(),
                finally_target: finally_bb,
                exit_target: exit_bb,
            },
        );

        let handle_cleanup_scope = handle
            .finally
            .as_ref()
            .cloned()
            .map(|finally| CleanupScope { finally });

        self.current_bb = body_bb;
        if let Some(scope) = handle_cleanup_scope.clone() {
            self.cleanup_scopes.push(scope);
        }
        let body_value = self.lower_block_as_expr(&handle.body);
        if handle_cleanup_scope.is_some() {
            let _ = self.cleanup_scopes.pop();
        }
        if !self.current_is_terminated() {
            self.assign(
                handle.body.span,
                result,
                Rvalue::Use(Operand::Local(body_value)),
            );
            self.set_terminator(
                self.current_bb,
                handle.body.span,
                TerminatorKind::Goto {
                    target: finally_bb.unwrap_or(exit_bb),
                },
            );
        }

        for ((arm, lowered_arm), arm_bb) in handle.arms.iter().zip(arms.iter()).zip(arm_bbs) {
            self.current_bb = arm_bb;
            if let Some(scope) = handle_cleanup_scope.clone() {
                self.cleanup_scopes.push(scope);
            }
            let shadowed = self.bind_handle_arm_symbols(arm, lowered_arm);
            let arm_value = self.lower_expr_to_local(&arm.body);
            if handle_cleanup_scope.is_some() {
                let _ = self.cleanup_scopes.pop();
            }
            self.restore_shadowed_symbols(shadowed);
            if !self.current_is_terminated() {
                self.assign(arm.span, result, Rvalue::Use(Operand::Local(arm_value)));
                self.set_terminator(
                    self.current_bb,
                    arm.span,
                    TerminatorKind::Goto {
                        target: finally_bb.unwrap_or(exit_bb),
                    },
                );
            }
        }

        if let Some((finally, finally_bb)) = handle.finally.as_ref().zip(finally_bb) {
            self.lower_cleanup_block_to_target(
                finally_bb,
                finally,
                exit_bb,
                self.cleanup_scopes.len(),
            );
        }

        self.current_bb = exit_bb;

        result
    }

    fn lower_handle_contract_from_hir(
        &mut self,
        result_ty: TypeId,
        handle: &hir::HandleExpr,
    ) -> (HandleMetadata, Vec<HandlerArm>) {
        let arms = handle
            .arms
            .iter()
            .map(|arm| {
                let payload_component_tys = arm
                    .op
                    .binders
                    .iter()
                    .map(|binder| binder.ty)
                    .collect::<Vec<_>>();
                let payload_tuple_ty = payload_tuple_ty_from_components(
                    self.types,
                    self.builtins.unit,
                    &payload_component_tys,
                );
                HandlerArm {
                    op_fqn: arm.op.op.fqn.clone(),
                    binder_count: arm.op.binders.len(),
                    handled_effect_ty: arm.op.effect_ty,
                    payload_tuple_ty,
                    binder_locals: Vec::new(),
                    continuation_local: None,
                    payload_component_tys,
                    body_ty: arm.body.ty,
                    kind: match arm.kind {
                        hir::HandleArmKind::NonResuming => HandlerArmKind::NonResuming,
                        hir::HandleArmKind::EscapeContinuation { .. } => {
                            HandlerArmKind::EscapeContinuation
                        }
                    },
                }
            })
            .collect();
        (
            HandleMetadata {
                result_ty,
                body_result_ty: handle.body.ty,
                finally_result_ty: handle.finally.as_ref().map(|finally| finally.ty),
            },
            arms,
        )
    }

    fn allocate_handle_arm_locals(&mut self, arm: &hir::HandleArm, lowered_arm: &mut HandlerArm) {
        lowered_arm.binder_locals = arm
            .op
            .binders
            .iter()
            .map(|binder| self.push_named_local(binder.span, &binder.name, binder.ty))
            .collect();
        lowered_arm.binder_count = lowered_arm.binder_locals.len();
        lowered_arm.continuation_local = match arm.kind {
            hir::HandleArmKind::EscapeContinuation { continuation } => {
                let ty = self
                    .infer_local_symbol_ty_in_expr(&arm.body, continuation)
                    .unwrap_or(self.builtins.any);
                Some(self.push_named_local(arm.span, "$continuation", ty))
            }
            hir::HandleArmKind::NonResuming => None,
        };
    }

    fn bind_handle_arm_symbols(
        &mut self,
        arm: &hir::HandleArm,
        lowered_arm: &HandlerArm,
    ) -> Vec<(hir::SymbolId, Option<LocalId>)> {
        let mut shadowed = Vec::with_capacity(
            lowered_arm.binder_locals.len() + usize::from(lowered_arm.continuation_local.is_some()),
        );
        for (binder, local) in arm
            .op
            .binders
            .iter()
            .zip(lowered_arm.binder_locals.iter().copied())
        {
            let previous = self.symbol_locals.insert(binder.id, local);
            shadowed.push((binder.id, previous));
        }
        if let hir::HandleArmKind::EscapeContinuation { continuation } = arm.kind
            && let Some(local) = lowered_arm.continuation_local
        {
            let previous = self.symbol_locals.insert(continuation, local);
            shadowed.push((continuation, previous));
        }
        shadowed
    }

    fn infer_local_symbol_ty_in_expr(
        &self,
        expr: &hir::Expr,
        symbol: hir::SymbolId,
    ) -> Option<TypeId> {
        match &expr.kind {
            hir::ExprKind::VarRef(hir::ValueRef::Local { id, .. }) if *id == symbol => {
                Some(expr.ty)
            }
            hir::ExprKind::Missing
            | hir::ExprKind::Literal(_)
            | hir::ExprKind::VarRef(_)
            | hir::ExprKind::UnresolvedIdent { .. }
            | hir::ExprKind::Todo(_) => None,
            hir::ExprKind::StructLit { fields, .. } => fields
                .iter()
                .find_map(|field| self.infer_local_symbol_ty_in_expr(&field.value, symbol)),
            hir::ExprKind::TupleLit { elements } => elements
                .iter()
                .find_map(|element| self.infer_local_symbol_ty_in_expr(element, symbol)),
            hir::ExprKind::InterpolatedString { parts, .. } => parts.iter().find_map(|part| {
                let hir::InterpolatedStringPart::Expr { expr } = part else {
                    return None;
                };
                self.infer_local_symbol_ty_in_expr(expr, symbol)
            }),
            hir::ExprKind::Unary { expr, .. }
            | hir::ExprKind::TypeCheck { expr, .. }
            | hir::ExprKind::Cast { expr, .. }
            | hir::ExprKind::MemberAccess { receiver: expr, .. } => {
                self.infer_local_symbol_ty_in_expr(expr, symbol)
            }
            hir::ExprKind::Binary { lhs, rhs, .. } => self
                .infer_local_symbol_ty_in_expr(lhs, symbol)
                .or_else(|| self.infer_local_symbol_ty_in_expr(rhs, symbol)),
            hir::ExprKind::Block(block) => block
                .stmts
                .iter()
                .find_map(|stmt| self.infer_local_symbol_ty_in_stmt(stmt, symbol)),
            hir::ExprKind::Call { callee, args } => self
                .infer_local_symbol_ty_in_expr(callee, symbol)
                .or_else(|| {
                    args.iter().find_map(|arg| match arg {
                        hir::CallArg::Positional(expr) => {
                            self.infer_local_symbol_ty_in_expr(expr, symbol)
                        }
                        hir::CallArg::Named { value, .. } => {
                            self.infer_local_symbol_ty_in_expr(value, symbol)
                        }
                    })
                }),
            hir::ExprKind::Closure(closure) => {
                self.infer_local_symbol_ty_in_expr(&closure.body, symbol)
            }
            hir::ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => self
                .infer_local_symbol_ty_in_expr(cond, symbol)
                .or_else(|| self.infer_local_symbol_ty_in_expr(then_branch, symbol))
                .or_else(|| {
                    else_branch
                        .as_ref()
                        .and_then(|expr| self.infer_local_symbol_ty_in_expr(expr, symbol))
                }),
            hir::ExprKind::When { subject, arms } => self
                .infer_local_symbol_ty_in_expr(subject, symbol)
                .or_else(|| {
                    arms.iter().find_map(|arm| {
                        arm.guard
                            .as_ref()
                            .and_then(|guard| self.infer_local_symbol_ty_in_expr(guard, symbol))
                            .or_else(|| self.infer_local_symbol_ty_in_expr(&arm.body, symbol))
                    })
                }),
            hir::ExprKind::Perform { args, .. } => args.iter().find_map(|arg| match arg {
                hir::CallArg::Positional(expr) => self.infer_local_symbol_ty_in_expr(expr, symbol),
                hir::CallArg::Named { value, .. } => {
                    self.infer_local_symbol_ty_in_expr(value, symbol)
                }
            }),
            hir::ExprKind::Handle(handle) => self
                .infer_local_symbol_ty_in_block(&handle.body, symbol)
                .or_else(|| {
                    handle
                        .arms
                        .iter()
                        .find_map(|arm| self.infer_local_symbol_ty_in_expr(&arm.body, symbol))
                })
                .or_else(|| {
                    handle
                        .finally
                        .as_ref()
                        .and_then(|block| self.infer_local_symbol_ty_in_block(block, symbol))
                }),
        }
    }

    fn infer_local_symbol_ty_in_block(
        &self,
        block: &hir::Block,
        symbol: hir::SymbolId,
    ) -> Option<TypeId> {
        block
            .stmts
            .iter()
            .find_map(|stmt| self.infer_local_symbol_ty_in_stmt(stmt, symbol))
    }

    fn infer_local_symbol_ty_in_stmt(
        &self,
        stmt: &hir::Stmt,
        symbol: hir::SymbolId,
    ) -> Option<TypeId> {
        match &stmt.kind {
            hir::StmtKind::Empty
            | hir::StmtKind::Break { .. }
            | hir::StmtKind::Continue { .. }
            | hir::StmtKind::Todo(_) => None,
            hir::StmtKind::Expr(expr) => self.infer_local_symbol_ty_in_expr(expr, symbol),
            hir::StmtKind::Val(decl) => decl
                .init
                .as_ref()
                .and_then(|expr| self.infer_local_symbol_ty_in_expr(expr, symbol)),
            hir::StmtKind::Assign { lhs, rhs, .. } => self
                .infer_local_symbol_ty_in_expr(lhs, symbol)
                .or_else(|| self.infer_local_symbol_ty_in_expr(rhs, symbol)),
            hir::StmtKind::While { cond, body } => self
                .infer_local_symbol_ty_in_expr(cond, symbol)
                .or_else(|| self.infer_local_symbol_ty_in_block(body, symbol)),
            hir::StmtKind::Return { value } => value
                .as_ref()
                .and_then(|expr| self.infer_local_symbol_ty_in_expr(expr, symbol)),
        }
    }

    /// 降低字面量：把常量写入一个临时 local。
    fn lower_literal(&mut self, span: Span, ty: TypeId, lit: &hir::LiteralKind) -> LocalId {
        let tmp = self.push_temp_local(span, ty);
        let c = match lit {
            hir::LiteralKind::Bool(b) => ConstValue::Bool(*b),
            hir::LiteralKind::Char(_) => ConstValue::Char,
            hir::LiteralKind::Unit => ConstValue::Unit,
            hir::LiteralKind::Int => ConstValue::Int,
            hir::LiteralKind::SynthInt(value) => ConstValue::SynthInt(*value),
            hir::LiteralKind::Float64(_) => ConstValue::Float64,
            hir::LiteralKind::Float32(_) => ConstValue::Float32,
            hir::LiteralKind::String => ConstValue::String,
        };
        self.assign(span, tmp, Rvalue::Use(Operand::Const(c)));
        tmp
    }

    fn try_lower_compare_to_binary_expr(
        &mut self,
        span: Span,
        result_ty: TypeId,
        lhs: &hir::Expr,
        op: ast::BinaryOp,
        rhs: &hir::Expr,
    ) -> Option<LocalId> {
        let binding = self
            .facts
            .top_level_fun_call_binding(self.source_path.as_path(), span)?;
        let result = self.push_temp_local(span, result_ty);
        let lhs_local = self.lower_expr_to_local(lhs);
        if self.current_is_terminated() {
            return Some(result);
        }
        let rhs_local = self.lower_expr_to_local(rhs);
        if self.current_is_terminated() {
            return Some(result);
        }

        let compare_result = self.push_temp_local(span, self.builtins.int);
        let site_id = self.fresh_site_id();
        self.assign(
            span,
            compare_result,
            Rvalue::Call {
                site_id,
                kind: CallKind::Direct {
                    callee_fqn: binding.fqn.clone(),
                },
                args: vec![
                    CallArg {
                        span: lhs.span,
                        name: None,
                        value: Operand::Local(lhs_local),
                    },
                    CallArg {
                        span: rhs.span,
                        name: None,
                        value: Operand::Local(rhs_local),
                    },
                ],
            },
        );

        let zero = self.push_temp_local(span, self.builtins.int);
        self.assign(
            span,
            zero,
            Rvalue::Use(Operand::Const(ConstValue::SynthInt(0))),
        );
        self.assign(
            span,
            result,
            Rvalue::Binary {
                lhs: Operand::Local(compare_result),
                op,
                rhs: Operand::Local(zero),
            },
        );
        Some(result)
    }

    /// 降低变量引用：
    /// - 普通 local：直接返回其 local；
    /// - 被 capture 的 `var`（box 存储）：生成 `CaptureBoxGet` 并返回读取到的临时值 local；
    /// - 其它引用：降为 `Todo`。
    fn lower_var_ref(&mut self, span: Span, ty: TypeId, v: &hir::ValueRef) -> LocalId {
        match v {
            hir::ValueRef::Local { id, .. } => {
                let Some(local) = self.symbol_locals.get(id).copied() else {
                    return self.emit_todo_value(span, ty, "unbound local ref");
                };

                if self.boxed_symbols.contains(id) {
                    let tmp = self.push_temp_local(span, ty);
                    self.assign(
                        span,
                        tmp,
                        Rvalue::CaptureBoxGet {
                            box_operand: Operand::Local(local),
                        },
                    );
                    tmp
                } else {
                    local
                }
            }
            hir::ValueRef::TopLevel { .. } => {
                let hir::ValueRef::TopLevel { fqn, .. } = v else {
                    unreachable!("matched above");
                };
                let tmp = self.push_temp_local(span, ty);
                self.assign(
                    span,
                    tmp,
                    Rvalue::TopLevelRef(TopLevelRef { fqn: fqn.clone() }),
                );
                tmp
            }
        }
    }

    fn lower_closure_expr(
        &mut self,
        span: Span,
        ty: TypeId,
        closure: &hir::ClosureExpr,
    ) -> LocalId {
        let name = format!("$lambda{}", closure.id.as_u32());
        let fqn = format!("{}.{}", self.owner_fqn, name);

        // 1) 计算 capture set，并决定 env 的 tuple 类型。
        let mut captures: Vec<ClosureCaptureLayout> = Vec::new();
        for cap in &closure.captures {
            let Some(source_local) = self.symbol_locals.get(&cap.id).copied() else {
                // 防御性：若当前函数未为该 symbol 分配 local（理论上不应发生），跳过该 capture。
                continue;
            };
            let source_ty = self.body.locals[source_local.as_u32() as usize].ty;
            captures.push(ClosureCaptureLayout {
                id: cap.id,
                name: cap.name.clone(),
                decl_span: cap.decl_span,
                ty: source_ty,
                mutable: cap.mutable,
                source_local,
            });
        }

        let (env_ty, env_operand) = if captures.is_empty() {
            (self.builtins.unit, Operand::Const(ConstValue::Unit))
        } else {
            let env_ty = self.types.ty_tuple(captures.iter().map(|c| c.ty).collect());
            let env_local = self.push_temp_local(span, env_ty);
            self.assign(
                span,
                env_local,
                Rvalue::MakeTuple {
                    elements: captures
                        .iter()
                        .map(|c| Operand::Local(c.source_local))
                        .collect(),
                },
            );
            (env_ty, Operand::Local(env_local))
        };

        let (fun, nested) = {
            let types = &mut *self.types;
            FnLowering::new(
                self.builtins,
                types,
                self.facts,
                fqn.clone(),
                self.source_path.clone(),
            )
            .lower_closure_fun(fqn.clone(), name, closure, env_ty, &captures)
        };
        self.nested_funs.push(fun);
        self.nested_funs.extend(nested);

        let tmp = self.push_temp_local(span, ty);
        self.assign(
            span,
            tmp,
            Rvalue::MakeClosure {
                env: env_operand,
                fn_ptr: fqn,
            },
        );
        tmp
    }

    fn lower_closure_fun(
        mut self,
        closure_fqn: String,
        closure_name: String,
        closure: &hir::ClosureExpr,
        env_ty: TypeId,
        captures: &[ClosureCaptureLayout],
    ) -> (FunDecl, Vec<FunDecl>) {
        // 0) 预扫描 closure body：本 closure 内部若存在嵌套 closure 捕获 `var`，则需要 box 存储（T0714）。
        self.boxed_symbols = boxed_symbols_in_expr(closure.body.as_ref());

        // 1) 创建入口块。
        let entry = self.push_block(closure.span);
        self.body.start = entry;
        self.current_bb = entry;

        // 2) env + captures + 参数变为 locals。
        let mut params = Vec::with_capacity(closure.params.len() + 1);

        let env_local = self.push_named_local(closure.span, "$env", env_ty);
        params.push(Param {
            span: closure.span,
            name: "$env".to_string(),
            ty: env_ty,
            local: env_local,
        });

        // 把捕获字段从 `$env` 解包到局部 local，并写入 SymbolId → LocalId 映射，使得后续 body lowering
        // 可以像普通局部变量一样引用它们。
        for (idx, cap) in captures.iter().enumerate() {
            let local = self.push_named_local(cap.decl_span, &cap.name, cap.ty);
            self.symbol_locals.insert(cap.id, local);
            if cap.mutable {
                self.boxed_symbols.insert(cap.id);
            }
            self.assign(
                cap.decl_span,
                local,
                Rvalue::TupleGet {
                    tuple: Operand::Local(env_local),
                    index: idx,
                },
            );
        }

        for p in &closure.params {
            let local = self.push_named_local(p.span, &p.name, p.ty);
            self.symbol_locals.insert(p.id, local);
            params.push(Param {
                span: p.span,
                name: p.name.clone(),
                ty: p.ty,
                local,
            });
        }

        // 3) lower lambda body. A closure body is an expression, so its value is the callable
        // result unless the body already terminated through an explicit control-flow edge.
        let body_result = self.lower_expr_to_local(closure.body.as_ref());
        if !self.current_is_terminated() {
            self.set_terminator(
                self.current_bb,
                closure.span,
                TerminatorKind::Return {
                    value: Some(Operand::Local(body_result)),
                },
            );
        }

        let out = FunDecl {
            span: closure.span,
            fqn: closure_fqn,
            name: closure_name,
            ty: self.builtins.any,
            params,
            return_ty: closure.body.ty,
            body: Some(self.body),
        };

        (out, self.nested_funs)
    }

    /// 降低 `if` 表达式：生成 then/else/merge 基本块，并在 merge 点写回一个临时结果 local。
    fn lower_if_expr(
        &mut self,
        span: Span,
        ty: TypeId,
        cond: &hir::Expr,
        then_branch: &hir::Expr,
        else_branch: Option<&hir::Expr>,
    ) -> LocalId {
        let result = self.push_temp_local(span, ty);

        // 1) 先在当前块求值条件，并以 CondBr 结束当前块。
        let cond_local = self.lower_expr_to_local(cond);
        let parent = self.current_bb;
        let then_bb = self.push_block(then_branch.span);
        let else_bb = self.push_block(else_branch.map(|e| e.span).unwrap_or(span));
        let merge_bb = self.push_block(span);

        self.set_terminator(
            parent,
            span,
            TerminatorKind::CondBr {
                cond: Operand::Local(cond_local),
                then_target: then_bb,
                else_target: else_bb,
            },
        );

        // 2) then 分支：lower 表达式并写回 result，然后跳到 merge。
        self.current_bb = then_bb;
        let then_value = self.lower_expr_to_local(then_branch);
        if !self.current_is_terminated() {
            self.assign(
                then_branch.span,
                result,
                Rvalue::Use(Operand::Local(then_value)),
            );
            self.set_terminator(
                self.current_bb,
                then_branch.span,
                TerminatorKind::Goto { target: merge_bb },
            );
        }

        // 3) else 分支：同上；若缺省 else，则使用 Unit 占位。
        self.current_bb = else_bb;
        let else_value = else_branch
            .map(|e| self.lower_expr_to_local(e))
            .unwrap_or_else(|| self.emit_unit(span));
        if !self.current_is_terminated() {
            self.assign(span, result, Rvalue::Use(Operand::Local(else_value)));
            self.set_terminator(
                self.current_bb,
                span,
                TerminatorKind::Goto { target: merge_bb },
            );
        }

        // 4) merge：后续语句继续在 merge 块中生成。
        self.current_bb = merge_bb;
        result
    }

    fn lower_pattern(&self, pat: &hir::WhenPat) -> Pattern {
        match pat {
            hir::WhenPat::Else { .. } => Pattern::Else,
            hir::WhenPat::Or { pats, .. } => Pattern::Or {
                pats: pats.iter().map(|pat| self.lower_pattern(pat)).collect(),
            },
            hir::WhenPat::Wildcard { .. } => Pattern::Wildcard,
            hir::WhenPat::Rest { .. } => Pattern::Rest,
            hir::WhenPat::Is { ty, .. } => Pattern::Is { ty: *ty },
            hir::WhenPat::Bind { span, name, .. } => Pattern::Bind {
                name: name.clone(),
                ty: self
                    .facts
                    .when_pat_binding_ty(*span)
                    .unwrap_or(self.builtins.any),
            },
            hir::WhenPat::Tuple { elements, .. } => Pattern::Tuple {
                elements: elements.iter().map(|pat| self.lower_pattern(pat)).collect(),
            },
            hir::WhenPat::Variant { name, args, .. } => Pattern::Variant {
                name: name.clone(),
                args: args.iter().map(|pat| self.lower_pattern(pat)).collect(),
            },
            hir::WhenPat::IntLit { raw, .. } => Pattern::IntLit { raw: raw.clone() },
            hir::WhenPat::CharLit { value, .. } => Pattern::CharLit { value: *value },
            hir::WhenPat::StringLit { value, .. } => Pattern::StringLit {
                value: value.clone(),
            },
            hir::WhenPat::BoolLit { value, .. } => Pattern::BoolLit { value: *value },
        }
    }

    fn when_pat_is_irrefutable(&self, pat: &hir::WhenPat) -> bool {
        matches!(
            pat,
            hir::WhenPat::Else { .. } | hir::WhenPat::Wildcard { .. } | hir::WhenPat::Bind { .. }
        )
    }

    fn collect_when_pattern_bindings(
        &self,
        pat: &hir::WhenPat,
        path: &mut Vec<PatternBindingStep>,
        out: &mut Vec<WhenPatternBinding>,
    ) {
        match pat {
            hir::WhenPat::Bind { span, id, name } => {
                out.push(WhenPatternBinding {
                    id: *id,
                    span: *span,
                    name: name.clone(),
                    ty: self
                        .facts
                        .when_pat_binding_ty(*span)
                        .unwrap_or(self.builtins.any),
                    path: path.clone(),
                });
            }
            hir::WhenPat::Tuple { elements, .. } => {
                for (index, element) in elements.iter().enumerate() {
                    path.push(PatternBindingStep::TupleIndex(index));
                    self.collect_when_pattern_bindings(element, path, out);
                    let _ = path.pop();
                }
            }
            hir::WhenPat::Variant { name, args, .. } => {
                for (field_index, arg) in args.iter().enumerate() {
                    if matches!(arg, hir::WhenPat::Rest { .. }) {
                        continue;
                    }
                    path.push(PatternBindingStep::VariantField {
                        variant: name.clone(),
                        field_index,
                    });
                    self.collect_when_pattern_bindings(arg, path, out);
                    let _ = path.pop();
                }
            }
            hir::WhenPat::Or { pats, .. } => {
                for pat in pats {
                    self.collect_when_pattern_bindings(pat, path, out);
                }
            }
            hir::WhenPat::Else { .. }
            | hir::WhenPat::Wildcard { .. }
            | hir::WhenPat::Rest { .. }
            | hir::WhenPat::Is { .. }
            | hir::WhenPat::IntLit { .. }
            | hir::WhenPat::CharLit { .. }
            | hir::WhenPat::StringLit { .. }
            | hir::WhenPat::BoolLit { .. } => {}
        }
    }

    fn bind_when_pattern_locals(
        &mut self,
        subject_local: LocalId,
        pat: &hir::WhenPat,
    ) -> Vec<(hir::SymbolId, Option<LocalId>)> {
        let mut bindings = Vec::new();
        self.collect_when_pattern_bindings(pat, &mut Vec::new(), &mut bindings);

        let mut shadowed = Vec::with_capacity(bindings.len());
        for binding in bindings {
            let local = self.push_named_local(binding.span, &binding.name, binding.ty);
            self.assign(
                binding.span,
                local,
                Rvalue::PatternExtract {
                    subject: Operand::Local(subject_local),
                    path: binding.path,
                },
            );
            let previous = self.symbol_locals.insert(binding.id, local);
            shadowed.push((binding.id, previous));
        }
        shadowed
    }

    fn restore_shadowed_symbols(&mut self, shadowed: Vec<(hir::SymbolId, Option<LocalId>)>) {
        for (id, previous) in shadowed.into_iter().rev() {
            match previous {
                Some(local) => {
                    self.symbol_locals.insert(id, local);
                }
                None => {
                    self.symbol_locals.remove(&id);
                }
            }
        }
    }

    /// 降低 `when` 表达式：把每个 arm 降为显式 pattern test / binder extract / guard CFG。
    fn lower_when_expr(
        &mut self,
        span: Span,
        ty: TypeId,
        subject: &hir::Expr,
        arms: &[hir::WhenArm],
    ) -> LocalId {
        let result = self.push_temp_local(span, ty);

        // 1) 先在当前块求值 subject。
        let subject_local = self.lower_expr_to_local(subject);
        if self.current_is_terminated() {
            return result;
        }

        // 2) 构造 merge block，并从当前块开始链式生成“匹配测试块”。
        let merge_bb = self.push_block(span);
        let mut test_bb = self.current_bb;

        for arm in arms {
            let irrefutable = self.when_pat_is_irrefutable(&arm.pat);
            let needs_next_test_bb = !irrefutable || arm.guard.is_some();
            let body_bb = arm.guard.as_ref().map(|_| self.push_block(arm.span));
            let next_test_bb = needs_next_test_bb.then(|| self.push_block(arm.span));
            let match_bb = if irrefutable {
                self.current_bb = test_bb;
                let match_bb = self.push_block(arm.span);
                self.set_terminator(test_bb, arm.span, TerminatorKind::Goto { target: match_bb });
                match_bb
            } else {
                let match_bb = self.push_block(arm.span);
                self.current_bb = test_bb;
                let cond = self.push_temp_local(arm.span, self.builtins.bool_);
                self.assign(
                    arm.pat.span(),
                    cond,
                    Rvalue::PatternMatch {
                        subject: Operand::Local(subject_local),
                        pattern: self.lower_pattern(&arm.pat),
                    },
                );
                self.set_terminator(
                    test_bb,
                    arm.span,
                    TerminatorKind::CondBr {
                        cond: Operand::Local(cond),
                        then_target: match_bb,
                        else_target: next_test_bb
                            .expect("refutable when arm should allocate next test block"),
                    },
                );
                match_bb
            };

            self.current_bb = match_bb;
            let shadowed = self.bind_when_pattern_locals(subject_local, &arm.pat);
            if let Some(guard) = &arm.guard {
                let guard_local = self.lower_expr_to_local(guard);
                if !self.current_is_terminated() {
                    self.set_terminator(
                        self.current_bb,
                        guard.span,
                        TerminatorKind::CondBr {
                            cond: Operand::Local(guard_local),
                            then_target: body_bb
                                .expect("guarded when arm should allocate body block"),
                            else_target: next_test_bb
                                .expect("guarded when arm should allocate next test block"),
                        },
                    );
                }
                self.current_bb = body_bb.expect("guarded when arm should allocate body block");
            }

            let body_value = self.lower_expr_to_local(&arm.body);
            if !self.current_is_terminated() {
                self.assign(arm.span, result, Rvalue::Use(Operand::Local(body_value)));
                self.set_terminator(
                    self.current_bb,
                    arm.span,
                    TerminatorKind::Goto { target: merge_bb },
                );
            }
            self.restore_shadowed_symbols(shadowed);

            // 继续下一个 arm 的测试块。
            if irrefutable && arm.guard.is_none() {
                self.current_bb = merge_bb;
                return result;
            }

            test_bb = next_test_bb.expect("fallthrough when arm should allocate next test block");
            self.current_bb = test_bb;
        }

        // 若没有兜底 arm，当前阶段以 `unreachable` 收束。
        self.set_terminator(test_bb, span, TerminatorKind::Unreachable);
        self.current_bb = merge_bb;
        result
    }
}

fn payload_tuple_ty_from_components(
    types: &mut TypeStore,
    unit_ty: TypeId,
    components: &[TypeId],
) -> Option<TypeId> {
    match components {
        [] => Some(unit_ty),
        [single] => Some(*single),
        _ => Some(types.ty_tuple(components.to_vec())),
    }
}

fn continuation_contract_from_type(
    types: &TypeStore,
    continuation_ty: TypeId,
) -> Option<(TypeId, TypeId, EffectRow)> {
    let TypeKind::Ref(RefTypeKind::Nominal(nominal)) = types.kind(continuation_ty) else {
        return None;
    };
    if nominal.fqn != "scoop.core.Continuation" || nominal.args.len() < 2 {
        return None;
    }
    Some((
        nominal.args[0],
        nominal.args[1],
        nominal.eff.clone().unwrap_or_else(EffectRow::pure),
    ))
}

fn find_raise_runtime_error_effect(types: &TypeStore) -> Option<TypeId> {
    let runtime_error_ty = types.iter_ids().find(|&id| {
        matches!(
            types.kind(id),
            TypeKind::Ref(RefTypeKind::Nominal(nominal)) if nominal.fqn == "scoop.core.RuntimeError"
        ) || matches!(
            types.kind(id),
            TypeKind::Value(crate::ty::ValueTypeKind::Nominal(nominal))
                if nominal.fqn == "scoop.core.RuntimeError"
        )
    })?;
    types.iter_ids().find(|&id| {
        matches!(
            types.kind(id),
            TypeKind::Ref(RefTypeKind::Nominal(nominal))
                if nominal.fqn == "scoop.core.Raise"
                    && nominal.args.as_slice() == [runtime_error_ty]
        )
    })
}

fn boxed_symbols_in_block(block: &hir::Block) -> HashSet<hir::SymbolId> {
    let mut out = HashSet::new();
    collect_boxed_symbols_in_block(block, &mut out);
    out
}

fn boxed_symbols_in_expr(expr: &hir::Expr) -> HashSet<hir::SymbolId> {
    let mut out = HashSet::new();
    collect_boxed_symbols_in_expr(expr, &mut out);
    out
}

fn collect_boxed_symbols_in_block(block: &hir::Block, out: &mut HashSet<hir::SymbolId>) {
    for stmt in &block.stmts {
        match &stmt.kind {
            hir::StmtKind::Empty
            | hir::StmtKind::Break { .. }
            | hir::StmtKind::Continue { .. }
            | hir::StmtKind::Todo(_) => {}
            hir::StmtKind::Expr(expr) => collect_boxed_symbols_in_expr(expr, out),
            hir::StmtKind::Val(decl) => {
                if let Some(init) = &decl.init {
                    collect_boxed_symbols_in_expr(init, out);
                }
            }
            hir::StmtKind::Assign { lhs, rhs, .. } => {
                collect_boxed_symbols_in_expr(lhs, out);
                collect_boxed_symbols_in_expr(rhs, out);
            }
            hir::StmtKind::While { cond, body } => {
                collect_boxed_symbols_in_expr(cond, out);
                collect_boxed_symbols_in_block(body, out);
            }
            hir::StmtKind::Return { value } => {
                if let Some(v) = value {
                    collect_boxed_symbols_in_expr(v, out);
                }
            }
        }
    }
}

fn collect_boxed_symbols_in_expr(expr: &hir::Expr, out: &mut HashSet<hir::SymbolId>) {
    match &expr.kind {
        hir::ExprKind::Missing
        | hir::ExprKind::Literal(_)
        | hir::ExprKind::VarRef(_)
        | hir::ExprKind::UnresolvedIdent { .. }
        | hir::ExprKind::Todo(_) => {}
        hir::ExprKind::StructLit { fields, .. } => {
            for f in fields {
                collect_boxed_symbols_in_expr(&f.value, out);
            }
        }
        hir::ExprKind::TupleLit { elements } => {
            for e in elements {
                collect_boxed_symbols_in_expr(e, out);
            }
        }
        hir::ExprKind::InterpolatedString { parts, .. } => {
            for p in parts {
                if let hir::InterpolatedStringPart::Expr { expr } = p {
                    collect_boxed_symbols_in_expr(expr, out);
                }
            }
        }
        hir::ExprKind::Unary { expr, .. } => collect_boxed_symbols_in_expr(expr.as_ref(), out),
        hir::ExprKind::Binary { lhs, rhs, .. } => {
            collect_boxed_symbols_in_expr(lhs.as_ref(), out);
            collect_boxed_symbols_in_expr(rhs.as_ref(), out);
        }
        hir::ExprKind::TypeCheck { expr, .. } | hir::ExprKind::Cast { expr, .. } => {
            collect_boxed_symbols_in_expr(expr.as_ref(), out);
        }
        hir::ExprKind::Block(block) => collect_boxed_symbols_in_block(block, out),
        hir::ExprKind::Closure(closure) => {
            for cap in &closure.captures {
                if cap.mutable {
                    out.insert(cap.id);
                }
            }
            collect_boxed_symbols_in_expr(closure.body.as_ref(), out);
        }
        hir::ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            collect_boxed_symbols_in_expr(cond, out);
            collect_boxed_symbols_in_expr(then_branch, out);
            if let Some(e) = else_branch.as_deref() {
                collect_boxed_symbols_in_expr(e, out);
            }
        }
        hir::ExprKind::When { subject, arms } => {
            collect_boxed_symbols_in_expr(subject, out);
            for arm in arms {
                if let Some(g) = &arm.guard {
                    collect_boxed_symbols_in_expr(g, out);
                }
                collect_boxed_symbols_in_expr(&arm.body, out);
            }
        }
        hir::ExprKind::MemberAccess { receiver, .. } => {
            collect_boxed_symbols_in_expr(receiver, out)
        }
        hir::ExprKind::Call { callee, args } => {
            collect_boxed_symbols_in_expr(callee, out);
            for arg in args {
                match arg {
                    hir::CallArg::Positional(expr) => collect_boxed_symbols_in_expr(expr, out),
                    hir::CallArg::Named { value, .. } => collect_boxed_symbols_in_expr(value, out),
                }
            }
        }
        hir::ExprKind::Perform { args, .. } => {
            for arg in args {
                match arg {
                    hir::CallArg::Positional(expr) => collect_boxed_symbols_in_expr(expr, out),
                    hir::CallArg::Named { value, .. } => collect_boxed_symbols_in_expr(value, out),
                }
            }
        }
        hir::ExprKind::Handle(handle) => {
            collect_boxed_symbols_in_block(&handle.body, out);
            for arm in &handle.arms {
                collect_boxed_symbols_in_expr(&arm.body, out);
            }
            if let Some(finally) = &handle.finally {
                collect_boxed_symbols_in_block(finally, out);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effect_refactor_pipeline::TypedHirEffectContracts;
    use crate::session::Session;
    use crate::source::SourceFile;
    use std::path::PathBuf;

    #[test]
    fn refactor_typed_contracts_clear_legacy_resume_and_perform_fallbacks() {
        let span = Span::new(1, 2);
        let legacy_effect_sites = std::iter::once((
            hir::CallSite::new(PathBuf::from("fixtures/mir_lower_facts.scoop"), span),
            hir::EffectOpCallInfo {
                arg_mapping: vec![0],
                payload_tuple_ty: None,
            },
        ))
        .collect::<hir::EffectOpCallSiteIndex>();
        let dispatch_sites = hir::DispatchCallSiteIndex::default();
        let when_pat_binding_tys = hir::WhenPatBindingTypeIndex::default();
        let top_level_fun_call_sites = hir::TopLevelFunCallSiteIndex::default();

        let facts = MirLoweringFacts::from_hir_side_tables_and_resume_spans(
            &dispatch_sites,
            [span],
            [span],
            &legacy_effect_sites,
            &when_pat_binding_tys,
            &top_level_fun_call_sites,
        )
        .with_refactor_typed_contracts(&TypedHirEffectContracts::default());

        assert!(facts.uses_refactor_typed_contracts());
        assert!(!facts.legacy_resume_site_matches(span));
        assert!(!facts.legacy_resume_site_suspends_outward(span));
        assert!(facts.legacy_perform_site_info(span).is_none());
    }

    #[test]
    fn dump_mir_emits_type_body_generic_member_fun_roots() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/mir_member_root_generic.scoop",
            r#"
package fixtures.mirlower

class Box() {
    fun <eff E = Pure> forward(): Int / E {
        return 1
    }
}

fun <eff E = Pure> wrap(box: Box): Int / E {
    return box.forward<eff E>()
}
"#,
        );

        let lowered = lower_for_dump(&sess, &source).unwrap();
        let fun_fqns = lowered
            .file
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Fun(fun) => Some(fun.fqn.as_str()),
                Item::Todo { .. } => None,
            })
            .collect::<Vec<_>>();

        assert!(
            fun_fqns.contains(&"fixtures.mirlower.Box.forward"),
            "generic MIR lowering 应显式发射 type-body generic member fun root"
        );
        assert!(
            fun_fqns.contains(&"fixtures.mirlower.wrap"),
            "顶层 generic fun root 仍应继续保留"
        );
    }

    #[test]
    fn dump_mir_emits_companion_generic_member_fun_roots() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/mir_companion_member_root_generic.scoop",
            r#"
package fixtures.mirlower

class Box() {
    companion object {
        fun <eff E = Pure> forward(): Int / E {
            return 1
        }
    }
}

fun <eff E = Pure> wrap(): Int / E {
    return Box.forward<eff E>()
}
"#,
        );

        let lowered = lower_for_dump(&sess, &source).unwrap();
        let fun_fqns = lowered
            .file
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Fun(fun) => Some(fun.fqn.as_str()),
                Item::Todo { .. } => None,
            })
            .collect::<Vec<_>>();

        assert!(
            fun_fqns.contains(&"fixtures.mirlower.Box.Companion.forward"),
            "generic MIR lowering 应显式发射 companion generic member fun root"
        );
        assert!(
            fun_fqns.contains(&"fixtures.mirlower.wrap"),
            "顶层 generic fun root 仍应继续保留"
        );
    }

    #[test]
    fn dump_mir_types_comparison_condition_as_bool_in_generic_template() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/mir_generic_compare_bool.scoop",
            r#"
package fixtures.mirlower

fun repeat<T>(x: T, n: Int): T {
    if (n <= 0) {
        return x
    }
    return repeat(x, n - 1)
}
"#,
        );

        let mut lowered = lower_for_dump(&sess, &source).unwrap();
        let builtins = lowered.types.intern_builtins();
        let fun = lowered
            .file
            .items
            .iter()
            .find_map(|item| match item {
                Item::Fun(fun) if fun.fqn == "fixtures.mirlower.repeat" => Some(fun),
                Item::Fun(_) | Item::Todo { .. } => None,
            })
            .expect("expected generic repeat MIR root");
        let body = fun.body.as_ref().expect("repeat should have a MIR body");
        let TerminatorKind::CondBr { cond, .. } =
            &body.blocks[body.start.as_usize()].terminator.kind
        else {
            panic!("expected repeat entry block to branch on comparison");
        };
        let Operand::Local(cond_local) = cond else {
            panic!("comparison condition should be stored in a local");
        };
        let cond_ty = body.locals[cond_local.as_u32() as usize].ty;

        assert_eq!(
            cond_ty, builtins.bool_,
            "MIR comparison result local should be Bool, not an overly broad fallback type"
        );
    }

    #[test]
    fn dump_mir_lowers_user_defined_compare_to_as_direct_call_plus_zero_compare() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/mir_compare_to_direct_call.scoop",
            r#"
package fixtures.mirlower

struct Num(val value: Int) {
    fun compareTo(other: Num): Int {
        return this.value - other.value
    }
}

fun entry(lhs: Num, rhs: Num): Bool {
    return lhs < rhs
}
"#,
        );

        let lowered = lower_for_dump(&sess, &source).unwrap();
        let fun = lowered
            .file
            .items
            .iter()
            .find_map(|item| match item {
                Item::Fun(fun) if fun.fqn == "fixtures.mirlower.entry" => Some(fun),
                Item::Fun(_) | Item::Todo { .. } => None,
            })
            .expect("expected entry MIR root");
        let body = fun.body.as_ref().expect("entry should have a MIR body");
        let entry_block = &body.blocks[body.start.as_usize()];

        assert!(
            entry_block.stmts.iter().any(|stmt| matches!(
                &stmt.kind,
                StatementKind::Assign {
                    value: Rvalue::Call {
                        kind: CallKind::Direct { callee_fqn },
                        args,
                        ..
                    },
                    ..
                } if callee_fqn == "fixtures.mirlower.Num.compareTo" && args.len() == 2
            )),
            "generic MIR compareTo lowering 应显式发射 direct-call target"
        );
        assert!(
            entry_block.stmts.iter().any(|stmt| matches!(
                &stmt.kind,
                StatementKind::Assign {
                    value: Rvalue::Binary { rhs: Operand::Local(local), .. },
                    ..
                } if matches!(
                    body.locals.get(local.as_u32() as usize),
                    Some(LocalDecl { .. })
                )
            )),
            "compareTo direct-call 结果仍应继续进入普通 MIR Binary 比较主线"
        );
        assert!(
            entry_block.stmts.iter().any(|stmt| matches!(
                &stmt.kind,
                StatementKind::Assign {
                    value: Rvalue::Use(Operand::Const(ConstValue::SynthInt(0))),
                    ..
                }
            )),
            "compareTo → 0 比较应在 MIR 中保留显式的合成整数常量"
        );
    }

    #[test]
    fn dump_mir_lowers_compare_to_in_if_condition_as_direct_call() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/mir_compare_to_if_condition.scoop",
            r#"
package fixtures.mirlower

struct Num(val value: Int) {
    fun compareTo(other: Num): Int {
        return this.value - other.value
    }
}

fun entry(lhs: Num, rhs: Num): Int {
    if (lhs < rhs) {
        return 0
    } else {
        return 1
    }
}
"#,
        );

        let lowered = lower_for_dump(&sess, &source).unwrap();
        let fun = lowered
            .file
            .items
            .iter()
            .find_map(|item| match item {
                Item::Fun(fun) if fun.fqn == "fixtures.mirlower.entry" => Some(fun),
                Item::Fun(_) | Item::Todo { .. } => None,
            })
            .expect("expected entry MIR root");
        let body = fun.body.as_ref().expect("entry should have a MIR body");
        let direct_call_stmt = body
            .blocks
            .iter()
            .flat_map(|block| block.stmts.iter())
            .find(|stmt| {
                matches!(
                    &stmt.kind,
                    StatementKind::Assign {
                        value: Rvalue::Call {
                            kind: CallKind::Direct { callee_fqn },
                            args,
                            ..
                        },
                        ..
                    } if callee_fqn == "fixtures.mirlower.Num.compareTo" && args.len() == 2
                )
            });
        assert!(
            direct_call_stmt.is_some(),
            "if 条件里的 compareTo 比较也应显式发射 direct-call target"
        );
        assert!(
            body.blocks
                .iter()
                .flat_map(|block| block.stmts.iter())
                .any(|stmt| matches!(
                    &stmt.kind,
                    StatementKind::Assign {
                        value: Rvalue::Use(Operand::Const(ConstValue::SynthInt(0))),
                        ..
                    }
                )),
            "if 条件里的 compareTo → 0 比较应保留显式 SynthInt(0)"
        );
    }

    #[test]
    fn typed_hir_fixture_preserves_compare_to_direct_call_binding() {
        let sess = Session::new().unwrap();
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/run-pass/operator_overload_struct_basic.scoop")
            .canonicalize()
            .unwrap();
        let source = SourceFile::load(&fixture).unwrap();

        let lowered = crate::hir::lower_typed_for_dump(&sess, &source).unwrap();
        assert!(
            lowered
                .top_level_fun_call_sites
                .values()
                .any(|binding| binding.fqn == "Num.compareTo"),
            "typed HIR side table 应保留 fixture compareTo 站点的 direct-call binding"
        );
    }

    #[test]
    fn dump_mir_publishes_member_write_contract_for_escape_continuation_cell() {
        let sess = Session::new().unwrap();
        let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/run-pass/effect_multi_escape_indirect_direct_while.scoop")
            .canonicalize()
            .unwrap();
        let source = SourceFile::load(&fixture).unwrap();

        let lowered = lower_for_dump(&sess, &source).unwrap();
        let fun = lowered
            .file
            .items
            .iter()
            .find_map(|item| match item {
                Item::Fun(fun) if fun.fqn == "main" => Some(fun),
                Item::Fun(_) | Item::Todo { .. } => None,
            })
            .expect("expected main MIR root");
        let body = fun.body.as_ref().expect("main should have a MIR body");

        assert!(
            body.blocks
                .iter()
                .flat_map(|block| block.stmts.iter())
                .all(|stmt| !matches!(
                    stmt.kind,
                    StatementKind::Todo("assign lhs lowering pending")
                )),
            "member writes should no longer fall back to assign lhs TODO"
        );

        let mut saw_some_k_write = false;
        let mut saw_none_write = false;
        for stmt in body.blocks.iter().flat_map(|block| block.stmts.iter()) {
            let StatementKind::StoreMember {
                member,
                continuation_route,
                ..
            } = &stmt.kind
            else {
                continue;
            };
            let Some(MemberTarget::Value { fqn }) = member.resolved.as_ref() else {
                continue;
            };
            if fqn != "Cell.k" {
                continue;
            }
            match continuation_route {
                StoredContinuationRoutePublication::Unique(route)
                    if matches!(
                        route.path.as_slice(),
                        [PatternBindingStep::VariantField {
                            variant,
                            field_index: 0,
                        }] if variant == "Some"
                    ) =>
                {
                    saw_some_k_write = true;
                }
                StoredContinuationRoutePublication::None => {
                    saw_none_write = true;
                }
                StoredContinuationRoutePublication::Ambiguous
                | StoredContinuationRoutePublication::Unique(_) => {}
            }
        }

        assert!(
            saw_some_k_write,
            "cell.k = Some(k) 应发布 wrapper path + source local 的 continuation write contract"
        );
        assert!(
            saw_none_write,
            "cell.k = none_k 应发布显式 member write contract，而不是 TODO"
        );
    }
}
