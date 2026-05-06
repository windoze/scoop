use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::Path;

use crate::hir::{
    CallArg, CallSite, Expr, ExprKind, FunDecl, HandleArmKind, HirLowerError, HirStageError, Item,
    LoweredHir, Stmt, StmtKind, ValueRef,
};
use crate::session::Session;
use crate::source::SourceFile;
use crate::span::Span;
use crate::ty::{EffectRow, RefTypeKind, TypeId, TypeKind, TypeStore, ValueTypeKind};

use super::hir_completeness::RefactorHirCompletenessVerifier;

/// 单个 `Continuation.resume(...)` 调用点的 typed contract。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContinuationResumeSiteContract {
    receiver_ty: TypeId,
    resume_ty: TypeId,
    answer_ty: TypeId,
    return_ty: TypeId,
    out_effects: EffectRow,
    runtime_error_effect_ty: Option<TypeId>,
}

impl ContinuationResumeSiteContract {
    fn new(
        receiver_ty: TypeId,
        resume_ty: TypeId,
        answer_ty: TypeId,
        return_ty: TypeId,
        out_effects: EffectRow,
        runtime_error_effect_ty: Option<TypeId>,
    ) -> Self {
        Self {
            receiver_ty,
            resume_ty,
            answer_ty,
            return_ty,
            out_effects,
            runtime_error_effect_ty,
        }
    }

    pub fn receiver_ty(&self) -> TypeId {
        self.receiver_ty
    }

    pub fn resume_ty(&self) -> TypeId {
        self.resume_ty
    }

    pub fn answer_ty(&self) -> TypeId {
        self.answer_ty
    }

    pub fn return_ty(&self) -> TypeId {
        self.return_ty
    }

    pub fn out_effects(&self) -> &EffectRow {
        &self.out_effects
    }

    pub fn runtime_error_effect_ty(&self) -> Option<TypeId> {
        self.runtime_error_effect_ty
    }

    pub fn required_effects_include_runtime_error(&self) -> bool {
        self.runtime_error_effect_ty.is_some()
    }
}

/// 单个函数在 typed HIR stage 中对外暴露的 allowed-row / required-effects contract。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionEffectContract {
    span: Span,
    fqn: String,
    return_ty: TypeId,
    allowed_effects: EffectRow,
    effects_closed: bool,
}

impl FunctionEffectContract {
    fn new(
        span: Span,
        fqn: String,
        return_ty: TypeId,
        allowed_effects: EffectRow,
        effects_closed: bool,
    ) -> Self {
        Self {
            span,
            fqn,
            return_ty,
            allowed_effects,
            effects_closed,
        }
    }

    pub fn span(&self) -> Span {
        self.span
    }

    pub fn fqn(&self) -> &str {
        &self.fqn
    }

    pub fn return_ty(&self) -> TypeId {
        self.return_ty
    }

    pub fn allowed_effects(&self) -> &EffectRow {
        &self.allowed_effects
    }

    pub fn effects_closed(&self) -> bool {
        self.effects_closed
    }
}

/// `perform` / `handle` payload 的结构化 typed contract。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PayloadTypeContract {
    ty: Option<TypeId>,
    components: Vec<TypeId>,
}

impl PayloadTypeContract {
    fn new(ty: Option<TypeId>, components: Vec<TypeId>) -> Self {
        Self { ty, components }
    }

    pub fn ty(&self) -> Option<TypeId> {
        self.ty
    }

    pub fn components(&self) -> &[TypeId] {
        &self.components
    }

    fn display(&self, types: &TypeStore) -> String {
        if let Some(ty) = self.ty {
            return types.display(ty).to_string();
        }

        if self.components.is_empty() {
            return "<missing>".to_string();
        }

        let mut rendered = String::from("(");
        for (index, ty) in self.components.iter().enumerate() {
            if index > 0 {
                rendered.push_str(", ");
            }
            rendered.push_str(&types.display(*ty).to_string());
        }
        rendered.push(')');
        rendered
    }
}

/// 单个 `perform` 站点的 typed contract。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PerformSiteContract {
    effect_ty: TypeId,
    op_fqn: String,
    payload: PayloadTypeContract,
    arg_mapping: Vec<usize>,
}

impl PerformSiteContract {
    fn new(
        effect_ty: TypeId,
        op_fqn: String,
        payload: PayloadTypeContract,
        arg_mapping: Vec<usize>,
    ) -> Self {
        Self {
            effect_ty,
            op_fqn,
            payload,
            arg_mapping,
        }
    }

    pub fn effect_ty(&self) -> TypeId {
        self.effect_ty
    }

    pub fn op_fqn(&self) -> &str {
        &self.op_fqn
    }

    pub fn payload(&self) -> &PayloadTypeContract {
        &self.payload
    }

    pub fn arg_mapping(&self) -> &[usize] {
        &self.arg_mapping
    }
}

/// `handle` arm 的语义 kind 在 typed HIR contract 中的稳定枚举。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandleArmContractKind {
    NonResuming,
    EscapeContinuation,
}

/// 单个 `handle` arm 的 typed contract。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandleArmSiteContract {
    handled_effect_ty: TypeId,
    op_fqn: String,
    payload: PayloadTypeContract,
    body_ty: TypeId,
    kind: HandleArmContractKind,
}

impl HandleArmSiteContract {
    fn new(
        handled_effect_ty: TypeId,
        op_fqn: String,
        payload: PayloadTypeContract,
        body_ty: TypeId,
        kind: HandleArmContractKind,
    ) -> Self {
        Self {
            handled_effect_ty,
            op_fqn,
            payload,
            body_ty,
            kind,
        }
    }

    pub fn handled_effect_ty(&self) -> TypeId {
        self.handled_effect_ty
    }

    pub fn op_fqn(&self) -> &str {
        &self.op_fqn
    }

    pub fn payload(&self) -> &PayloadTypeContract {
        &self.payload
    }

    pub fn body_ty(&self) -> TypeId {
        self.body_ty
    }

    pub fn kind(&self) -> HandleArmContractKind {
        self.kind
    }
}

/// 单个 `handle { ... } with { ... }` 站点的 typed contract。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandleSiteContract {
    result_ty: TypeId,
    body_result_ty: TypeId,
    arm_contracts: Vec<HandleArmSiteContract>,
    finally_result_ty: Option<TypeId>,
}

impl HandleSiteContract {
    fn new(
        result_ty: TypeId,
        body_result_ty: TypeId,
        arm_contracts: Vec<HandleArmSiteContract>,
        finally_result_ty: Option<TypeId>,
    ) -> Self {
        Self {
            result_ty,
            body_result_ty,
            arm_contracts,
            finally_result_ty,
        }
    }

    pub fn result_ty(&self) -> TypeId {
        self.result_ty
    }

    pub fn body_result_ty(&self) -> TypeId {
        self.body_result_ty
    }

    pub fn arm_contracts(&self) -> &[HandleArmSiteContract] {
        &self.arm_contracts
    }

    pub fn finally_result_ty(&self) -> Option<TypeId> {
        self.finally_result_ty
    }
}

/// P2 typed HIR 已显式区分出的调用点 kind。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypedCallSiteKind {
    DirectCall,
    ContinuationResume,
    Perform,
}

/// refactor typed HIR stage 显式输出的 effect / continuation contract side tables。
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct TypedHirEffectContracts {
    function_effects: Vec<FunctionEffectContract>,
    continuation_resume_sites: HashMap<CallSite, ContinuationResumeSiteContract>,
    perform_sites: HashMap<CallSite, PerformSiteContract>,
    handle_sites: HashMap<CallSite, HandleSiteContract>,
    call_site_kinds: HashMap<CallSite, TypedCallSiteKind>,
}

impl TypedHirEffectContracts {
    fn from_lowered_hir(lowered_hir: &LoweredHir, source_path: &Path) -> Self {
        ContractCollector::new(lowered_hir).collect(source_path)
    }

    pub const fn is_placeholder(&self) -> bool {
        false
    }

    pub fn is_empty(&self) -> bool {
        self.function_effects.is_empty()
            && self.continuation_resume_sites.is_empty()
            && self.perform_sites.is_empty()
            && self.handle_sites.is_empty()
            && self.call_site_kinds.is_empty()
    }

    pub fn function_effects(&self) -> &[FunctionEffectContract] {
        &self.function_effects
    }

    pub fn continuation_resume_sites(&self) -> &HashMap<CallSite, ContinuationResumeSiteContract> {
        &self.continuation_resume_sites
    }

    pub fn continuation_resume_site(
        &self,
        call_site: &CallSite,
    ) -> Option<&ContinuationResumeSiteContract> {
        self.continuation_resume_sites.get(call_site)
    }

    pub fn perform_sites(&self) -> &HashMap<CallSite, PerformSiteContract> {
        &self.perform_sites
    }

    pub fn perform_site(&self, call_site: &CallSite) -> Option<&PerformSiteContract> {
        self.perform_sites.get(call_site)
    }

    pub fn handle_sites(&self) -> &HashMap<CallSite, HandleSiteContract> {
        &self.handle_sites
    }

    pub fn handle_site(&self, call_site: &CallSite) -> Option<&HandleSiteContract> {
        self.handle_sites.get(call_site)
    }

    pub fn call_site_kinds(&self) -> &HashMap<CallSite, TypedCallSiteKind> {
        &self.call_site_kinds
    }

    pub fn call_site_kind(&self, call_site: &CallSite) -> Option<TypedCallSiteKind> {
        self.call_site_kinds.get(call_site).copied()
    }

    /// 以稳定顺序渲染 typed HIR side tables，供 `dump-hir` 与 snapshot tests 使用。
    pub fn stable_dump(&self, types: &TypeStore) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "TypedHirEffectContracts {{");

        let _ = writeln!(out, "    function_effects: [");
        for contract in &self.function_effects {
            let _ = writeln!(out, "        FunctionEffectContract {{");
            let _ = writeln!(out, "            span: {:?},", contract.span());
            let _ = writeln!(out, "            fqn: {:?},", contract.fqn());
            let _ = writeln!(
                out,
                "            return_ty: {},",
                types.display(contract.return_ty())
            );
            let _ = writeln!(
                out,
                "            allowed_effects: {},",
                format_effect_row(types, contract.allowed_effects())
            );
            let _ = writeln!(
                out,
                "            effects_closed: {},",
                contract.effects_closed()
            );
            let _ = writeln!(out, "        }},");
        }
        let _ = writeln!(out, "    ],");

        let mut call_site_kinds = self.call_site_kinds.iter().collect::<Vec<_>>();
        call_site_kinds.sort_by(|(lhs, _), (rhs, _)| compare_call_sites(lhs, rhs));
        let _ = writeln!(out, "    call_site_kinds: [");
        for (call_site, kind) in call_site_kinds {
            let _ = writeln!(out, "        TypedCallSiteContract {{");
            let _ = writeln!(out, "            span: {:?},", call_site.span);
            let _ = writeln!(out, "            kind: {:?},", kind);
            let _ = writeln!(out, "        }},");
        }
        let _ = writeln!(out, "    ],");

        let mut continuation_resume_sites =
            self.continuation_resume_sites.iter().collect::<Vec<_>>();
        continuation_resume_sites.sort_by(|(lhs, _), (rhs, _)| compare_call_sites(lhs, rhs));
        let _ = writeln!(out, "    continuation_resume_sites: [");
        for (call_site, contract) in continuation_resume_sites {
            let _ = writeln!(out, "        ContinuationResumeSiteContract {{");
            let _ = writeln!(out, "            span: {:?},", call_site.span);
            let _ = writeln!(
                out,
                "            receiver_ty: {},",
                types.display(contract.receiver_ty())
            );
            let _ = writeln!(
                out,
                "            resume_ty: {},",
                types.display(contract.resume_ty())
            );
            let _ = writeln!(
                out,
                "            answer_ty: {},",
                types.display(contract.answer_ty())
            );
            let _ = writeln!(
                out,
                "            return_ty: {},",
                types.display(contract.return_ty())
            );
            let _ = writeln!(
                out,
                "            out_effects: {},",
                format_effect_row(types, contract.out_effects())
            );
            let _ = writeln!(
                out,
                "            required_effects: {},",
                format_required_effects(
                    types,
                    contract.out_effects(),
                    contract.runtime_error_effect_ty(),
                )
            );
            let _ = writeln!(
                out,
                "            includes_runtime_error_effect: {},",
                contract.required_effects_include_runtime_error()
            );
            let _ = writeln!(out, "        }},");
        }
        let _ = writeln!(out, "    ],");

        let mut perform_sites = self.perform_sites.iter().collect::<Vec<_>>();
        perform_sites.sort_by(|(lhs, _), (rhs, _)| compare_call_sites(lhs, rhs));
        let _ = writeln!(out, "    perform_sites: [");
        for (call_site, contract) in perform_sites {
            let _ = writeln!(out, "        PerformSiteContract {{");
            let _ = writeln!(out, "            span: {:?},", call_site.span);
            let _ = writeln!(
                out,
                "            effect_ty: {},",
                types.display(contract.effect_ty())
            );
            let _ = writeln!(out, "            op_fqn: {:?},", contract.op_fqn());
            let _ = writeln!(
                out,
                "            payload_ty: {},",
                contract.payload().display(types)
            );
            let _ = writeln!(out, "            payload_components: [");
            for ty in contract.payload().components() {
                let _ = writeln!(out, "                {},", types.display(*ty));
            }
            let _ = writeln!(out, "            ],");
            let _ = writeln!(
                out,
                "            arg_mapping: {:?},",
                contract.arg_mapping()
            );
            let _ = writeln!(out, "        }},");
        }
        let _ = writeln!(out, "    ],");

        let mut handle_sites = self.handle_sites.iter().collect::<Vec<_>>();
        handle_sites.sort_by(|(lhs, _), (rhs, _)| compare_call_sites(lhs, rhs));
        let _ = writeln!(out, "    handle_sites: [");
        for (call_site, contract) in handle_sites {
            let _ = writeln!(out, "        HandleSiteContract {{");
            let _ = writeln!(out, "            span: {:?},", call_site.span);
            let _ = writeln!(
                out,
                "            result_ty: {},",
                types.display(contract.result_ty())
            );
            let _ = writeln!(
                out,
                "            body_result_ty: {},",
                types.display(contract.body_result_ty())
            );
            let _ = writeln!(out, "            arm_contracts: [");
            for arm in contract.arm_contracts() {
                let _ = writeln!(out, "                HandleArmSiteContract {{");
                let _ = writeln!(out, "                    op_fqn: {:?},", arm.op_fqn());
                let _ = writeln!(
                    out,
                    "                    handled_effect_ty: {},",
                    types.display(arm.handled_effect_ty())
                );
                let _ = writeln!(
                    out,
                    "                    payload_ty: {},",
                    arm.payload().display(types)
                );
                let _ = writeln!(out, "                    payload_components: [");
                for ty in arm.payload().components() {
                    let _ = writeln!(out, "                        {},", types.display(*ty));
                }
                let _ = writeln!(out, "                    ],");
                let _ = writeln!(
                    out,
                    "                    body_ty: {},",
                    types.display(arm.body_ty())
                );
                let _ = writeln!(out, "                    kind: {:?},", arm.kind());
                let _ = writeln!(out, "                }},");
            }
            let _ = writeln!(out, "            ],");
            match contract.finally_result_ty() {
                Some(finally_ty) => {
                    let _ = writeln!(
                        out,
                        "            finally_result_ty: Some({}),",
                        types.display(finally_ty)
                    );
                }
                None => {
                    let _ = writeln!(out, "            finally_result_ty: None,");
                }
            }
            let _ = writeln!(out, "        }},");
        }
        let _ = writeln!(out, "    ],");

        let _ = write!(out, "}}");
        out
    }
}

/// refactor typed HIR stage 的稳定输出形状。
///
/// 本阶段固定如下 invariants，供 P2/P3 及后续阶段直接消费：
/// - 输出已经过 resolver + typecheck，可直接视为 typed HIR handoff；
/// - `Continuation` / `resume` / `perform` / `handle` 的 typed contract 应在此阶段显式化，
///   下游不应再回 AST 猜测 surface 语义；
/// - `dump-hir` 的 refactor 路径必须优先消费这一 stage 输出，而不是 legacy
///   `hir::lower_for_dump(...)`；
/// - `effect_contracts` 现在显式输出函数级 allowed-row contract，以及 `Continuation.resume(...)` /
///   `perform` / `handle` 的结构化 typed contract，固定 `ResumeTuple` / `Answer` / `Out`、
///   runtime error ordinary effect 贡献、performed effect/payload、以及 handler arm typed 关系，
///   供后续阶段直接消费。
#[derive(Debug)]
pub struct TypedHirStageOutput {
    lowered_hir: LoweredHir,
    effect_contracts: TypedHirEffectContracts,
}

impl TypedHirStageOutput {
    pub(crate) fn new(lowered_hir: LoweredHir, source_path: &Path) -> Result<Self, HirStageError> {
        RefactorHirCompletenessVerifier::new(&lowered_hir, source_path).verify()?;
        Ok(Self::new_unchecked(lowered_hir, source_path))
    }

    pub(crate) fn new_unchecked(lowered_hir: LoweredHir, source_path: &Path) -> Self {
        let effect_contracts = TypedHirEffectContracts::from_lowered_hir(&lowered_hir, source_path);
        Self {
            lowered_hir,
            effect_contracts,
        }
    }

    pub fn hir_file(&self) -> &crate::hir::File {
        &self.lowered_hir.file
    }

    pub fn types(&self) -> &TypeStore {
        &self.lowered_hir.types
    }

    pub fn lowered_hir(&self) -> &LoweredHir {
        &self.lowered_hir
    }

    pub fn effect_contracts(&self) -> &TypedHirEffectContracts {
        &self.effect_contracts
    }

    /// 以稳定文本渲染 refactor typed HIR dump：先打印 HIR `File`，再追加 typed side tables。
    pub fn stable_dump(&self) -> String {
        let mut out = format!("{:#?}\n", self.hir_file());
        out.push('\n');
        out.push_str(&self.effect_contracts.stable_dump(self.types()));
        out.push('\n');
        out
    }

    pub fn into_lowered_hir(self) -> LoweredHir {
        self.lowered_hir
    }
}

pub(crate) fn run(
    session: &Session,
    source: &SourceFile,
) -> Result<TypedHirStageOutput, HirLowerError> {
    let lowered_hir = crate::hir::lower_typed_for_dump(session, source)?;
    TypedHirStageOutput::new(lowered_hir, source.path()).map_err(HirLowerError::from)
}

struct ContractCollector<'a> {
    lowered_hir: &'a LoweredHir,
    runtime_error_effect_ty: Option<TypeId>,
    function_effects: Vec<FunctionEffectContract>,
    continuation_resume_sites: HashMap<CallSite, ContinuationResumeSiteContract>,
    perform_sites: HashMap<CallSite, PerformSiteContract>,
    handle_sites: HashMap<CallSite, HandleSiteContract>,
    call_site_kinds: HashMap<CallSite, TypedCallSiteKind>,
}

impl<'a> ContractCollector<'a> {
    fn new(lowered_hir: &'a LoweredHir) -> Self {
        Self {
            lowered_hir,
            runtime_error_effect_ty: find_raise_runtime_error_effect(&lowered_hir.types),
            function_effects: Vec::new(),
            continuation_resume_sites: HashMap::new(),
            perform_sites: HashMap::new(),
            handle_sites: HashMap::new(),
            call_site_kinds: HashMap::new(),
        }
    }

    fn collect(mut self, source_path: &Path) -> TypedHirEffectContracts {
        for item in &self.lowered_hir.file.items {
            self.collect_item(source_path, item);
        }

        for member_fun in &self.lowered_hir.member_funs {
            self.record_function_effect_contract(member_fun);
            self.collect_fun(member_fun);
        }

        self.function_effects
            .sort_by(compare_function_effect_contracts);
        TypedHirEffectContracts {
            function_effects: self.function_effects,
            continuation_resume_sites: self.continuation_resume_sites,
            perform_sites: self.perform_sites,
            handle_sites: self.handle_sites,
            call_site_kinds: self.call_site_kinds,
        }
    }

    fn collect_item(&mut self, source_path: &Path, item: &Item) {
        match item {
            Item::Fun(fun) => {
                self.record_function_effect_contract(fun);
                self.collect_fun(fun);
            }
            Item::Val(val) => {
                if let Some(init) = &val.init {
                    self.collect_expr(source_path, init);
                }
            }
            Item::Todo { .. } => {}
        }
    }

    fn record_function_effect_contract(&mut self, fun: &FunDecl) {
        let Some((allowed_effects, effects_closed)) =
            function_effect_contract(&self.lowered_hir.types, fun.ty)
        else {
            return;
        };

        self.function_effects.push(FunctionEffectContract::new(
            fun.span,
            fun.fqn.clone(),
            fun.return_ty,
            allowed_effects,
            effects_closed,
        ));
    }

    fn collect_fun(&mut self, fun: &FunDecl) {
        if let Some(body) = &fun.body {
            self.collect_block(&fun.source_path, body);
        }
    }

    fn collect_block(&mut self, source_path: &Path, block: &crate::hir::Block) {
        for stmt in &block.stmts {
            self.collect_stmt(source_path, stmt);
        }
    }

    fn collect_stmt(&mut self, source_path: &Path, stmt: &Stmt) {
        match &stmt.kind {
            StmtKind::Empty
            | StmtKind::Break { .. }
            | StmtKind::Continue { .. }
            | StmtKind::Todo(_) => {}
            StmtKind::Expr(expr) => self.collect_expr(source_path, expr),
            StmtKind::Val(val) => {
                if let Some(init) = &val.init {
                    self.collect_expr(source_path, init);
                }
            }
            StmtKind::Assign { lhs, rhs, .. } => {
                self.collect_expr(source_path, lhs);
                self.collect_expr(source_path, rhs);
            }
            StmtKind::While { cond, body } => {
                self.collect_expr(source_path, cond);
                self.collect_block(source_path, body);
            }
            StmtKind::Return { value } => {
                if let Some(value) = value {
                    self.collect_expr(source_path, value);
                }
            }
        }
    }

    fn collect_expr(&mut self, source_path: &Path, expr: &Expr) {
        match &expr.kind {
            ExprKind::Missing
            | ExprKind::Literal(_)
            | ExprKind::VarRef(_)
            | ExprKind::UnresolvedIdent { .. }
            | ExprKind::Todo(_) => {}
            ExprKind::StructLit { fields, .. } => {
                for field in fields {
                    self.collect_expr(source_path, &field.value);
                }
            }
            ExprKind::TupleLit { elements } => {
                for element in elements {
                    self.collect_expr(source_path, element);
                }
            }
            ExprKind::InterpolatedString { parts, .. } => {
                for part in parts {
                    if let crate::hir::InterpolatedStringPart::Expr { expr } = part {
                        self.collect_expr(source_path, expr);
                    }
                }
            }
            ExprKind::Unary { expr, .. }
            | ExprKind::TypeCheck { expr, .. }
            | ExprKind::Cast { expr, .. }
            | ExprKind::MemberAccess { receiver: expr, .. } => {
                self.collect_expr(source_path, expr);
            }
            ExprKind::Binary { lhs, rhs, .. } => {
                self.collect_expr(source_path, lhs);
                self.collect_expr(source_path, rhs);
            }
            ExprKind::Block(block) => self.collect_block(source_path, block),
            ExprKind::Closure(closure) => self.collect_expr(source_path, &closure.body),
            ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                self.collect_expr(source_path, cond);
                self.collect_expr(source_path, then_branch);
                if let Some(else_branch) = else_branch {
                    self.collect_expr(source_path, else_branch);
                }
            }
            ExprKind::When { subject, arms } => {
                self.collect_expr(source_path, subject);
                for arm in arms {
                    if let Some(guard) = &arm.guard {
                        self.collect_expr(source_path, guard);
                    }
                    self.collect_expr(source_path, &arm.body);
                }
            }
            ExprKind::Call { callee, args } => {
                self.record_call_contract(source_path, expr, callee, args);
                self.collect_expr(source_path, callee);
                for arg in args {
                    self.collect_call_arg_expr(source_path, arg);
                }
            }
            ExprKind::Perform {
                effect_ty,
                op,
                args,
            } => {
                self.record_perform_contract(source_path, expr, *effect_ty, op, args);
                for arg in args {
                    self.collect_call_arg_expr(source_path, arg);
                }
            }
            ExprKind::Handle(handle) => {
                self.record_handle_contract(source_path, expr, handle);
                self.collect_block(source_path, &handle.body);
                for arm in &handle.arms {
                    self.collect_expr(source_path, &arm.body);
                }
                if let Some(finally) = &handle.finally {
                    self.collect_block(source_path, finally);
                }
            }
        }
    }

    fn collect_call_arg_expr(&mut self, source_path: &Path, arg: &CallArg) {
        match arg {
            CallArg::Positional(expr) => self.collect_expr(source_path, expr),
            CallArg::Named { value, .. } => self.collect_expr(source_path, value),
        }
    }

    fn record_call_contract(
        &mut self,
        source_path: &Path,
        expr: &Expr,
        callee: &Expr,
        args: &[CallArg],
    ) {
        let call_site = self.call_site(source_path, expr.span);
        if let Some(contract) = self.continuation_resume_contract(expr, callee, args) {
            self.continuation_resume_sites
                .insert(call_site.clone(), contract);
            self.call_site_kinds
                .insert(call_site, TypedCallSiteKind::ContinuationResume);
            return;
        }

        self.call_site_kinds
            .insert(call_site, TypedCallSiteKind::DirectCall);
    }

    fn record_perform_contract(
        &mut self,
        source_path: &Path,
        expr: &Expr,
        effect_ty: TypeId,
        op: &crate::hir::EffectOpRef,
        args: &[CallArg],
    ) {
        let call_site = self.call_site(source_path, expr.span);
        let info = self.lowered_hir.effect_op_call_sites.get(&call_site);
        let arg_mapping = info
            .map(|binding| binding.arg_mapping.clone())
            .unwrap_or_else(|| (0..args.len()).collect());
        let payload_components = arg_mapping
            .iter()
            .filter_map(|&arg_idx| args.get(arg_idx).map(call_arg_value_ty))
            .collect::<Vec<_>>();
        let payload_ty = match payload_components.as_slice() {
            [] => Some(self.lowered_hir.builtins.unit),
            [single] => Some(*single),
            _ => info.and_then(|binding| binding.payload_tuple_ty),
        };

        self.perform_sites.insert(
            call_site.clone(),
            PerformSiteContract::new(
                effect_ty,
                op.fqn.clone(),
                PayloadTypeContract::new(payload_ty, payload_components),
                arg_mapping,
            ),
        );
        self.call_site_kinds
            .insert(call_site, TypedCallSiteKind::Perform);
    }

    fn record_handle_contract(
        &mut self,
        source_path: &Path,
        expr: &Expr,
        handle: &crate::hir::HandleExpr,
    ) {
        let arm_contracts = handle
            .arms
            .iter()
            .map(|arm| {
                let payload_components = arm
                    .op
                    .binders
                    .iter()
                    .map(|binder| binder.ty)
                    .collect::<Vec<_>>();
                let payload_ty = match payload_components.as_slice() {
                    [] => Some(self.lowered_hir.builtins.unit),
                    [single] => Some(*single),
                    _ => self
                        .lowered_hir
                        .handle_payload_tuple_tys
                        .get(&self.call_site(source_path, arm.op.span))
                        .copied(),
                };
                let kind = match arm.kind {
                    HandleArmKind::NonResuming => HandleArmContractKind::NonResuming,
                    HandleArmKind::EscapeContinuation { .. } => {
                        HandleArmContractKind::EscapeContinuation
                    }
                };

                HandleArmSiteContract::new(
                    arm.op.effect_ty,
                    arm.op.op.fqn.clone(),
                    PayloadTypeContract::new(payload_ty, payload_components),
                    arm.body.ty,
                    kind,
                )
            })
            .collect::<Vec<_>>();

        self.handle_sites.insert(
            self.call_site(source_path, expr.span),
            HandleSiteContract::new(
                expr.ty,
                handle.body.ty,
                arm_contracts,
                handle.finally.as_ref().map(|finally| finally.ty),
            ),
        );
    }

    fn continuation_resume_contract(
        &self,
        expr: &Expr,
        callee: &Expr,
        args: &[CallArg],
    ) -> Option<ContinuationResumeSiteContract> {
        let ExprKind::VarRef(ValueRef::TopLevel { fqn, .. }) = &callee.kind else {
            return None;
        };
        if fqn != "scoop.core.Continuation.resume" {
            return None;
        }

        let Some(CallArg::Positional(receiver)) = args.first() else {
            return None;
        };
        let (resume_ty, answer_ty, out_effects) =
            continuation_receiver_contract(&self.lowered_hir.types, receiver.ty)?;

        Some(ContinuationResumeSiteContract::new(
            receiver.ty,
            resume_ty,
            answer_ty,
            expr.ty,
            out_effects,
            self.runtime_error_effect_ty,
        ))
    }

    fn call_site(&self, source_path: &Path, span: Span) -> CallSite {
        CallSite::new(source_path.to_path_buf(), span)
    }
}

fn call_arg_value_ty(arg: &CallArg) -> TypeId {
    match arg {
        CallArg::Positional(expr) => expr.ty,
        CallArg::Named { value, .. } => value.ty,
    }
}

fn function_effect_contract(types: &TypeStore, fun_ty: TypeId) -> Option<(EffectRow, bool)> {
    let TypeKind::Ref(RefTypeKind::Function(function)) = types.kind(fun_ty) else {
        return None;
    };

    Some((function.effects.clone(), function.effects_closed))
}

fn find_raise_runtime_error_effect(types: &TypeStore) -> Option<TypeId> {
    let runtime_error_ty = find_nominal_type_by_fqn(types, "scoop.core.RuntimeError")?;

    types.iter_ids().find(|&id| {
        matches!(
            types.kind(id),
            TypeKind::Ref(RefTypeKind::Nominal(nominal))
                if nominal.fqn == "scoop.core.Raise"
                    && nominal.args.as_slice() == [runtime_error_ty]
        )
    })
}

fn find_nominal_type_by_fqn(types: &TypeStore, fqn: &str) -> Option<TypeId> {
    types.iter_ids().find(|&id| {
        matches!(
            types.kind(id),
            TypeKind::Ref(RefTypeKind::Nominal(nominal)) if nominal.fqn == fqn
        ) || matches!(
            types.kind(id),
            TypeKind::Value(ValueTypeKind::Nominal(nominal)) if nominal.fqn == fqn
        )
    })
}

fn continuation_receiver_contract(
    types: &TypeStore,
    receiver_ty: TypeId,
) -> Option<(TypeId, TypeId, EffectRow)> {
    let TypeKind::Ref(RefTypeKind::Nominal(nominal)) = types.kind(receiver_ty) else {
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

fn compare_call_sites(lhs: &CallSite, rhs: &CallSite) -> Ordering {
    lhs.source_path
        .cmp(&rhs.source_path)
        .then(lhs.span.start.cmp(&rhs.span.start))
        .then(lhs.span.end.cmp(&rhs.span.end))
}

fn compare_function_effect_contracts(
    lhs: &FunctionEffectContract,
    rhs: &FunctionEffectContract,
) -> Ordering {
    lhs.fqn()
        .cmp(rhs.fqn())
        .then(lhs.span().start.cmp(&rhs.span().start))
        .then(lhs.span().end.cmp(&rhs.span().end))
}

fn format_effect_row(types: &TypeStore, row: &EffectRow) -> String {
    if row.is_pure() {
        return "Pure".to_string();
    }

    row.terms
        .iter()
        .map(|ty| types.display(*ty).to_string())
        .collect::<Vec<_>>()
        .join(" + ")
}

fn format_required_effects(
    types: &TypeStore,
    out_effects: &EffectRow,
    runtime_error_effect_ty: Option<TypeId>,
) -> String {
    let mut terms = out_effects.terms.clone();
    if let Some(runtime_error_effect_ty) = runtime_error_effect_ty
        && !terms.contains(&runtime_error_effect_ty)
    {
        terms.push(runtime_error_effect_ty);
    }
    format_effect_row(types, &EffectRow::new(terms))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::path::PathBuf;

    use crate::session::{EffectPipelineMode, SessionOptions};

    fn refactor_session() -> Session {
        Session::with_options(SessionOptions::new(EffectPipelineMode::Refactor)).unwrap()
    }

    fn load_hir_fixture(name: &str) -> SourceFile {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/hir")
            .join(name);
        SourceFile::load(&path).expect("fixture 应可加载")
    }

    fn clean_lowered_hir() -> (LoweredHir, PathBuf) {
        let session = refactor_session();
        let source = SourceFile::new_virtual(
            "<mem>/refactor_hir_no_todo_clean.scoop",
            "package sample\nfun main() {}\n",
        );
        let source_path = source.path().to_path_buf();
        let lowered = crate::hir::lower_typed_for_dump(&session, &source).unwrap();
        (lowered, source_path)
    }

    fn stage_error_for(lowered: LoweredHir, source_path: &std::path::Path) -> HirStageError {
        TypedHirStageOutput::new(lowered, source_path)
            .expect_err("refactor HIR completeness verifier 应拒绝 placeholder")
    }

    fn test_span() -> Span {
        Span::new(21, 22)
    }

    fn expr_with_kind(lowered: &LoweredHir, kind: ExprKind) -> Expr {
        Expr {
            span: test_span(),
            ty: lowered.builtins.unit,
            kind,
        }
    }

    fn stmt_with_kind(lowered: &LoweredHir, kind: StmtKind) -> Stmt {
        Stmt {
            span: test_span(),
            ty: lowered.builtins.unit,
            kind,
        }
    }

    fn replace_main_body_with_stmt(lowered: &mut LoweredHir, stmt: Stmt) {
        let fun = lowered
            .file
            .items
            .iter_mut()
            .find_map(|item| match item {
                Item::Fun(fun) if fun.fqn == "sample.main" => Some(fun),
                _ => None,
            })
            .expect("clean fixture 应包含 sample.main");
        fun.body = Some(crate::hir::Block {
            span: test_span(),
            ty: lowered.builtins.unit,
            stmts: vec![stmt],
        });
    }

    fn main_fun_clone(lowered: &LoweredHir) -> FunDecl {
        lowered
            .file
            .items
            .iter()
            .find_map(|item| match item {
                Item::Fun(fun) if fun.fqn == "sample.main" => Some(fun.clone()),
                _ => None,
            })
            .expect("clean fixture 应包含 sample.main")
    }

    #[test]
    fn refactor_hir_no_todo_rejects_current_placeholder_reasons() {
        const EXPR_REASONS: &[&str] = &[
            "array_lit",
            "class_lit",
            "spread_arg",
            "named_arg",
            "structured_concurrency_spawn_deferred",
            "structured_concurrency_join_deferred",
            "splice_field",
            "assign",
            "with_update",
        ];
        for reason in EXPR_REASONS {
            let (mut lowered, source_path) = clean_lowered_hir();
            let stmt = stmt_with_kind(
                &lowered,
                StmtKind::Expr(expr_with_kind(&lowered, ExprKind::Todo(reason))),
            );
            replace_main_body_with_stmt(&mut lowered, stmt);

            let err = stage_error_for(lowered, &source_path);
            let expected = format!("ExprKind::Todo({reason})");
            assert_eq!(err.reason(), expected);
            assert_eq!(err.owner(), "fun sample.main");
            assert_eq!(err.span(), test_span());
            assert_eq!(err.source_path(), source_path.as_path());
        }

        const STMT_REASONS: &[&str] = &[
            "missing_stmt",
            "comptime_block",
            "comptime_if",
            "comptime_for",
            "for_custom_iterator",
        ];
        for reason in STMT_REASONS {
            let (mut lowered, source_path) = clean_lowered_hir();
            let stmt = stmt_with_kind(&lowered, StmtKind::Todo(reason));
            replace_main_body_with_stmt(&mut lowered, stmt);

            let err = stage_error_for(lowered, &source_path);
            let expected = format!("StmtKind::Todo({reason})");
            assert_eq!(err.reason(), expected);
            assert_eq!(err.owner(), "fun sample.main");
            assert_eq!(err.span(), test_span());
            assert_eq!(err.source_path(), source_path.as_path());
        }

        const ITEM_REASONS: &[&str] = &[
            "typealias",
            "comptime_if_item",
            "type",
            "object",
            "extension_property_no_getter",
        ];
        for reason in ITEM_REASONS {
            let (mut lowered, source_path) = clean_lowered_hir();
            lowered.file.items = vec![Item::Todo {
                span: test_span(),
                kind: reason,
            }];

            let err = stage_error_for(lowered, &source_path);
            let expected = format!("Item::Todo({reason})");
            assert_eq!(err.reason(), expected);
            assert_eq!(err.owner(), "top-level item");
            assert_eq!(err.span(), test_span());
            assert_eq!(err.source_path(), source_path.as_path());
        }

        let (mut lowered, source_path) = clean_lowered_hir();
        let stmt = stmt_with_kind(
            &lowered,
            StmtKind::Expr(expr_with_kind(&lowered, ExprKind::Missing)),
        );
        replace_main_body_with_stmt(&mut lowered, stmt);

        let err = stage_error_for(lowered, &source_path);
        assert_eq!(err.reason(), "ExprKind::Missing");
        assert_eq!(err.owner(), "fun sample.main");
        assert_eq!(err.span(), test_span());
        assert_eq!(err.source_path(), source_path.as_path());
    }

    #[test]
    fn refactor_hir_no_todo_scans_member_fun_and_init_roots() {
        let (mut lowered, source_path) = clean_lowered_hir();
        let mut member_fun = main_fun_clone(&lowered);
        member_fun.fqn = "sample.Box.member".to_string();
        member_fun.name = "member".to_string();
        let stmt = stmt_with_kind(
            &lowered,
            StmtKind::Expr(expr_with_kind(&lowered, ExprKind::Todo("array_lit"))),
        );
        member_fun.body = Some(crate::hir::Block {
            span: test_span(),
            ty: lowered.builtins.unit,
            stmts: vec![stmt],
        });
        lowered.member_funs.push(member_fun);

        let err = stage_error_for(lowered, &source_path);
        assert_eq!(err.reason(), "ExprKind::Todo(array_lit)");
        assert_eq!(err.owner(), "member fun sample.Box.member");

        let (mut lowered, source_path) = clean_lowered_hir();
        lowered.top_level_vars.insert(
            "sample.global".to_string(),
            crate::hir::TopLevelVar {
                fqn: "sample.global".to_string(),
                source_path: source_path.clone(),
                span: test_span(),
                storage: crate::hir::TopLevelVarStorage::Global,
                ty: lowered.builtins.unit,
                init: Some(expr_with_kind(&lowered, ExprKind::Todo("array_lit"))),
            },
        );

        let err = stage_error_for(lowered, &source_path);
        assert_eq!(err.reason(), "ExprKind::Todo(array_lit)");
        assert_eq!(err.owner(), "top-level var sample.global");

        let (mut lowered, source_path) = clean_lowered_hir();
        lowered.object_inits.insert(
            "sample.Singleton".to_string(),
            crate::hir::ObjectInit {
                fqn: "sample.Singleton".to_string(),
                source_path: source_path.clone(),
                properties: HashMap::new(),
                steps: vec![crate::hir::ObjectInitStep::PropertyInit {
                    name: "x".to_string(),
                    init: expr_with_kind(&lowered, ExprKind::Todo("array_lit")),
                }],
            },
        );

        let err = stage_error_for(lowered, &source_path);
        assert_eq!(err.reason(), "ExprKind::Todo(array_lit)");
        assert_eq!(err.owner(), "object sample.Singleton");

        let (mut lowered, source_path) = clean_lowered_hir();
        lowered.class_inits.insert(
            "sample.Box".to_string(),
            crate::hir::ClassInit {
                fqn: "sample.Box".to_string(),
                source_path: source_path.clone(),
                super_class_fqn: None,
                super_ctor_args_span: None,
                super_ctor_call: None,
                super_ctor_args: Vec::new(),
                this_id: crate::hir::SymbolId::from_raw(1),
                fields: Vec::new(),
                field_indices: HashMap::new(),
                steps: vec![crate::hir::ClassInitStep::PropertyInit {
                    field_fqn: "sample.Box.x".to_string(),
                    init: expr_with_kind(&lowered, ExprKind::Todo("array_lit")),
                }],
                ctors: Vec::new(),
            },
        );

        let err = stage_error_for(lowered, &source_path);
        assert_eq!(err.reason(), "ExprKind::Todo(array_lit)");
        assert_eq!(err.owner(), "class sample.Box");
    }

    #[test]
    fn refactor_hir_no_todo_stage_reports_source_diagnostic_for_real_input() {
        let session = refactor_session();
        let source = SourceFile::new_virtual(
            "<mem>/typealias_placeholder.scoop",
            "package sample\ntypealias Alias = Int\nfun main() {}\n",
        );

        let err = run(&session, &source).expect_err("typealias placeholder 应被 HIR stage 拒绝");
        let HirLowerError::Stage(stage_error) = err else {
            panic!("应返回结构化 HirStageError")
        };

        assert_eq!(stage_error.reason(), "Item::Todo(typealias)");
        assert_eq!(stage_error.owner(), "top-level item");
        assert_eq!(stage_error.source_path(), source.path());
        assert!(!stage_error.span().is_empty());
    }

    fn assert_fixture_effect_contract_dump(name: &str, expected: &str) {
        let session = refactor_session();
        let source = load_hir_fixture(name);
        let output = run(&session, &source).expect("fixture 应能通过 refactor typed HIR stage");

        assert_eq!(
            output.effect_contracts().stable_dump(output.types()),
            expected
        );
    }

    #[test]
    fn refactor_typed_hir_stage_output_is_constructible() {
        let session = refactor_session();
        let source = SourceFile::new_virtual("<mem>", "package sample\nfun main() {}\n");

        let output = run(&session, &source).unwrap();

        assert_eq!(output.hir_file().items.len(), 1);
        assert!(!output.effect_contracts().is_placeholder());
        assert_eq!(output.effect_contracts().function_effects().len(), 1);
        assert!(
            output
                .effect_contracts()
                .continuation_resume_sites()
                .is_empty()
        );
        assert!(output.effect_contracts().perform_sites().is_empty());
        assert!(output.effect_contracts().handle_sites().is_empty());
        assert!(output.effect_contracts().call_site_kinds().is_empty());
    }

    #[test]
    fn refactor_typed_hir_stage_builds_explicit_contract_tables() {
        let session = refactor_session();
        let source = SourceFile::new_virtual("<mem>", "package sample\nfun main() {}\n");

        let output = run(&session, &source).unwrap();

        assert!(!output.types().is_empty());
        assert!(!output.effect_contracts().is_placeholder());
        assert_eq!(
            output.effect_contracts().function_effects()[0].fqn(),
            "sample.main"
        );
        assert!(output.stable_dump().contains("TypedHirEffectContracts"));
    }

    #[test]
    fn refactor_typed_hir_records_resume_contracts_in_typed_hir_stage() {
        let session = refactor_session();
        let source = SourceFile::new_virtual(
            "<mem>/refactor_continuation_contracts.scoop",
            r#"
package fixtures.hirstage

import scoop.core.*

fun resumeWithEffects(k: Continuation<Int, Int, eff Raise<Int>>): Int / (Raise<Int> + Raise<RuntimeError>) {
    return k.resume(1)
}
"#,
        );

        let output = run(&session, &source).unwrap();
        let contracts = output.effect_contracts();

        assert_eq!(contracts.continuation_resume_sites().len(), 1);
        let (call_site, contract) = contracts
            .continuation_resume_sites()
            .iter()
            .next()
            .expect("应收集到唯一的 continuation resume contract");

        assert_eq!(call_site.source_path, source.path());
        assert_eq!(
            contracts.call_site_kind(call_site),
            Some(TypedCallSiteKind::ContinuationResume)
        );
        assert_eq!(
            output.types().display(contract.receiver_ty()).to_string(),
            "scoop.core.Continuation<Int, Int, eff scoop.core.Raise<Int>>"
        );
        assert_eq!(
            output.types().display(contract.resume_ty()).to_string(),
            "Int"
        );
        assert_eq!(
            output.types().display(contract.answer_ty()).to_string(),
            "Int"
        );
        assert_eq!(
            output.types().display(contract.return_ty()).to_string(),
            "Int"
        );
        assert_eq!(contract.out_effects().terms.len(), 1);
        assert_eq!(
            output
                .types()
                .display(contract.out_effects().terms[0])
                .to_string(),
            "scoop.core.Raise<Int>"
        );
        assert_eq!(
            output
                .types()
                .display(contract.runtime_error_effect_ty().unwrap())
                .to_string(),
            "scoop.core.Raise<scoop.core.RuntimeError>"
        );
        assert!(contract.required_effects_include_runtime_error());
    }

    #[test]
    fn refactor_typed_hir_continuation_contract_dump_snapshot() {
        assert_fixture_effect_contract_dump(
            "continuation_resume_surface_named_tuple_and_unit_basic.scoop",
            r#"TypedHirEffectContracts {
    function_effects: [
        FunctionEffectContract {
            span: 233..351,
            fqn: "fixtures.hir.resumePair",
            return_ty: Unit,
            allowed_effects: scoop.core.Raise<scoop.core.RuntimeError>,
            effects_closed: false,
        },
        FunctionEffectContract {
            span: 80..231,
            fqn: "fixtures.hir.resumeUnit",
            return_ty: Unit,
            allowed_effects: scoop.core.Raise<scoop.core.RuntimeError>,
            effects_closed: false,
        },
        FunctionEffectContract {
            span: 43..78,
            fqn: "fixtures.hir.takesUnit",
            return_ty: Unit,
            allowed_effects: Pure,
            effects_closed: false,
        },
    ],
    call_site_kinds: [
        TypedCallSiteContract {
            span: 168..178,
            kind: ContinuationResume,
        },
        TypedCallSiteContract {
            span: 183..195,
            kind: ContinuationResume,
        },
        TypedCallSiteContract {
            span: 200..211,
            kind: DirectCall,
        },
        TypedCallSiteContract {
            span: 216..229,
            kind: DirectCall,
        },
        TypedCallSiteContract {
            span: 330..349,
            kind: ContinuationResume,
        },
    ],
    continuation_resume_sites: [
        ContinuationResumeSiteContract {
            span: 168..178,
            receiver_ty: scoop.core.Continuation<Unit, Unit, eff Pure>,
            resume_ty: Unit,
            answer_ty: Unit,
            return_ty: Unit,
            out_effects: Pure,
            required_effects: scoop.core.Raise<scoop.core.RuntimeError>,
            includes_runtime_error_effect: true,
        },
        ContinuationResumeSiteContract {
            span: 183..195,
            receiver_ty: scoop.core.Continuation<Unit, Unit, eff Pure>,
            resume_ty: Unit,
            answer_ty: Unit,
            return_ty: Unit,
            out_effects: Pure,
            required_effects: scoop.core.Raise<scoop.core.RuntimeError>,
            includes_runtime_error_effect: true,
        },
        ContinuationResumeSiteContract {
            span: 330..349,
            receiver_ty: scoop.core.Continuation<(Int, String), Unit, eff Pure>,
            resume_ty: (Int, String),
            answer_ty: Unit,
            return_ty: Unit,
            out_effects: Pure,
            required_effects: scoop.core.Raise<scoop.core.RuntimeError>,
            includes_runtime_error_effect: true,
        },
    ],
    perform_sites: [
    ],
    handle_sites: [
    ],
}"#,
        );
    }

    #[test]
    fn refactor_typed_hir_runtime_error_contract_dump_records_required_effect() {
        let session = refactor_session();
        let source = SourceFile::new_virtual(
            "<mem>/runtime_error_contract.scoop",
            r#"
package fixtures.hir

import scoop.core.*

fun resumeWithEffects(k: Continuation<Int, Int, eff Raise<Int>>): Int / (Raise<Int> + Raise<RuntimeError>) {
    return k.resume(1)
}
"#,
        );

        let output = run(&session, &source).unwrap();
        let rendered = output.effect_contracts().stable_dump(output.types());

        assert!(rendered.contains("out_effects: scoop.core.Raise<Int>"));
        assert!(rendered.contains(
            "required_effects: scoop.core.Raise<Int> + scoop.core.Raise<scoop.core.RuntimeError>"
        ));
        assert!(rendered.contains("includes_runtime_error_effect: true"));
    }

    #[test]
    fn refactor_typed_hir_handle_contract_dump_snapshot() {
        assert_fixture_effect_contract_dump(
            "handle_perform.scoop",
            r#"TypedHirEffectContracts {
    function_effects: [
        FunctionEffectContract {
            span: 36..125,
            fqn: "a.main",
            return_ty: Int,
            allowed_effects: Pure,
            effects_closed: false,
        },
    ],
    call_site_kinds: [
        TypedCallSiteContract {
            span: 64..78,
            kind: Perform,
        },
    ],
    continuation_resume_sites: [
    ],
    perform_sites: [
        PerformSiteContract {
            span: 64..78,
            effect_ty: scoop.core.Raise<Int>,
            op_fqn: "scoop.core.Raise.raise",
            payload_ty: Int,
            payload_components: [
                Int,
            ],
            arg_mapping: [0],
        },
    ],
    handle_sites: [
        HandleSiteContract {
            span: 51..123,
            result_ty: Int,
            body_result_ty: Int,
            arm_contracts: [
                HandleArmSiteContract {
                    op_fqn: "scoop.core.Raise.raise",
                    handled_effect_ty: scoop.core.Raise<Int>,
                    payload_ty: Int,
                    payload_components: [
                        Int,
                    ],
                    body_ty: Int,
                    kind: NonResuming,
                },
            ],
            finally_result_ty: None,
        },
    ],
}"#,
        );
    }

    #[test]
    fn refactor_typed_hir_collects_perform_and_handle_contracts() {
        let session = refactor_session();
        let source = load_hir_fixture("handle_perform.scoop");

        let output = run(&session, &source).unwrap();
        let contracts = output.effect_contracts();

        assert_eq!(contracts.perform_sites().len(), 1);
        let (perform_site, perform_contract) = contracts
            .perform_sites()
            .iter()
            .next()
            .expect("应收集到 perform site");
        assert_eq!(
            contracts.call_site_kind(perform_site),
            Some(TypedCallSiteKind::Perform)
        );
        assert_eq!(perform_contract.op_fqn(), "scoop.core.Raise.raise");
        assert_eq!(perform_contract.payload().components().len(), 1);
        assert_eq!(
            output
                .types()
                .display(perform_contract.payload().components()[0])
                .to_string(),
            "Int"
        );

        assert_eq!(contracts.handle_sites().len(), 1);
        let handle_contract = contracts
            .handle_sites()
            .values()
            .next()
            .expect("应收集到 handle site");
        assert_eq!(
            output
                .types()
                .display(handle_contract.result_ty())
                .to_string(),
            "Int"
        );
        assert_eq!(
            output
                .types()
                .display(handle_contract.body_result_ty())
                .to_string(),
            "Int"
        );
        assert_eq!(handle_contract.arm_contracts().len(), 1);
        let arm = &handle_contract.arm_contracts()[0];
        assert_eq!(arm.op_fqn(), "scoop.core.Raise.raise");
        assert_eq!(arm.kind(), HandleArmContractKind::NonResuming);
        assert_eq!(
            output.types().display(arm.handled_effect_ty()).to_string(),
            "scoop.core.Raise<Int>"
        );
    }
}
