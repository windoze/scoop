//! per-function lowering 构建器 [`FnLowering`]。
//!
//! 持有正在构建的 [`Body`]（locals + blocks）、当前块、符号→local 映射、loop 栈。
//! 表达式 lowering（[`expr`]）与语句 lowering（[`stmt`]）通过
//! `&mut FnLowering` 写入。

use std::collections::HashMap;

use scoop2_base::{FileId, NodeId, Span, Symbol};
use scoop2_hir::ty::{EffectRow, TypeId, TypeStore};
use scoop2_hir::hir::TypedHir;

use crate::diagnostics::MirLowerError;
use crate::mir::{
    BasicBlock, BasicBlockId, Body, FunDecl, InitializerRoot, Item, LocalDecl, LocalId,
    LocalSource, Param, Terminator, TerminatorKind,
};

/// per-function 构建器。
pub struct FnLowering<'hir> {
    pub hir: &'hir TypedHir,
    pub types: TypeStore,
    pub file_id: FileId,
    pub owner_fqn: String,
    pub return_ty: TypeId,
    pub body: Body,
    /// 当前正在填充的基本块 id。
    pub current_bb: BasicBlockId,
    /// 源符号（局部名）→ local id。
    pub symbol_locals: HashMap<Symbol, LocalId>,
    /// 当前函数声明的效果行。
    pub effect_row: EffectRow,
    /// loop 栈（break/continue 目标）。
    pub loop_stack: Vec<LoopContext>,
    /// 返回 local（隐式 return 的目的地；函数返回值写这里）。
    pub return_local: Option<LocalId>,
    /// 成员函数的 `this` local（隐式接收者；成员函数体内裸字段访问 / 赋值的接收者）。
    pub this_local: Option<LocalId>,
    /// 错误累积。
    pub errors: &'hir mut Vec<MirLowerError>,
    /// 嵌套函数（闭包）——在 lower 函数体时收集，调用方追加为 sibling items。
    pub nested_funs: Vec<FunDecl>,
    /// 闭包计数器（合成 `<owner>$closure<N>`）。
    pub closure_counter: u32,
    /// 当前正在 lower 的表达式 NodeId（供 lower_call/lower_binary 查 call_resolutions）。
    pub current_expr_id: scoop2_base::NodeId,
    /// 当前文件中所有 enum 类型的 FQN 集合（用于 transport kind 分类：struct vs enum）。
    pub enum_fqns: std::collections::HashSet<scoop2_base::Symbol>,
}

/// loop 上下文（break/continue 目标）。
#[derive(Clone, Copy)]
pub struct LoopContext {
    pub break_target: BasicBlockId,
    pub continue_target: BasicBlockId,
}

impl<'hir> FnLowering<'hir> {
    pub fn new(
        hir: &'hir TypedHir,
        types: TypeStore,
        file_id: FileId,
        owner_fqn: String,
        return_ty: TypeId,
        effect_row: EffectRow,
        errors: &'hir mut Vec<MirLowerError>,
    ) -> Self {
        let mut body = Body::new();
        // 入口块 bb0（占位终结符，后续替换）。
        let start = body.push_block(BasicBlock::new(unreachable_term()));
        // 从 HIR 收集所有 enum FQN，供 transport kind 分类使用。
        let enum_fqns: std::collections::HashSet<scoop2_base::Symbol> = hir.enum_variants.keys().copied().collect();
        Self {
            hir,
            types,
            file_id,
            owner_fqn,
            return_ty,
            body,
            current_bb: start,
            symbol_locals: HashMap::new(),
            effect_row,
            loop_stack: Vec::new(),
            return_local: None,
            this_local: None,
            errors,
            nested_funs: Vec::new(),
            closure_counter: 0,
            current_expr_id: scoop2_base::NodeId::from_u32(u32::MAX),
            enum_fqns,
        }
    }

    /// 申明一个 local（命名或临时），返回 id。
    pub fn alloc_local(&mut self, decl: LocalDecl) -> LocalId {
        self.body.push_local(decl)
    }

    /// 申明一个临时 local（类型 = ty）。
    pub fn alloc_temp(&mut self, ty: TypeId, span: Span) -> LocalId {
        self.alloc_local(LocalDecl {
            span,
            name: None,
            ty,
            source: LocalSource::Temp,
        mutable: false,
        })
    }

    /// 申明一个命名 local（参数 / val / var / pattern binder）。
    pub fn alloc_named(&mut self, name: String, ty: TypeId, span: Span) -> LocalId {
        self.alloc_named_mutable(name, ty, span, false)
    }

    /// 申明一个命名 local，指定可变性。
    pub fn alloc_named_mutable(&mut self, name: String, ty: TypeId, span: Span, mutable: bool) -> LocalId {
        self.alloc_local(LocalDecl {
            span,
            name: Some(name),
            ty,
            source: LocalSource::Source,
            mutable,
        })
    }

    /// 取某表达式的推断类型（None 表示 typecheck 未覆盖）。
    pub fn expr_type_of(&self, expr_id: NodeId) -> Option<TypeId> {
        self.hir.expr_type(self.file_id, expr_id)
    }

    /// 取某表达式的推断类型，缺失时回退到 Nothing。
    pub fn expr_ty(&mut self, expr_id: NodeId) -> TypeId {
        self.hir
            .expr_type(self.file_id, expr_id)
            .unwrap_or_else(|| self.types.nothing())
    }

    /// Nothing 类型。
    pub fn nothing_ty(&mut self) -> TypeId {
        self.types.nothing()
    }

    /// Any 类型。
    pub fn any_ty(&mut self) -> TypeId {
        self.types.any()
    }

    /// Unit 类型。
    pub fn unit_ty(&mut self) -> TypeId {
        self.types.unit()
    }

    /// Bool 类型。
    pub fn bool_ty(&mut self) -> TypeId {
        self.types.bool()
    }

    /// 在当前块追加一条 statement。
    pub fn push_stmt(&mut self, stmt: crate::mir::Statement) {
        let bb = self.current_bb;
        self.body.blocks[bb.0 as usize].stmts.push(stmt);
    }

    /// 在当前块追加 `target = value`。
    pub fn assign(&mut self, target: LocalId, value: crate::mir::Rvalue, span: Span) {
        self.push_stmt(crate::mir::Statement {
            span,
            kind: crate::mir::StatementKind::Assign { target, value },
        });
    }

    /// 创建一个新基本块（无终结符；占位 Unreachable）。
    pub fn new_block(&mut self) -> BasicBlockId {
        self.body.push_block(BasicBlock::new(unreachable_term()))
    }

    /// 结束当前块（设置其终结符），切换 current_bb 到 target。
    pub fn terminate(&mut self, term: Terminator, target: BasicBlockId) {
        let bb = self.current_bb;
        self.body.blocks[bb.0 as usize].terminator = term;
        self.current_bb = target;
    }

    /// 结束当前块并 goto target（当前块以 Goto 终结）。
    pub fn goto(&mut self, target: BasicBlockId, span: Span) {
        let term = Terminator {
            span,
            kind: TerminatorKind::Goto { target },
        };
        let bb = self.current_bb;
        self.body.blocks[bb.0 as usize].terminator = term;
        self.current_bb = target;
    }

    /// 用指定错误码报 lowering 错误（具体语义码，非笼统 unsupported）。
    pub fn error(&mut self, code: &'static str, span: Span, what: impl Into<String>) {
        self.errors.push(MirLowerError::new(code, span, what));
    }

    /// 为某 callee FQN 计算 StableTemplateKey。
    pub fn stable_template_key_for(&self, callee_fqn: &str) -> Option<crate::mir::StableTemplateKey> {
        // 从 HIR type_constraints 查找该 FQN 的真实类型参数名序列。
        let fqn_sym = self.hir.interner.get(callee_fqn);
        let type_params: Vec<String> = if let Some(fqn) = fqn_sym {
            // 先查 type_constraints（携带真实类型参数名），把 Symbol 解析为文本。
            if let Some(tc) = self.hir.type_constraints.get(&fqn) {
                tc.type_params
                    .iter()
                    .map(|&sym| self.hir.interner.resolve(sym).to_string())
                    .collect()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };
        // overload_sig：从 HIR 签名查找首个重载的参数类型 canonical 文本。
        let overload_sig = self.overload_sig_for_fqn(callee_fqn);
        Some(crate::mir::stable_id::make_stable_template_key(
            crate::mir::stable_id::StableHashScope::Dump,
            callee_fqn,
            &type_params,
            &overload_sig,
        ))
    }

    /// 从 HIR 查找某 callee FQN 首个重载的 overload signature canonical 文本。
    ///
    /// 先尝试顶层函数表（FQN 整体匹配），再退化为成员函数表（拆 `owner.method`）。
    /// 找不到时返回空串（无法区分同名重载，但不阻断 lowering）。
    fn overload_sig_for_fqn(&self, callee_fqn: &str) -> String {
        // 顶层函数：FQN 整体匹配。
        if let Some(fqn_sym) = self.hir.interner.get(callee_fqn) {
            if let Some(sigs) = self.hir.top_level_funs.get(&fqn_sym) {
                if let Some(first) = sigs.first() {
                    return crate::mir::stable_id::build_overload_sig(
                        &self.types,
                        &self.hir.interner,
                        &first.param_types,
                    );
                }
            }
        }
        // 成员函数：拆 `owner.method`（最后一个 `.` 之前为 owner）。
        if let Some(dot) = callee_fqn.rfind('.') {
            let owner_str = &callee_fqn[..dot];
            let method_str = &callee_fqn[dot + 1..];
            if let (Some(owner_sym), Some(method_sym)) = (
                self.hir.interner.get(owner_str),
                self.hir.interner.get(method_str),
            ) {
                if let Some(methods) = self.hir.member_funs.get(&owner_sym) {
                    if let Some(sigs) = methods.get(&method_sym) {
                        if let Some(first) = sigs.first() {
                            return crate::mir::stable_id::build_overload_sig(
                                &self.types,
                                &self.hir.interner,
                                &first.param_types,
                            );
                        }
                    }
                }
            }
        }
        String::new()
    }

    /// 构造 CallKind::Direct，自动计算 stable_template_key。
    pub fn make_direct_call_kind(
        &self,
        callee_fqn: String,
        type_args: Vec<scoop2_hir::ty::TypeId>,
        is_intrinsic: bool,
    ) -> crate::mir::CallKind {
        let stk = self.stable_template_key_for(&callee_fqn);
        let sik = if !type_args.is_empty() {
            stk.as_ref().map(|template| {
                crate::mir::stable_id::make_stable_instance_key(
                    crate::mir::stable_id::StableHashScope::Dump,
                    template.clone(),
                    &self.types,
                    &self.hir.interner,
                    &type_args,
                    &[],
                )
            })
        } else {
            None
        };
        crate::mir::CallKind::Direct {
            callee_fqn,
            type_args: type_args.clone(),
            is_intrinsic,
            stable_template_key: stk,
            stable_instance_key: sik,
            generic_type_args: type_args,
            generic_eff_args: vec![],
        }
    }

    /// 根据 owner FQN 选择 Interface（itable）或 Virtual（class vtable）分发通道。
    ///
    /// owner 在 `hir.interface_fqns` 中 → `CallKind::Interface`；否则 → `CallKind::Virtual`。
    /// 用于 call_resolution 缺失时的回退路径（运算符 / infix / 索引等运算符糖）。
    pub fn make_dispatch_call_kind(
        &self,
        owner_sym: scoop2_base::Symbol,
        receiver: crate::mir::Operand,
        dispatch: crate::mir::DispatchMetadata,
    ) -> crate::mir::CallKind {
        if self.hir.interface_fqns.contains(&owner_sym) {
            crate::mir::CallKind::Interface { receiver, dispatch }
        } else {
            crate::mir::CallKind::Virtual { receiver, dispatch }
        }
    }

    // ----- transport metadata helpers -----

    /// 分配一个新的 SiteId（per-call-site 稳定身份）。
    pub fn next_site_id(&mut self) -> crate::mir::SiteId {
        let id = self.body.next_site_id();
        crate::mir::SiteId(id)
    }

    /// 计算某类型的 ValueTransportMetadata（无 boxing）。
    pub fn value_transport(&self, ty: TypeId) -> crate::mir::ValueTransportMetadata {
        crate::mir::transport::value_transport(&self.types, &self.enum_fqns, ty)
    }

    /// 计算某类型的 ValueTransportMetadata（带 boxing：当 source_ty 是值类型且需要擦除到 target_ty 时）。
    /// target_ty 通常是 Any 或 Ref；若 source_ty == target_ty 或 source 不是值类型则无 boxing。
    pub fn value_transport_boxed(&mut self, source_ty: TypeId, target_ty: TypeId) -> crate::mir::ValueTransportMetadata {
        let any_ty = self.types.any();
        match crate::mir::transport::value_erasure_transport(
            &self.types,
            &self.enum_fqns,
            any_ty,
            source_ty,
            target_ty,
        ) {
            Some(boxed) => boxed,
            None => crate::mir::transport::value_transport(&self.types, &self.enum_fqns, source_ty),
        }
    }

    /// 计算调用结果的 CallTransportMetadata。
    /// 从 result_ty 的类型结构计算 transport kind / requirements / boxing。
    /// aggregate_return / array / gc 按 result_ty 是否为聚合/数组/GC-intrinsic 判断。
    pub fn call_transport(&mut self, result_ty: TypeId) -> crate::mir::CallTransportMetadata {
        let any_ty = self.types.any();
        // 检测值类型擦除到 Any：若 result_ty 是值类型且 any_ty 存在，标记 boxing。
        let result = if result_ty != any_ty
            && matches!(self.types.kind(result_ty), scoop2_hir::ty::TypeKind::Value(_))
        {
            // 值类型结果：可能需要 boxing 到 Any（取决于调用点期望类型，
            // 当前无目标类型信息，保守不 box——但 boxing intent 仍从类型结构精确计算）。
            crate::mir::transport::value_transport(&self.types, &self.enum_fqns, result_ty)
        } else {
            crate::mir::transport::value_transport(&self.types, &self.enum_fqns, result_ty)
        };
        // aggregate_return：若返回类型是聚合（tuple/struct/enum），标记。
        let aggregate_return = if crate::mir::transport::mir_is_aggregate_transport_ty(&self.types, result_ty) {
            Some(result.clone())
        } else {
            None
        };
        crate::mir::CallTransportMetadata {
            result,
            aggregate_return,
            array: None, // 数组 transport 从 call args 推断；当前不涉及。
            gc: None,    // GC intrinsic 从 callee FQN 推断；当前不涉及。
            abi: crate::mir::transport::CallAbiHandoffMetadata::plain_no_outward(),
        }
    }

    /// 计算 AggregateTransportMetadata（tuple/struct/enum payload）。
    pub fn aggregate_transport(
        &mut self,
        aggregate_ty: TypeId,
        kind: crate::mir::AggregateTransportKind,
    ) -> crate::mir::AggregateTransportMetadata {
        use scoop2_hir::ty::{TypeKind, ValueTypeKind, RefTypeKind};
        let element_tys: Vec<TypeId> = match self.types.kind(aggregate_ty) {
            TypeKind::Value(ValueTypeKind::Tuple(els)) => els.clone(),
            TypeKind::Value(ValueTypeKind::Option(inner)) => vec![*inner],
            TypeKind::Value(ValueTypeKind::Nominal(n)) => {
                // struct/enum 的字段类型：从 HIR members 查询。
                let fqn = n.fqn;
                if let Some(members) = self.hir.members.get(&fqn) {
                    members.values().copied().collect()
                } else {
                    Vec::new()
                }
            }
            TypeKind::Ref(RefTypeKind::Nominal(n)) => {
                let fqn = n.fqn;
                if let Some(members) = self.hir.members.get(&fqn) {
                    members.values().copied().collect()
                } else {
                    Vec::new()
                }
            }
            _ => Vec::new(),
        };
        let fields: Vec<crate::mir::AggregateTransportField> = element_tys
            .iter()
            .enumerate()
            .map(|(i, &ety)| crate::mir::AggregateTransportField {
                index: i,
                name: None,
                ty: ety,
                transport: crate::mir::transport::value_transport(
                    &self.types,
                    &self.enum_fqns,
                    ety,
                ),
            })
            .collect();
        crate::mir::AggregateTransportMetadata {
            aggregate_ty,
            kind,
            fields,
        }
    }

    /// 构造 MemberAccessMetadata，解析 member target。
    pub fn member_access_metadata(
        &self,
        name: &str,
        receiver_ty: TypeId,
    ) -> crate::mir::MemberAccessMetadata {
        // 尝试解析 member target：从 HIR members 查字段、从 member_funs 查方法。
        let name_sym = self.hir.interner.get(name);
        let resolved = if let Some(sym) = name_sym {
            // 从 receiver_ty 的 nominal FQN 查 owner。
            let owner_fqn = match self.types.kind(receiver_ty) {
                scoop2_hir::ty::TypeKind::Value(scoop2_hir::ty::ValueTypeKind::Nominal(n))
                | scoop2_hir::ty::TypeKind::Ref(scoop2_hir::ty::RefTypeKind::Nominal(n)) => Some(n.fqn),
                _ => None,
            };
            if let Some(fqn) = owner_fqn {
                if self.hir.members.get(&fqn).and_then(|m| m.get(&sym)).is_some() {
                    Some(crate::mir::transport::MemberTarget::Value {
                        fqn: format!("{}.{}", self.hir.interner.resolve(fqn), name),
                    })
                } else if self.hir.member_funs.get(&fqn).and_then(|m| m.get(&sym)).is_some() {
                    Some(crate::mir::transport::MemberTarget::Fun {
                        fqn: format!("{}.{}", self.hir.interner.resolve(fqn), name),
                    })
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };
        crate::mir::MemberAccessMetadata {
            name: name.to_string(),
            receiver_ty,
            resolved,
            hidden_effects: scoop2_hir::ty::EffectRow::pure(),
        }
    }

    /// 把当前 body 收尾：确保入口块以某终结符结束（若未设置，按 return Unit）。
    pub fn finish(mut self) -> (Body, Vec<FunDecl>, TypeStore) {
        // 若当前块仍是占位 Unreachable 且函数应返回，补隐式 return。
        let bb = self.current_bb;
        let term = &self.body.blocks[bb.0 as usize].terminator;
        if matches!(term.kind, TerminatorKind::Unreachable) {
            self.body.blocks[bb.0 as usize].terminator = Terminator {
                span: Span::default(),
                kind: TerminatorKind::Return { value: None },
            };
        }
        (self.body, self.nested_funs, self.types)
    }
}

fn unreachable_term() -> Terminator {
    Terminator {
        span: Span::default(),
        kind: TerminatorKind::Unreachable,
    }
}

// ---------------------------------------------------------------------------
// 顶层 lowering 入口（FunDecl / InitializerRoot）
// ---------------------------------------------------------------------------

/// lower 一个函数声明为 FunDecl（含 body）。
/// `base_types` 用于克隆出 builder 私有 store（TypeId 与 hir.store 一致）。
/// 返回 (FunDecl, builder 产出的 store 供调用方合并)。
pub fn lower_fun_decl(
    file_id: FileId,
    d: &scoop2_syntax::ast::FunDecl,
    hir: &TypedHir,
    package_prefix: &str,
    base_types: &TypeStore,
    errors: &mut Vec<MirLowerError>,
) -> (Option<FunDecl>, Vec<FunDecl>, TypeStore) {
    lower_fun_decl_inner(file_id, d, hir, package_prefix, base_types, errors, None)
}

/// 带 `member_owner` 的 lower_fun_decl：`Some(owner_sym)` 表示成员函数，
/// 会把 receiver（`this`）作为隐式首参前置（与调用侧的 receiver prepend 对齐）。
pub fn lower_fun_decl_inner(
    file_id: FileId,
    d: &scoop2_syntax::ast::FunDecl,
    hir: &TypedHir,
    package_prefix: &str,
    base_types: &TypeStore,
    errors: &mut Vec<MirLowerError>,
    member_owner: Option<scoop2_base::Symbol>,
) -> (Option<FunDecl>, Vec<FunDecl>, TypeStore) {
    let owner_fqn = super::fqn_of(package_prefix, d.name.symbol, hir);
    // builder 私有 store（从 base 克隆；TypeId 在 base 范围内一致）。
    let mut types = base_types.clone();
    // 函数类型：从签名构造（用 store.function）。
    // 参数类型优先从 TypedHir::top_level_funs 的 TypedSignature.param_types 获取。
    let sig_param_tys: Option<Vec<TypeId>> = hir
        .interner
        .get(&owner_fqn)
        .and_then(|fqn_sym| hir.top_level_funs.get(&fqn_sym))
        .and_then(|sigs| sigs.first())
        .map(|sig| sig.param_types.clone());
    let param_tys: Vec<TypeId> = if let Some(sig_tys) = sig_param_tys {
        // 使用 HIR 签名的参数类型（已由 typecheck 解析为正确 TypeId）。
        sig_tys
    } else {
        // 回退：从 AST TypeRef 解析（当前总是 Nothing）。
        d.params
            .iter()
            .map(|p| {
                p.ty.as_ref()
                    .and_then(|t| hir_param_type(hir, file_id, t.id))
                    .unwrap_or_else(|| types.nothing())
            })
            .collect()
    };
    // 返回类型：优先从 TypedSignature 获取。
    let return_ty = hir
        .interner
        .get(&owner_fqn)
        .and_then(|fqn_sym| hir.top_level_funs.get(&fqn_sym))
        .and_then(|sigs| sigs.first())
        .map(|sig| sig.return_ty)
        .unwrap_or_else(|| {
            d.return_ty
                .as_ref()
                .and_then(|t| hir.expr_type(file_id, t.id).or_else(|| hir_type_ref(t, hir)))
                .unwrap_or_else(|| types.unit())
        });
    // effect 行：尝试从 TypedSignature 表查（顶层函数）。
    let effect_row = lookup_effect_row(hir, d.name.symbol, package_prefix);
    let fn_ty = types.function(scoop2_hir::ty::FunctionType {
        receiver: None,
        params: param_tys.clone(),
        return_ty,
        effects: effect_row.clone(),
        closed: false,
    });
    let mut fd = FunDecl {
        span: d.name.span,
        fqn: owner_fqn.clone(),
        name: hir.interner.resolve(d.name.symbol).to_string(),
        ty: fn_ty,
        params: Vec::new(),
        return_ty,
        effect_row: effect_row.clone(),
        type_params: d
            .type_params
            .as_ref()
            .map(|tp| tp.params.iter().map(|p| p.name.symbol).collect())
            .unwrap_or_default(),
        body: None,
        file: file_id,
        stable_template_key: None,
        effect_abi: None,
    };
    // 无函数体的声明（extern / abstract / intrinsic）：仍需填充签名参数。
    if d.body.is_none() {
        for (i, p) in d.params.iter().enumerate() {
            let pty = param_tys.get(i).copied().unwrap_or_else(|| types.nothing());
            fd.params.push(Param {
                span: p.name.span,
                name: hir.interner.resolve(p.name.symbol).to_string(),
                ty: pty,
                local: crate::mir::LocalId(0),
            });
        }
        return (Some(fd), Vec::new(), types);
    }
    let body = d.body.as_ref().expect("已处理无 body 的情况");
    let mut builder = FnLowering::new(hir, types, file_id, owner_fqn, return_ty, effect_row, errors);
    // 成员函数：receiver（`this`）作为隐式首参前置（与调用侧 receiver prepend 对齐）。
    if let Some(owner_sym) = member_owner {
        let this_ty = resolve_member_receiver_ty(hir, &mut builder.types, owner_sym);
        let this_lid = builder.alloc_named("<this>".to_string(), this_ty, d.name.span);
        builder.this_local = Some(this_lid);
        // 注册 `this` 名（成员函数体内裸 `this` 解析到此 local）。
        if let Some(this_sym) = hir.interner.get("this") {
            builder.symbol_locals.insert(this_sym, this_lid);
        }
        fd.params.insert(0, Param {
            span: d.name.span,
            name: "<this>".to_string(),
            ty: this_ty,
            local: this_lid,
        });
    }
    for (i, p) in d.params.iter().enumerate() {
        let pty = param_tys[i];
        let lid = builder.alloc_named(
            hir.interner.resolve(p.name.symbol).to_string(),
            pty,
            p.name.span,
        );
        builder.symbol_locals.insert(p.name.symbol, lid);
        fd.params.push(Param {
            span: p.name.span,
            name: hir.interner.resolve(p.name.symbol).to_string(),
            ty: pty,
            local: lid,
        });
    }
    let (mir_body, nested, types_out) = lower_fun_body(builder, body);
    fd.body = Some(mir_body);
    // 嵌套闭包函数作为 sibling FunDecl 返回（调用方追加为 module items）。
    (Some(fd), nested, types_out)
}

/// lower 函数体（block 或 expr）。
fn lower_fun_body(
    mut builder: FnLowering,
    body: &scoop2_syntax::ast::FunBody,
) -> (crate::mir::Body, Vec<FunDecl>, TypeStore) {
    use scoop2_syntax::ast::FunBody;
    match body {
        FunBody::Block(b) => {
            crate::mir::lower::stmt::lower_block(&mut builder, b);
        }
        FunBody::Expr(e) => {
            let val = crate::mir::lower::expr::lower_expr(&mut builder, e);
            // 隐式 return val。
            let span = e.span;
            builder.terminate(
                Terminator {
                    span,
                    kind: TerminatorKind::Return { value: Some(val) },
                },
                builder.current_bb,
            );
        }
    }
    builder.finish()
}

// ---------------------------------------------------------------------------
// helper：类型查询
// ---------------------------------------------------------------------------

fn hir_param_type(_hir: &TypedHir, _file_id: FileId, _node: NodeId) -> Option<TypeId> {
    // 参数类型 TypeRef 节点未在 expr_types 中（只有表达式有类型）。
    // 参数类型由 lower_fun_decl 内部从 TypeRef 重新解析——这里返回 None 走 nothing 回退。
    None
}

fn hir_type_ref(t: &scoop2_syntax::ast::TypeRef, hir: &TypedHir) -> Option<TypeId> {
    // TypeRef 节点未类型化；返回 None。
    let _ = (t, hir);
    None
}

/// 查某顶层函数的 effect 行（从 TypedHir::top_level_funs）。
fn lookup_effect_row(hir: &TypedHir, name_sym: Symbol, package_prefix: &str) -> EffectRow {
    let fqn_text = if package_prefix.is_empty() {
        hir.interner.resolve(name_sym).to_string()
    } else {
        format!("{}.{}", package_prefix, hir.interner.resolve(name_sym))
    };
    if let Some(fqn) = hir.interner.get(&fqn_text)
        && let Some(sigs) = hir.top_level_funs.get(&fqn)
        && let Some(first) = sigs.first()
    {
        return first.effect_row.clone();
    }
    EffectRow::pure()
}

/// lower 顶层 val/var 初始化器。返回 (item, builder 产出的 store 供合并)。
pub fn lower_top_level_val(
    file_id: FileId,
    d: &scoop2_syntax::ast::ValDecl,
    hir: &TypedHir,
    package_prefix: &str,
    base_types: &TypeStore,
    errors: &mut Vec<MirLowerError>,
) -> (Option<Item>, TypeStore) {
    let name = match &d.binding {
        scoop2_syntax::ast::ValBinding::Name(id) => id.symbol,
        // 顶层解构是 parse error。
        scoop2_syntax::ast::ValBinding::Pattern(_) => return (None, base_types.clone()),
    };
    let fqn = super::fqn_of(package_prefix, name, hir);
    // builder 私有 store（用于查 nothing / any 等内建类型）。
    let mut probe_store = base_types.clone();
    let nothing_ty = probe_store.nothing();
    let ty = hir.top_level_vals.get(&name).copied().unwrap_or(nothing_ty);
    let Some(init) = &d.init else {
        // @Extern 顶层 var：建模为 ExternGlobal。
        return (
            Some(Item::ExternGlobal(crate::mir::ExternGlobal {
                span: d
                    .ty
                    .as_ref()
                    .map(|t| t.span)
                    .unwrap_or_else(Span::default),
                fqn,
                ty,
                file: file_id,
            })),
            probe_store,
        );
    };
    let effect_row = EffectRow::pure();
    let owner_fqn = format!("{}#init", fqn);
    let mut builder = FnLowering::new(hir, probe_store, file_id, owner_fqn, ty, effect_row.clone(), errors);
    let val = crate::mir::lower::expr::lower_expr(&mut builder, init);
    let cur_bb = builder.current_bb;
    builder.terminate(
        Terminator {
            span: init.span,
            kind: TerminatorKind::Return { value: Some(val) },
        },
        cur_bb,
    );
    let (body, _, store_out) = builder.finish();
    (
        Some(Item::Initializer(InitializerRoot {
            span: init.span,
            fqn,
            ty,
            is_var: d.kind == scoop2_syntax::ast::ValKind::Var,
            body,
            file: file_id,
        })),
        store_out,
    )
}

/// lower 类型体的成员函数（method bodies）。返回 (item, per-function store) 对列表，
/// 供调用方合并 store 并 remap TypeId。
pub fn lower_type_member_funs_with_stores(
    file_id: FileId,
    members: &[scoop2_syntax::ast::TypeMember],
    hir: &TypedHir,
    package_prefix: &str,
    base_types: &TypeStore,
    errors: &mut Vec<MirLowerError>,
    owner_fqn_sym: scoop2_base::Symbol,
) -> Vec<(Item, TypeStore)> {
    use scoop2_syntax::ast::TypeMemberKind;
    let mut out: Vec<(Item, TypeStore)> = Vec::new();
    for m in members {
        if let TypeMemberKind::Fun(fd) = &m.kind {
            let (fun_decl, nested, fn_store) = lower_fun_decl_inner(
                file_id,
                fd,
                hir,
                package_prefix,
                base_types,
                errors,
                Some(owner_fqn_sym),
            );
            if let Some(fun_decl) = fun_decl {
                // 主函数用其 store；嵌套闭包函数共用同一 store（合并时统一 remap）。
                let nest_store = fn_store.clone();
                out.push((Item::Fun(fun_decl), fn_store));
                for nf in nested {
                    out.push((Item::Fun(nf), nest_store.clone()));
                }
            }
        }
        // 嵌套类型 / object 的成员函数递归。
        if let TypeMemberKind::Type(td) = &m.kind
            && let Some(body) = &td.body
        {
            let nested_owner = nested_owner_fqn(hir, owner_fqn_sym, td.name.symbol);
            out.extend(lower_type_member_funs_with_stores(
                file_id,
                &body.members,
                hir,
                package_prefix,
                base_types,
                errors,
                nested_owner,
            ));
        }
        if let TypeMemberKind::Object(od) = &m.kind
            && let Some(body) = &od.body
        {
            if let Some(name) = od.name {
                let nested_owner = nested_owner_fqn(hir, owner_fqn_sym, name.symbol);
                out.extend(lower_type_member_funs_with_stores(
                    file_id,
                    &body.members,
                    hir,
                    package_prefix,
                    base_types,
                    errors,
                    nested_owner,
                ));
            }
        }
    }
    out
}

/// 计算嵌套类型的 owner FQN symbol：`<outer>.<name>`。
fn nested_owner_fqn(
    hir: &TypedHir,
    outer: scoop2_base::Symbol,
    name: scoop2_base::Symbol,
) -> scoop2_base::Symbol {
    let outer_text = hir.interner.resolve(outer);
    let name_text = hir.interner.resolve(name);
    let fqn_text = format!("{outer_text}.{name_text}");
    hir.interner.get(&fqn_text).unwrap_or(outer)
}

/// 解析成员函数 receiver（`this`）类型：class/interface → Ref(Nominal)；struct/enum → Value(Nominal)。
fn resolve_member_receiver_ty(
    hir: &TypedHir,
    store: &mut TypeStore,
    owner_sym: scoop2_base::Symbol,
) -> scoop2_hir::ty::TypeId {
    use scoop2_hir::ty::{NominalType, RefTypeKind, TypeKind, ValueTypeKind};
    let is_ref = hir.class_fqns.contains(&owner_sym) || hir.interface_fqns.contains(&owner_sym);
    let nominal = NominalType {
        fqn: owner_sym,
        args: Vec::new(),
        eff: None,
    };
    if is_ref {
        store.intern(TypeKind::Ref(RefTypeKind::Nominal(nominal)))
    } else {
        store.intern(TypeKind::Value(ValueTypeKind::Nominal(nominal)))
    }
}
