//! MIR（Mid-level IR / 当前阶段的 generic early MIR template）。
//!
//! 当前这层 MIR 的职责边界：
//! - 负责把 typed/lowered HIR 收口为 backend-agnostic 的显式 CFG、locals、ANF 风格 operand/materialization，
//!   以及语言级的 call / perform / resume / pattern / member-access 事实；
//! - 负责保留 generic template 语义：函数 `fqn`、`TypeKind::Param`、语言级 dispatch metadata
//!   都在这层继续保持抽象，不提前 materialize 成单态实例；
//! - 不负责承载 LLVM statepoint/address space/stackmap、mangled symbol name、vtable slot / itable id、
//!   GC ABI 或 runtime thunk 等 backend 落地细节。
//!
//! 后续阶段会在此基础上继续做：
//! - monomorphization / instance materialization
//! - per-instance summary / devirtualization / inlining
//! - backend lowering（例如 LLVM codegen）
//!
//! 当前入口仍主要服务 `dump-mir` 与 MIR fixtures；未覆盖节点继续以 `Todo(...)` 占位，
//! 避免在边界收口阶段退回到 panic/隐式后端推断。

mod callables;
mod closure_simplify;
mod escape;
mod inline;
mod lower;
mod materialize;
mod pass_view;
mod summary;

use std::collections::{HashMap, VecDeque};
use std::fmt;

use thiserror::Error;

use crate::ast;
use crate::span::Span;
use crate::ty::{EffectRow, TypeId};

pub(crate) use callables::{MaterializedCallableFamilies, MaterializedCallableFamilyInput};
pub use callables::{MaterializedCallableFamilyView, MaterializedCallableView};
pub use escape::{
    CallableEscapeFacts, ClosureEscapeFact, ContinuationEscapeFact, EscapeStatus,
    MaterializedEscapeFacts,
};
pub use lower::{LoweredMir, MirLowerError, lower_for_dump};
pub(crate) use lower::{MirLoweringFacts, lower_hir_file_for_dump_with_facts};
pub use materialize::{
    InstanceKey, MaterializedMir, MirMaterializeError, TemplateKey, materialize_for_dump,
    materialize_for_dump_with_opt_level,
};
pub use pass_view::{
    MaterializedMirPassArtifacts, MaterializedMirPassView, MaterializedPassCallableFamilyView,
    MaterializedPassCallableView,
};
pub(crate) use summary::{
    DeclOnlySummaryInput, InstanceRootSummaryInput, build_materialized_summary_table,
    summarize_pass_rewritten_fun,
};
pub use summary::{
    InstanceSummary, MaterializedMirSummaries, ParamUseSummary, ResultProvenance,
    ResultProvenanceSource,
};

/// MIR materialization 的 request-root 策略。
#[derive(Debug, Clone, Copy)]
pub enum MaterializeRequestRootMode<'a> {
    /// 将 request source 中的全部 callable 作为 request roots；dump / 调试路径沿用该模式。
    RequestSources,
    /// 只从选定 entry main 和显式 export entry points 出发做实例可达扫描。
    EntryMain { fqn: Option<&'a str> },
}

pub(crate) struct MaterializeCompilationUnitOptions<'a> {
    pub request_source_paths: &'a [std::path::PathBuf],
    pub request_root_mode: MaterializeRequestRootMode<'a>,
    pub opt_level: crate::opt::OptLevel,
}

/// 为编译单元 frontend/build 路径暴露可复用的 MIR materialization 入口。
#[cfg(test)]
pub(crate) fn materialize_compilation_unit_from_typechecked_inputs(
    compilation_unit: &[(&crate::source::SourceFile, &crate::ast::File)],
    request_source_paths: &[std::path::PathBuf],
    index: &crate::resolve::Index,
    type_env: Option<&crate::typecheck::TypeEnv>,
    typecheck_types: &crate::ty::TypeStore,
    monomorph_requests: &[crate::monomorph::MonomorphRequest],
) -> Result<MaterializedMir, Box<MirMaterializeError>> {
    materialize_compilation_unit_from_typechecked_inputs_with_options(
        compilation_unit,
        index,
        type_env,
        typecheck_types,
        monomorph_requests,
        MaterializeCompilationUnitOptions {
            request_source_paths,
            request_root_mode: MaterializeRequestRootMode::RequestSources,
            opt_level: crate::opt::OptLevel::O0,
        },
    )
}

pub(crate) fn materialize_compilation_unit_from_typechecked_inputs_with_opt_level(
    compilation_unit: &[(&crate::source::SourceFile, &crate::ast::File)],
    request_source_paths: &[std::path::PathBuf],
    index: &crate::resolve::Index,
    type_env: Option<&crate::typecheck::TypeEnv>,
    typecheck_types: &crate::ty::TypeStore,
    monomorph_requests: &[crate::monomorph::MonomorphRequest],
    opt_level: crate::opt::OptLevel,
) -> Result<MaterializedMir, Box<MirMaterializeError>> {
    materialize_compilation_unit_from_typechecked_inputs_with_options(
        compilation_unit,
        index,
        type_env,
        typecheck_types,
        monomorph_requests,
        MaterializeCompilationUnitOptions {
            request_source_paths,
            request_root_mode: MaterializeRequestRootMode::RequestSources,
            opt_level,
        },
    )
}

pub(crate) fn materialize_compilation_unit_from_typechecked_inputs_with_options(
    compilation_unit: &[(&crate::source::SourceFile, &crate::ast::File)],
    index: &crate::resolve::Index,
    type_env: Option<&crate::typecheck::TypeEnv>,
    typecheck_types: &crate::ty::TypeStore,
    monomorph_requests: &[crate::monomorph::MonomorphRequest],
    options: MaterializeCompilationUnitOptions<'_>,
) -> Result<MaterializedMir, Box<MirMaterializeError>> {
    materialize::materialize_compilation_unit_from_typechecked_inputs(
        compilation_unit,
        index,
        type_env,
        typecheck_types,
        monomorph_requests,
        options,
    )
}

/// 一个源文件 lowering 后的 MIR（当前阶段主要用于 dump/fixtures）。
#[derive(Debug, Clone)]
pub struct File {
    pub items: Vec<Item>,
}

/// 顶层条目（top-level items）。
#[derive(Debug, Clone)]
pub enum Item {
    Fun(FunDecl),
    /// 未纳入当前阶段 MIR 的条目占位（例如顶层 val/global init、type decl 等）。
    Todo {
        span: Span,
        kind: &'static str,
    },
}

/// 函数声明在 MIR 视图下的承载。
#[derive(Debug, Clone)]
pub struct FunDecl {
    pub span: Span,
    pub fqn: String,
    pub name: String,
    pub ty: TypeId,
    pub params: Vec<Param>,
    pub return_ty: TypeId,
    pub body: Option<Body>,
}

/// 参数在 MIR 视图下的表示：它同时对应一个 local。
#[derive(Debug, Clone)]
pub struct Param {
    pub span: Span,
    pub name: String,
    pub ty: TypeId,
    pub local: LocalId,
}

/// 基本块 ID（在 `Body::blocks` 内的索引）。
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BasicBlockId(u32);

impl BasicBlockId {
    pub(crate) const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    pub fn as_u32(self) -> u32 {
        self.0
    }

    fn as_usize(self) -> usize {
        self.0 as usize
    }
}

impl fmt::Debug for BasicBlockId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "bb{}", self.0)
    }
}

/// 局部变量 ID（在 `Body::locals` 内的索引）。
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LocalId(u32);

impl LocalId {
    pub(crate) const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    pub fn as_u32(self) -> u32 {
        self.0
    }
}

impl fmt::Debug for LocalId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "l{}", self.0)
    }
}

/// 一个 MIR body 内稳定的 effect/call site 身份。
///
/// 约定：
/// - `SiteId` 只要求在同一个 `Body` 内唯一；
/// - lowering 初始分配时按源码/构造顺序单调递增；
/// - 后续 MIR pass 若克隆出新的 `Call` / `Perform` / `Handle` 节点，应为克隆体分配新的
///   `SiteId`，避免与原节点冲突；
/// - 未来 site-level side table 可以用 `(callable/body identity, SiteId)` 作为稳定键。
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SiteId(u32);

impl SiteId {
    pub(crate) const fn from_raw(raw: u32) -> Self {
        Self(raw)
    }

    pub fn as_u32(self) -> u32 {
        self.0
    }
}

impl fmt::Debug for SiteId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "site{}", self.0)
    }
}

/// 一个函数（或顶层 initializer）在 MIR 中的 body。
#[derive(Debug, Clone)]
pub struct Body {
    /// locals 声明表（参数/局部/临时变量；后续会扩展 return local 等约定）。
    pub locals: Vec<LocalDecl>,
    /// 基本块列表（块内顺序执行 statements，最后以 terminator 结束）。
    pub blocks: Vec<BasicBlock>,
    /// CFG 入口块（通常为 `bb0`）。
    pub start: BasicBlockId,
}

impl Body {
    /// 创建一个空 body（调用方需要填充 blocks 并设置 `start`）。
    pub fn new_empty() -> Self {
        Self {
            locals: Vec::new(),
            blocks: Vec::new(),
            start: BasicBlockId(0),
        }
    }

    pub fn push_local(&mut self, decl: LocalDecl) -> LocalId {
        let id = LocalId(u32::try_from(self.locals.len()).expect("too many locals"));
        self.locals.push(decl);
        id
    }

    pub fn push_block(&mut self, bb: BasicBlock) -> BasicBlockId {
        let id = BasicBlockId(u32::try_from(self.blocks.len()).expect("too many basic blocks"));
        self.blocks.push(bb);
        id
    }

    /// 遍历当前 body 内已分配的所有 site id。
    pub fn for_each_site_id(&self, mut f: impl FnMut(SiteId)) {
        for block in &self.blocks {
            for stmt in &block.stmts {
                if let StatementKind::Assign { value, .. } = &stmt.kind
                    && let Some(site_id) = value.site_id()
                {
                    f(site_id);
                }
            }
            if let Some(site_id) = block.terminator.kind.site_id() {
                f(site_id);
            }
        }
    }

    /// 返回当前 body 中尚未使用的最小 `SiteId`。
    ///
    /// 注意：该方法本身不会把 id 写回 body；调用方若要批量生成新节点，应持有返回值并自行递增。
    pub fn next_unused_site_id(&self) -> SiteId {
        let mut next_raw = 0u32;
        self.for_each_site_id(|site_id| {
            next_raw = next_raw.max(site_id.as_u32().saturating_add(1));
        });
        SiteId::from_raw(next_raw)
    }

    /// 检查 CFG 的**结构合法性**：
    /// - `start` 必须在 `blocks` 范围内
    /// - 所有 terminator 的 target 必须在范围内（包含 cleanup/unwind target）
    pub fn validate_cfg(&self) -> Result<(), MirValidationError> {
        if self.blocks.is_empty() {
            return Err(MirValidationError::EmptyBody);
        }
        if self.start.as_usize() >= self.blocks.len() {
            return Err(MirValidationError::InvalidStartBlock {
                start: self.start,
                blocks_len: self.blocks.len(),
            });
        }

        for (idx, block) in self.blocks.iter().enumerate() {
            let from = BasicBlockId(idx as u32);

            // 注意：不分配 Vec，保持验证逻辑轻量；一旦发现无效 target 即返回。
            let mut invalid_target: Option<BasicBlockId> = None;
            block.terminator.for_each_successor(|target| {
                if invalid_target.is_some() {
                    return;
                }
                if target.as_usize() >= self.blocks.len() {
                    invalid_target = Some(target);
                }
            });
            if let Some(target) = invalid_target {
                return Err(MirValidationError::InvalidTarget {
                    from,
                    target,
                    blocks_len: self.blocks.len(),
                });
            }
        }

        Ok(())
    }

    /// 针对 refactor direct-style MIR 的额外形状校验。
    ///
    /// 说明：
    /// - 该验证器建立在 `validate_cfg()` 之上，因此会先检查所有 CFG/cleanup target 是否落在
    ///   `blocks` 范围内；
    /// - 它只约束 P3/P4 会依赖的 direct-style MIR contract，不试图把当前整个 MIR 限制为
    ///   “完全无 Todo”；未纳入本阶段的表达式 lowering 仍可继续用其它 `Todo(...)` 占位。
    pub fn validate_refactor_direct_style(&self) -> Result<(), MirValidationError> {
        self.validate_cfg()?;

        let mut seen_site_ids = HashMap::new();
        for (index, block) in self.blocks.iter().enumerate() {
            let block_id = BasicBlockId(index as u32);

            for stmt in &block.stmts {
                self.validate_refactor_statement(block_id, stmt)?;
                let StatementKind::Assign { value, .. } = &stmt.kind else {
                    continue;
                };
                if let Some(site_id) = value.site_id()
                    && let Some(first_block) = seen_site_ids.insert(site_id, block_id)
                {
                    return Err(MirValidationError::DuplicateSiteId {
                        site_id,
                        first_block,
                        second_block: block_id,
                    });
                }
            }

            self.validate_refactor_unwind(block_id, &block.terminator.unwind)?;
            self.validate_refactor_terminator(block_id, &block.terminator.kind)?;
            if let Some(site_id) = block.terminator.kind.site_id()
                && let Some(first_block) = seen_site_ids.insert(site_id, block_id)
            {
                return Err(MirValidationError::DuplicateSiteId {
                    site_id,
                    first_block,
                    second_block: block_id,
                });
            }
        }

        Ok(())
    }

    fn validate_refactor_statement(
        &self,
        block: BasicBlockId,
        stmt: &Statement,
    ) -> Result<(), MirValidationError> {
        match &stmt.kind {
            StatementKind::Nop => Ok(()),
            StatementKind::Assign { value, .. } => self.validate_refactor_rvalue(block, value),
            StatementKind::StoreMember { .. } | StatementKind::StoreTopLevelVar { .. } => Ok(()),
            StatementKind::Todo(reason) => {
                if is_forbidden_refactor_effect_todo(reason) {
                    return Err(MirValidationError::RefactorTodo { block, reason });
                }
                Ok(())
            }
        }
    }

    fn validate_refactor_rvalue(
        &self,
        block: BasicBlockId,
        value: &Rvalue,
    ) -> Result<(), MirValidationError> {
        if let Rvalue::Todo(reason) = value
            && is_forbidden_refactor_effect_todo(reason)
        {
            return Err(MirValidationError::RefactorTodo { block, reason });
        }
        Ok(())
    }

    fn validate_refactor_unwind(
        &self,
        block: BasicBlockId,
        unwind: &UnwindAction,
    ) -> Result<(), MirValidationError> {
        match unwind {
            UnwindAction::NoUnwind | UnwindAction::Propagate => Ok(()),
            UnwindAction::Cleanup { target } => {
                if !self.blocks[target.as_usize()].is_cleanup {
                    return Err(MirValidationError::CleanupTargetNotMarked {
                        from: block,
                        target: *target,
                    });
                }
                Ok(())
            }
            UnwindAction::Todo(reason) => Err(MirValidationError::RefactorTodo { block, reason }),
        }
    }

    fn validate_refactor_terminator(
        &self,
        block: BasicBlockId,
        kind: &TerminatorKind,
    ) -> Result<(), MirValidationError> {
        match kind {
            TerminatorKind::Handle {
                arms,
                has_finally,
                arm_targets,
                finally_target,
                exit_target,
                ..
            } => {
                if arm_targets.len() != arms.len() {
                    return Err(MirValidationError::InvalidHandleArmTargetCount {
                        from: block,
                        arms_len: arms.len(),
                        targets_len: arm_targets.len(),
                    });
                }
                if finally_target.is_some() != *has_finally {
                    return Err(MirValidationError::InvalidHandleFinallyTarget {
                        from: block,
                        has_finally: *has_finally,
                        finally_target: *finally_target,
                    });
                }
                if let Some(target) = finally_target
                    && !self.blocks[target.as_usize()].is_cleanup
                {
                    return Err(MirValidationError::HandleFinallyTargetNotCleanup {
                        from: block,
                        target: *target,
                    });
                }
                if exit_target.as_usize() >= self.blocks.len() {
                    return Err(MirValidationError::InvalidHandleExitTarget {
                        from: block,
                        target: *exit_target,
                        blocks_len: self.blocks.len(),
                    });
                }
                Ok(())
            }
            TerminatorKind::Todo(reason) if is_forbidden_refactor_effect_todo(reason) => {
                Err(MirValidationError::RefactorTodo { block, reason })
            }
            TerminatorKind::Return { .. }
            | TerminatorKind::ResumeUnwind
            | TerminatorKind::Goto { .. }
            | TerminatorKind::CondBr { .. }
            | TerminatorKind::Unreachable
            | TerminatorKind::Perform { .. }
            | TerminatorKind::Todo(_) => Ok(()),
        }
    }

    /// 从 `start` 出发，计算可达的基本块集合（按 BFS 顺序返回）。
    pub fn reachable_blocks(&self) -> Result<Vec<BasicBlockId>, MirValidationError> {
        self.validate_cfg()?;

        let mut visited = vec![false; self.blocks.len()];
        let mut order = Vec::new();
        let mut queue = VecDeque::new();

        visited[self.start.as_usize()] = true;
        queue.push_back(self.start);

        while let Some(bb) = queue.pop_front() {
            order.push(bb);

            let block = &self.blocks[bb.as_usize()];
            block.terminator.for_each_successor(|succ| {
                if visited[succ.as_usize()] {
                    return;
                }
                visited[succ.as_usize()] = true;
                queue.push_back(succ);
            });
        }

        Ok(order)
    }

    /// 检查 CFG 是否“全连通”：所有基本块都从 `start` 可达。
    pub fn is_fully_reachable(&self) -> Result<bool, MirValidationError> {
        let reachable = self.reachable_blocks()?;
        Ok(reachable.len() == self.blocks.len())
    }

    /// 列出不可达的基本块（用于测试与后续 pass 的诊断）。
    pub fn unreachable_blocks(&self) -> Result<Vec<BasicBlockId>, MirValidationError> {
        self.validate_cfg()?;

        let mut visited = vec![false; self.blocks.len()];
        let mut queue = VecDeque::new();

        visited[self.start.as_usize()] = true;
        queue.push_back(self.start);

        while let Some(bb) = queue.pop_front() {
            let block = &self.blocks[bb.as_usize()];
            block.terminator.for_each_successor(|succ| {
                if visited[succ.as_usize()] {
                    return;
                }
                visited[succ.as_usize()] = true;
                queue.push_back(succ);
            });
        }

        let mut unreachable = Vec::new();
        for (idx, ok) in visited.iter().copied().enumerate() {
            if !ok {
                unreachable.push(BasicBlockId(idx as u32));
            }
        }
        Ok(unreachable)
    }
}

/// MIR local 的稳定来源分类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalSourceKind {
    SourceLocal,
    CompilerTemporary,
}

/// 一个 local 的声明信息。
#[derive(Debug, Clone)]
pub struct LocalDecl {
    pub span: Span,
    pub name: Option<String>,
    pub ty: TypeId,
    pub source: LocalSourceKind,
}

/// MIR 基本块：顺序语句 + 终结指令（terminator）。
#[derive(Debug, Clone)]
pub struct BasicBlock {
    /// 是否为 cleanup block（用于 `finally`/effect unwinding）。
    ///
    /// 该标记本身不影响 CFG 连通性；主要用于 dump/诊断与后续更严格的 MIR 规则。
    pub is_cleanup: bool,
    pub stmts: Vec<Statement>,
    pub terminator: Terminator,
}

/// MIR 语句（顺序执行）。
#[derive(Debug, Clone)]
pub struct Statement {
    pub span: Span,
    pub kind: StatementKind,
}

#[derive(Debug, Clone)]
pub enum StatementKind {
    Nop,
    /// `target = value`（最小赋值语句，用于 if/when merge 等场景）。
    Assign {
        target: LocalId,
        value: Rvalue,
    },
    /// `receiver.member = value` 的显式 member write contract。
    ///
    /// 说明：
    /// - 该节点保留 member identity、写入值来源，以及 continuation 值穿过 wrapper/aggregate 时的
    ///   published route；
    /// - 它是供后续 effect/late-lowering/LLVM handoff 消费的 compiler-owned contract，而不是 backend-specific
    ///   store lowering；
    /// - `continuation_route=None` 表示该写入值不发布 continuation route；
    /// - `continuation_route=Ambiguous` 表示 lowering 观察到了多个互不兼容的 continuation payload path，
    ///   后续阶段必须显式拒绝而不是自行猜测。
    StoreMember {
        receiver: Operand,
        member: MemberAccessMetadata,
        value: Operand,
        value_ty: TypeId,
        continuation_route: StoredContinuationRoutePublication,
    },
    /// `top.level.var = value` 的显式写入 contract。
    StoreTopLevelVar {
        fqn: String,
        value: Operand,
        value_ty: TypeId,
    },
    /// 未实现节点占位（用于尽早落地数据结构但避免 `todo!()`/panic）。
    Todo(&'static str),
}

/// 一个“可以被使用的值”（最小 operand 模型）。
#[derive(Debug, Clone)]
pub enum Operand {
    Local(LocalId),
    Const(ConstValue),
}

/// 顶层值/函数引用在 MIR 上保留的最小 provenance。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopLevelRef {
    pub fqn: String,
}

/// 成员访问在 MIR 上保留的最小语言级 metadata。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemberAccessMetadata {
    pub name: String,
    pub receiver_ty: TypeId,
    pub resolved: Option<MemberTarget>,
    pub hidden_effects: EffectRow,
}

/// 已解析成员在 MIR 上的稳定目标种类。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemberTarget {
    Value { fqn: String },
    Fun { fqn: String },
    ExtensionValue { fqn: String },
    ExtensionFun { fqn: String },
}

/// 调用实参在 MIR 中的最小表示。
///
/// 说明：
/// - `value` 总是已经先被 lowering 为 operand / local，便于后续按 ANF 风格分析求值顺序；
/// - `name` 仅在源级为命名参数时存在；当前阶段保留它，避免后续 pass 被迫回到 HIR 读取调用形状。
#[derive(Debug, Clone)]
pub struct CallArg {
    pub span: Span,
    pub name: Option<String>,
    pub value: Operand,
}

/// 插值字符串在 MIR 上保留的 ANF 片段。
#[derive(Debug, Clone)]
pub enum InterpolatedStringPart {
    Text {
        span: Span,
    },
    Expr {
        span: Span,
        value: Operand,
        ty: TypeId,
    },
}

/// struct literal 在 MIR 上保留的 ANF 字段初始化项。
#[derive(Debug, Clone)]
pub struct StructLitField {
    pub span: Span,
    pub name: String,
    pub value: Operand,
}

/// `perform` payload 在 MIR 上的一个已排序参数槽位。
///
/// 说明：
/// - `value` 仍按源码求值顺序先被 lower 为 operand/local；
/// - `source_arg_index` 记录该 payload 来自调用点第几个显式实参，便于后续 pass 同时看到
///   “按参数顺序归一化后的 payload 视图”和“原始调用点位置”。
#[derive(Debug, Clone)]
pub struct PerformArg {
    pub span: Span,
    pub source_arg_index: usize,
    pub name: Option<String>,
    pub value: Operand,
}

/// `perform` 调用点在 MIR 上保留的最小 metadata。
#[derive(Debug, Clone)]
pub struct PerformMetadata {
    pub effect_ty: TypeId,
    pub payload_tuple_ty: Option<TypeId>,
    pub payload_component_tys: Vec<TypeId>,
    pub arg_mapping: Vec<usize>,
}

/// virtual / interface dispatch 在 MIR 上保留的最小语言级 metadata。
///
/// 注意：
/// - 这里只保留 receiver 的静态类型与被调成员的声明身份；
/// - 不把 vtable slot / itable id / runtime thunk 等后端细节编码进 MIR。
#[derive(Debug, Clone)]
pub struct DispatchMetadata {
    pub owner_fqn: String,
    pub member_name: String,
    pub receiver_ty: TypeId,
}

/// `Continuation.resume(...)` 在 MIR 上保留的最小语义 metadata。
///
/// 注意：
/// - 当前会显式记录 `ResumeTuple` / `Answer` / `Out`，以及 ordinary `Raise<RuntimeError>`
///   required-effect contract；
/// - runtime replay token / payload transport 等细节仍属于更晚的 lowering 阶段。
#[derive(Debug, Clone)]
pub struct ResumeMetadata {
    pub continuation_ty: TypeId,
    pub resume_ty: TypeId,
    pub answer_ty: TypeId,
    pub return_ty: TypeId,
    pub out_effects: EffectRow,
    pub runtime_error_effect_ty: Option<TypeId>,
    pub suspends_outward: bool,
}

/// `handle { ... } with { ... }` 站点在 MIR 上保留的 typed contract。
#[derive(Debug, Clone)]
pub struct HandleMetadata {
    pub result_ty: TypeId,
    pub body_result_ty: TypeId,
    pub finally_result_ty: Option<TypeId>,
}

/// `handle` arm 在 MIR 上的显式语义 kind。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandlerArmKind {
    NonResuming,
    EscapeContinuation,
}

/// MIR 上显式区分的调用种类。
///
/// 注意：
/// - 这里刻意只表达语言级调用形态，不表达 LLVM vtable/itable/statepoint 等后端细节；
/// - direct / closure / fun-value / virtual / interface / resume 共用同一调用层级，
///   避免后续 pass 再回到 HIR 或 LLVM codegen 现场恢复控制转移语义。
#[derive(Debug, Clone)]
pub enum CallKind {
    /// 目标函数在 MIR 上已经静态唯一确定。
    Direct { callee_fqn: String },
    /// 已知调用的是某个 closure value。
    ///
    /// `fn_ptr` 记录该 closure 当前可恢复出的唯一 invoke target，便于后续 closure/provenance 分析。
    Closure { callee: Operand, fn_ptr: String },
    /// 调用一个函数值，但当前还不足以把它恢复成更具体的 direct/closure 形态。
    FunValue { callee: Operand },
    /// class virtual dispatch（语言级“按 receiver 动态分派到 class override”）。
    Virtual {
        receiver: Operand,
        dispatch: DispatchMetadata,
    },
    /// interface dispatch（语言级“按 receiver 的 interface 实现做动态分派”）。
    Interface {
        receiver: Operand,
        dispatch: DispatchMetadata,
    },
    /// `Continuation.resume(...)`。
    Resume {
        continuation: Operand,
        resume: ResumeMetadata,
    },
}

/// 常量值（当前阶段不保留字面量原始内容，仅保留种类）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConstValue {
    Bool(bool),
    Char,
    Unit,
    Int,
    /// 编译器合成的整数字面量值（当前主要用于 desugaring / compareTo → 0 比较等场景）。
    ///
    /// 说明：
    /// - 与 `Int` 不同，这里显式保留字面量值，避免后续阶段必须回切源码才能恢复 `0` / `1`；
    /// - 目前仍只用于“编译器自身生成”的 `Int` 常量，不改变源码整数字面量继续按 `span` 回切的主路径。
    SynthInt(i64),
    Float64,
    Float32,
    String,
}

/// `when` pattern 在 MIR 上的 backend-agnostic 表示。
#[derive(Debug, Clone)]
pub enum Pattern {
    Else,
    Or { pats: Vec<Pattern> },
    Wildcard,
    Rest,
    Is { ty: TypeId },
    Bind { name: String, ty: TypeId },
    Tuple { elements: Vec<Pattern> },
    Variant { name: String, args: Vec<Pattern> },
    IntLit { raw: String },
    CharLit { value: char },
    StringLit { value: String },
    BoolLit { value: bool },
}

/// 从一个已匹配 subject 中提取 binder 值时使用的投影路径。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PatternBindingStep {
    TupleIndex(usize),
    VariantField { variant: String, field_index: usize },
}

/// member write 中“值内 continuation payload 路径”的最小 published contract。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredContinuationValueRoute {
    pub source_local: LocalId,
    pub source_ty: TypeId,
    pub path: Vec<PatternBindingStep>,
}

/// member write 对 continuation payload path 的 published 结论。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoredContinuationRoutePublication {
    None,
    Unique(StoredContinuationValueRoute),
    Ambiguous,
}

/// 右值（最小 rvalue 模型）。
#[derive(Debug, Clone)]
pub enum Rvalue {
    Use(Operand),
    TopLevelRef(TopLevelRef),
    UnresolvedName {
        name: String,
    },
    Unary {
        op: ast::UnaryOp,
        operand: Operand,
    },
    Binary {
        lhs: Operand,
        op: ast::BinaryOp,
        rhs: Operand,
    },
    TypeCheck {
        value: Operand,
        op: ast::TypeCheckOp,
        test_ty: TypeId,
    },
    Cast {
        value: Operand,
        op: ast::CastOp,
        target_ty: TypeId,
    },
    MemberAccess {
        site_id: Option<SiteId>,
        receiver: Operand,
        member: MemberAccessMetadata,
    },
    /// Enum/Option variant constructor after typed HIR has supplied the expected enum type.
    EnumVariant {
        enum_ty: TypeId,
        variant_name: String,
        args: Vec<CallArg>,
    },
    /// Class constructor call after typed HIR has identified the nominal result class.
    ClassCtor {
        site_id: SiteId,
        class_fqn: String,
        args: Vec<CallArg>,
        hidden_effects: EffectRow,
    },
    /// 一个显式普通调用节点。
    ///
    /// 当前阶段承载 direct / closure / fun-value / virtual / interface / resume 六类调用；
    /// 更晚若补更多调用/控制转移语义，也应继续复用同一调用层级，而不是再造平行表示。
    Call {
        site_id: SiteId,
        kind: CallKind,
        args: Vec<CallArg>,
    },
    /// 创建一个 tuple 值（最小 aggregate，用于 env struct 等场景）。
    MakeTuple {
        elements: Vec<Operand>,
    },
    /// 创建一个 struct 值。字段值已按源码求值顺序先 lower 为 operand。
    StructLit {
        fields: Vec<StructLitField>,
    },
    /// 编译期 `sizeOf(value)` intrinsic；`value` 本身不求值，只消费静态类型。
    SizeOf {
        value_ty: TypeId,
    },
    /// 运行期插值字符串构造。表达式片段已按 ANF 先求值为 operand。
    InterpolatedString {
        raw: bool,
        parts: Vec<InterpolatedStringPart>,
    },
    /// 读取 tuple 的某个字段：`tuple[index]`（按捕获顺序索引）。
    TupleGet {
        tuple: Operand,
        index: usize,
    },
    /// 创建一个“可变捕获盒”（T0714）。
    ///
    /// 语义：把一个值装入 heap cell，并返回该 cell 的引用；同一个 cell 可被多个 closure 共享捕获，
    /// 从而保证 `var` 在内外层的读写具备别名一致性。
    CaptureBoxNew {
        value: Operand,
    },
    /// 从“可变捕获盒”读取当前值（T0714）。
    CaptureBoxGet {
        box_operand: Operand,
    },
    /// 向“可变捕获盒”写入新值（T0714）。
    ///
    /// 注意：该 rvalue 本身的“结果值”在当前阶段并不重要（通常写入一个 `Unit` 临时 local），
    /// 主要用于在 MIR dump/fixtures 中显式体现写回语义。
    CaptureBoxSet {
        box_operand: Operand,
        value: Operand,
    },
    /// 一个 `when` arm 的 pattern test（结果为 Bool）。
    PatternMatch {
        subject: Operand,
        pattern: Pattern,
    },
    /// 从一个已经匹配成功的 subject 中提取 pattern binder 值。
    PatternExtract {
        subject: Operand,
        path: Vec<PatternBindingStep>,
    },
    /// 创建一个函数值（closure）：`{ env_struct, fn_ptr }`（T0710/T0711）。
    ///
    /// 当前阶段：
    /// - `env` 支持 `Unit`（无捕获）或最小 tuple env（T0711）；
    /// - 更丰富的 env 表示（真正的 struct/layout/heap/GC）会在后续 codegen/runtime 任务补齐。
    MakeClosure {
        env: Operand,
        fn_ptr: String,
    },
    /// `perform` 被 handler/resume 继续执行后，原表达式位置接收到的结果值 provenance。
    PerformResult {
        op_fqn: String,
        effect_ty: TypeId,
    },
    Todo(&'static str),
}

/// MIR terminator（显式控制流）。
#[derive(Debug, Clone)]
pub struct Terminator {
    pub span: Span,
    pub kind: TerminatorKind,
    /// 当该 terminator 发生 unwinding（例如 effect 传播）时应采取的动作。
    pub unwind: UnwindAction,
}

/// terminator 在发生 unwinding 时应采取的动作（最小模型，用于 `finally`/effect unwinding）。
#[derive(Debug, Clone)]
pub enum UnwindAction {
    /// 该 terminator 不会发生 unwinding。
    NoUnwind,
    /// 若发生 unwinding，则直接继续向外传播；当前 body 内无需额外 cleanup。
    Propagate,
    /// 若发生 unwinding，则先跳转到 cleanup block 执行清理逻辑。
    Cleanup { target: BasicBlockId },
    /// 未实现占位：表示“可能会 unwind，但具体行为尚未建模”。
    Todo(&'static str),
}

#[derive(Debug, Clone)]
pub enum TerminatorKind {
    /// 从当前 callable 正常返回；`value=None` 表示 `Unit`/隐式返回。
    Return {
        value: Option<Operand>,
    },
    /// cleanup block：执行完清理逻辑后继续向上传播 unwinding。
    ResumeUnwind,
    Goto {
        target: BasicBlockId,
    },
    /// 条件分支：若 `cond` 为真跳转到 `then_target`，否则跳转到 `else_target`。
    CondBr {
        cond: Operand,
        then_target: BasicBlockId,
        else_target: BasicBlockId,
    },
    Unreachable,
    /// effect operation 调用（对应 HIR 的 `ExprKind::Perform`）。
    ///
    /// 当前阶段仅保留“发生了哪一个 effect op”的信息；具体如何进入 handler/如何建模 unwinding
    /// 由后续 effect lowering 任务（TODO T0713/T0707）决定。
    Perform {
        site_id: SiteId,
        op_fqn: String,
        metadata: PerformMetadata,
        args: Vec<PerformArg>,
        /// 被 handler/continuation 恢复后，普通计算继续所在的 direct-style CFG block。
        resume_target: BasicBlockId,
    },
    /// effect handler 区域（对应 HIR 的 `ExprKind::Handle`）。
    ///
    /// 注意：该变体目前仍是“结构占位”，但会携带保守 CFG target，确保 MIR reachability
    /// 能看见 handler body / arms / finally 中保形保留下来的调用点。更晚的 effect lowering
    /// 仍会把 handle 展开为完整的 cleanup/handler 栈管理。
    Handle {
        site_id: SiteId,
        metadata: HandleMetadata,
        arms: Vec<HandlerArm>,
        has_finally: bool,
        body_target: BasicBlockId,
        arm_targets: Vec<BasicBlockId>,
        finally_target: Option<BasicBlockId>,
        /// handle 表达式正常完成（经 body/arm/finally 收束）后，外层求值继续所在的 block。
        exit_target: BasicBlockId,
    },
    /// 未实现控制流占位（例如 if/switch/call/cleanup 等）。
    Todo(&'static str),
}

/// `handle` 在 MIR 视图下的一个 handler arm（结构占位）。
#[derive(Debug, Clone)]
pub struct HandlerArm {
    pub op_fqn: String,
    /// arm payload binder 数量（与 `binder_locals.len()` 保持一致）。
    pub binder_count: usize,
    /// arm payload binder 在当前 body 中的隐式输入 local。
    ///
    /// 这些 local 没有单独的赋值语句；它们由 `TerminatorKind::Handle` 进入对应 `arm_target` 时
    /// 作为 block input 被带入，供 arm body 直接引用。
    pub binder_locals: Vec<LocalId>,
    /// 逃逸 continuation arm 的显式 continuation binder local（若存在）。
    pub continuation_local: Option<LocalId>,
    pub handled_effect_ty: TypeId,
    pub payload_tuple_ty: Option<TypeId>,
    pub payload_component_tys: Vec<TypeId>,
    pub body_ty: TypeId,
    pub kind: HandlerArmKind,
}

impl TerminatorKind {
    pub fn site_id(&self) -> Option<SiteId> {
        match self {
            TerminatorKind::Perform { site_id, .. } | TerminatorKind::Handle { site_id, .. } => {
                Some(*site_id)
            }
            TerminatorKind::Return { .. }
            | TerminatorKind::ResumeUnwind
            | TerminatorKind::Goto { .. }
            | TerminatorKind::CondBr { .. }
            | TerminatorKind::Unreachable
            | TerminatorKind::Todo(_) => None,
        }
    }

    /// 对 terminator 的“正常”后继基本块调用回调（不包含 cleanup/unwind 边）。
    ///
    /// 该接口适合做 CFG 分析（reachable/循环检测等），避免为每次查询分配 `Vec`。
    pub fn for_each_successor(&self, mut f: impl FnMut(BasicBlockId)) {
        match self {
            TerminatorKind::Perform { resume_target, .. } => f(*resume_target),
            TerminatorKind::Goto { target } => f(*target),
            TerminatorKind::CondBr {
                then_target,
                else_target,
                ..
            } => {
                f(*then_target);
                f(*else_target);
            }
            TerminatorKind::Handle {
                body_target,
                arm_targets,
                finally_target,
                ..
            } => {
                f(*body_target);
                for target in arm_targets {
                    f(*target);
                }
                if let Some(target) = finally_target {
                    f(*target);
                }
            }
            TerminatorKind::Return { .. }
            | TerminatorKind::ResumeUnwind
            | TerminatorKind::Unreachable
            | TerminatorKind::Todo(_) => {}
        }
    }
}

impl Rvalue {
    pub fn site_id(&self) -> Option<SiteId> {
        match self {
            Rvalue::Call { site_id, .. } | Rvalue::ClassCtor { site_id, .. } => Some(*site_id),
            Rvalue::Use(_)
            | Rvalue::TopLevelRef(_)
            | Rvalue::UnresolvedName { .. }
            | Rvalue::Unary { .. }
            | Rvalue::Binary { .. }
            | Rvalue::TypeCheck { .. }
            | Rvalue::Cast { .. }
            | Rvalue::MemberAccess { .. }
            | Rvalue::EnumVariant { .. }
            | Rvalue::MakeTuple { .. }
            | Rvalue::StructLit { .. }
            | Rvalue::SizeOf { .. }
            | Rvalue::InterpolatedString { .. }
            | Rvalue::TupleGet { .. }
            | Rvalue::CaptureBoxNew { .. }
            | Rvalue::CaptureBoxGet { .. }
            | Rvalue::CaptureBoxSet { .. }
            | Rvalue::PatternMatch { .. }
            | Rvalue::PatternExtract { .. }
            | Rvalue::MakeClosure { .. }
            | Rvalue::PerformResult { .. }
            | Rvalue::Todo(_) => None,
        }
    }
}

impl Terminator {
    /// 对 terminator 的后继基本块调用回调（包含 cleanup/unwind 边）。
    pub fn for_each_successor(&self, mut f: impl FnMut(BasicBlockId)) {
        self.kind.for_each_successor(&mut f);
        if let UnwindAction::Cleanup { target } = &self.unwind {
            f(*target);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MirValidationError {
    /// MIR body 为空（没有任何基本块）。
    #[error("MIR body is empty")]
    EmptyBody,
    /// `start` 超出 `blocks` 范围。
    #[error("invalid start block {start:?} for {blocks_len} blocks")]
    InvalidStartBlock {
        start: BasicBlockId,
        blocks_len: usize,
    },
    /// terminator 的 target 超出 `blocks` 范围。
    #[error("invalid target {target:?} from {from:?} for {blocks_len} blocks")]
    InvalidTarget {
        from: BasicBlockId,
        target: BasicBlockId,
        blocks_len: usize,
    },
    #[error("duplicate site id {site_id:?} in {first_block:?} and {second_block:?}")]
    DuplicateSiteId {
        site_id: SiteId,
        first_block: BasicBlockId,
        second_block: BasicBlockId,
    },
    #[error("cleanup target {target:?} from {from:?} is not marked cleanup")]
    CleanupTargetNotMarked {
        from: BasicBlockId,
        target: BasicBlockId,
    },
    #[error("handle at {from:?} has {arms_len} arms but {targets_len} arm targets")]
    InvalidHandleArmTargetCount {
        from: BasicBlockId,
        arms_len: usize,
        targets_len: usize,
    },
    #[error(
        "handle at {from:?} has_finally={has_finally} but finally target is {finally_target:?}"
    )]
    InvalidHandleFinallyTarget {
        from: BasicBlockId,
        has_finally: bool,
        finally_target: Option<BasicBlockId>,
    },
    #[error("handle finally target {target:?} from {from:?} is not marked cleanup")]
    HandleFinallyTargetNotCleanup {
        from: BasicBlockId,
        target: BasicBlockId,
    },
    #[error("handle exit target {target:?} from {from:?} is out of range for {blocks_len} blocks")]
    InvalidHandleExitTarget {
        from: BasicBlockId,
        target: BasicBlockId,
        blocks_len: usize,
    },
    #[error("refactor MIR still contains forbidden effect/control todo `{reason}` in {block:?}")]
    RefactorTodo {
        block: BasicBlockId,
        reason: &'static str,
    },
}

fn is_forbidden_refactor_effect_todo(reason: &str) -> bool {
    matches!(
        reason,
        "handle result pending"
            | "handle body exit pending"
            | "handle arm exit pending"
            | "handle finally exit pending"
            | "perform unwind pending"
            | "refactor perform contract missing"
            | "refactor handle contract missing"
            | "resume lowering requires canonical callee shape"
            | "break not in loop"
            | "continue not in loop"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::Session;
    use crate::source::SourceFile;
    use crate::ty::{TypeKind, TypeStore};

    #[test]
    fn cfg_reachable_two_blocks_ok() {
        let mut types = TypeStore::default();
        let builtins = types.intern_builtins();

        let mut body = Body::new_empty();
        let _tmp = body.push_local(LocalDecl {
            span: Span::new(0, 0),
            name: Some("tmp0".to_string()),
            ty: builtins.unit,
            source: LocalSourceKind::CompilerTemporary,
        });

        let bb0 = body.push_block(BasicBlock {
            is_cleanup: false,
            stmts: vec![Statement {
                span: Span::new(0, 0),
                kind: StatementKind::Nop,
            }],
            terminator: Terminator {
                span: Span::new(0, 0),
                kind: TerminatorKind::Goto {
                    target: BasicBlockId(1),
                },
                unwind: UnwindAction::NoUnwind,
            },
        });
        let bb1 = body.push_block(BasicBlock {
            is_cleanup: false,
            stmts: Vec::new(),
            terminator: Terminator {
                span: Span::new(0, 0),
                kind: TerminatorKind::Return { value: None },
                unwind: UnwindAction::NoUnwind,
            },
        });

        body.start = bb0;

        assert_eq!(bb0, BasicBlockId(0));
        assert_eq!(bb1, BasicBlockId(1));
        assert!(body.validate_cfg().is_ok());
        assert_eq!(body.reachable_blocks().unwrap(), vec![bb0, bb1]);
        assert!(body.is_fully_reachable().unwrap());
        assert!(body.unreachable_blocks().unwrap().is_empty());
    }

    #[test]
    fn cfg_invalid_target_is_error() {
        let mut body = Body::new_empty();
        let bb0 = body.push_block(BasicBlock {
            is_cleanup: false,
            stmts: Vec::new(),
            terminator: Terminator {
                span: Span::new(0, 0),
                kind: TerminatorKind::Goto {
                    target: BasicBlockId(42),
                },
                unwind: UnwindAction::NoUnwind,
            },
        });
        body.start = bb0;

        assert_eq!(
            body.validate_cfg(),
            Err(MirValidationError::InvalidTarget {
                from: BasicBlockId(0),
                target: BasicBlockId(42),
                blocks_len: 1,
            })
        );
    }

    #[test]
    fn cfg_cleanup_edge_is_reachable() {
        // 模拟一个“可能 unwind 的 terminator”：
        // - 正常路径不存在（Perform 目前作为占位 terminator）
        // - unwind 路径跳到 cleanup block，然后用 ResumeUnwind 继续传播
        let mut types = TypeStore::default();
        let builtins = types.intern_builtins();
        let mut body = Body::new_empty();

        let bb0 = body.push_block(BasicBlock {
            is_cleanup: false,
            stmts: Vec::new(),
            terminator: Terminator {
                span: Span::new(0, 0),
                kind: TerminatorKind::Perform {
                    site_id: SiteId::from_raw(0),
                    op_fqn: "scoop.core.Raise.raise".to_string(),
                    metadata: PerformMetadata {
                        effect_ty: builtins.unit,
                        payload_tuple_ty: None,
                        payload_component_tys: Vec::new(),
                        arg_mapping: Vec::new(),
                    },
                    args: Vec::new(),
                    resume_target: BasicBlockId(1),
                },
                unwind: UnwindAction::Cleanup {
                    target: BasicBlockId(2),
                },
            },
        });
        let bb1 = body.push_block(BasicBlock {
            is_cleanup: false,
            stmts: Vec::new(),
            terminator: Terminator {
                span: Span::new(0, 0),
                kind: TerminatorKind::Return { value: None },
                unwind: UnwindAction::NoUnwind,
            },
        });
        let bb2 = body.push_block(BasicBlock {
            is_cleanup: true,
            stmts: Vec::new(),
            terminator: Terminator {
                span: Span::new(0, 0),
                kind: TerminatorKind::ResumeUnwind,
                unwind: UnwindAction::NoUnwind,
            },
        });
        body.start = bb0;

        assert_eq!(bb0, BasicBlockId(0));
        assert_eq!(bb1, BasicBlockId(1));
        assert_eq!(bb2, BasicBlockId(2));
        assert!(body.validate_cfg().is_ok());
        assert_eq!(body.reachable_blocks().unwrap(), vec![bb0, bb1, bb2]);
        assert!(body.is_fully_reachable().unwrap());
        assert!(body.unreachable_blocks().unwrap().is_empty());
        assert!(body.blocks[bb2.as_usize()].is_cleanup);
    }

    #[test]
    fn refactor_mir_cfg_rejects_cleanup_target_without_cleanup_flag() {
        let mut types = TypeStore::default();
        let builtins = types.intern_builtins();
        let mut body = Body::new_empty();
        let result_local = body.push_local(LocalDecl {
            span: Span::new(0, 0),
            name: Some("tmp0".to_string()),
            ty: builtins.unit,
            source: LocalSourceKind::CompilerTemporary,
        });

        let bb0 = body.push_block(BasicBlock {
            is_cleanup: false,
            stmts: vec![Statement {
                span: Span::new(0, 0),
                kind: StatementKind::Assign {
                    target: result_local,
                    value: Rvalue::PerformResult {
                        op_fqn: "scoop.core.Raise.raise".to_string(),
                        effect_ty: builtins.unit,
                    },
                },
            }],
            terminator: Terminator {
                span: Span::new(0, 0),
                kind: TerminatorKind::Perform {
                    site_id: SiteId::from_raw(0),
                    op_fqn: "scoop.core.Raise.raise".to_string(),
                    metadata: PerformMetadata {
                        effect_ty: builtins.unit,
                        payload_tuple_ty: None,
                        payload_component_tys: Vec::new(),
                        arg_mapping: Vec::new(),
                    },
                    args: Vec::new(),
                    resume_target: BasicBlockId(1),
                },
                unwind: UnwindAction::Cleanup {
                    target: BasicBlockId(2),
                },
            },
        });
        let _bb1 = body.push_block(BasicBlock {
            is_cleanup: false,
            stmts: Vec::new(),
            terminator: Terminator {
                span: Span::new(0, 0),
                kind: TerminatorKind::Return { value: None },
                unwind: UnwindAction::NoUnwind,
            },
        });
        let _bb2 = body.push_block(BasicBlock {
            is_cleanup: false,
            stmts: Vec::new(),
            terminator: Terminator {
                span: Span::new(0, 0),
                kind: TerminatorKind::ResumeUnwind,
                unwind: UnwindAction::NoUnwind,
            },
        });
        body.start = bb0;

        assert_eq!(
            body.validate_refactor_direct_style(),
            Err(MirValidationError::CleanupTargetNotMarked {
                from: BasicBlockId(0),
                target: BasicBlockId(2),
            })
        );
    }

    #[test]
    fn refactor_mir_site_id_rejects_duplicate_call_and_terminator_site_ids() {
        let mut types = TypeStore::default();
        let builtins = types.intern_builtins();
        let mut body = Body::new_empty();
        let result_local = body.push_local(LocalDecl {
            span: Span::new(0, 0),
            name: Some("tmp0".to_string()),
            ty: builtins.unit,
            source: LocalSourceKind::CompilerTemporary,
        });

        let bb0 = body.push_block(BasicBlock {
            is_cleanup: false,
            stmts: vec![Statement {
                span: Span::new(0, 0),
                kind: StatementKind::Assign {
                    target: result_local,
                    value: Rvalue::Call {
                        site_id: SiteId::from_raw(0),
                        kind: CallKind::Direct {
                            callee_fqn: "sample.helper".to_string(),
                        },
                        args: Vec::new(),
                    },
                },
            }],
            terminator: Terminator {
                span: Span::new(0, 0),
                kind: TerminatorKind::Perform {
                    site_id: SiteId::from_raw(0),
                    op_fqn: "scoop.core.Raise.raise".to_string(),
                    metadata: PerformMetadata {
                        effect_ty: builtins.unit,
                        payload_tuple_ty: None,
                        payload_component_tys: Vec::new(),
                        arg_mapping: Vec::new(),
                    },
                    args: Vec::new(),
                    resume_target: BasicBlockId(1),
                },
                unwind: UnwindAction::Propagate,
            },
        });
        let _bb1 = body.push_block(BasicBlock {
            is_cleanup: false,
            stmts: Vec::new(),
            terminator: Terminator {
                span: Span::new(0, 0),
                kind: TerminatorKind::Return { value: None },
                unwind: UnwindAction::NoUnwind,
            },
        });
        body.start = bb0;

        assert_eq!(
            body.validate_refactor_direct_style(),
            Err(MirValidationError::DuplicateSiteId {
                site_id: SiteId::from_raw(0),
                first_block: BasicBlockId(0),
                second_block: BasicBlockId(0),
            })
        );
    }

    #[test]
    fn dump_mir_keeps_generic_functions_as_templates_before_monomorphization() {
        let sess = Session::new().unwrap();
        let source = SourceFile::new_virtual(
            "<mem>/generic_template_boundary.scoop",
            r#"
package fixtures.mir

fun id<T>(x: T): T {
    return x
}

fun use<T>(x: T): T {
    return id(x)
}

fun entry(): Int {
    return use(1)
}
"#,
        );

        let lowered = lower_for_dump(&sess, &source).unwrap();
        let fun_fqns: Vec<&str> = lowered
            .file
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Fun(fun) => Some(fun.fqn.as_str()),
                Item::Todo { .. } => None,
            })
            .collect();

        assert!(fun_fqns.contains(&"fixtures.mir.id"));
        assert!(fun_fqns.contains(&"fixtures.mir.use"));
        assert!(fun_fqns.contains(&"fixtures.mir.entry"));
        assert!(fun_fqns.iter().all(|fqn| !fqn.contains("::<")));

        let use_fun = lowered
            .file
            .items
            .iter()
            .find_map(|item| match item {
                Item::Fun(fun) if fun.fqn == "fixtures.mir.use" => Some(fun),
                _ => None,
            })
            .expect("expected generic use function in MIR dump");
        assert!(matches!(
            lowered.types.kind(use_fun.params[0].ty),
            TypeKind::Param(_)
        ));
        assert!(matches!(
            lowered.types.kind(use_fun.return_ty),
            TypeKind::Param(_)
        ));

        let body = use_fun
            .body
            .as_ref()
            .expect("generic use function should have body");
        let call_kind = body
            .blocks
            .iter()
            .flat_map(|block| block.stmts.iter())
            .find_map(|stmt| match &stmt.kind {
                StatementKind::Assign {
                    value: Rvalue::Call { kind, .. },
                    ..
                } => Some(kind),
                _ => None,
            })
            .expect("expected direct call in generic use function body");
        match call_kind {
            CallKind::Direct { callee_fqn } => {
                assert_eq!(callee_fqn, "fixtures.mir.id");
            }
            other => panic!("expected direct generic-template call, got {other:?}"),
        }
    }
}
