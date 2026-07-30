//! per-function lowering 构建器 [`FnLowering`]。
//!
//! 持有正在构建的 [`Body`]（locals + blocks）、当前块、符号→local 映射、loop 栈。
//! 表达式 lowering（[`expr`]）与语句 lowering（[`stmt`]）通过
//! `&mut FnLowering` 写入。

use std::collections::HashMap;

use scoop2_base::{FileId, NodeId, Span, Symbol};
use scoop2_hir::hir::{ClassCtorParamInfo, SuperCtorDelegation, TypedHir};
use scoop2_hir::ty::{EffectRow, TypeId, TypeStore};

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
        let enum_fqns: std::collections::HashSet<scoop2_base::Symbol> =
            hir.enum_variants.keys().copied().collect();
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
    pub fn alloc_named_mutable(
        &mut self,
        name: String,
        ty: TypeId,
        span: Span,
        mutable: bool,
    ) -> LocalId {
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

    /// Array 引用类型（`Ref(scoop.core.Array)`，无类型实参）。
    /// 用于空数组字面量 `[]` 的 MakeArray 临时（表达式类型为 Nothing 时回退）。
    pub fn array_ref_ty(&mut self) -> scoop2_hir::ty::TypeId {
        let array_fqn = self
            .hir
            .interner
            .get("scoop.core.Array")
            .unwrap_or_default();
        self.types.ref_nominal(scoop2_hir::ty::NominalType {
            fqn: array_fqn,
            args: Vec::new(),
            eff: None,
        })
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
    pub fn stable_template_key_for(
        &self,
        callee_fqn: &str,
    ) -> Option<crate::mir::StableTemplateKey> {
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
        self.make_direct_call_kind_with_params(callee_fqn, type_args, is_intrinsic, None)
    }

    /// 构建 Direct call kind，使用 HIR 携带的 param_types 构建 overload_sig
    /// （不再查 hir.top_level_funs / hir.member_funs）。
    pub fn make_direct_call_kind_with_params(
        &self,
        callee_fqn: String,
        type_args: Vec<scoop2_hir::ty::TypeId>,
        is_intrinsic: bool,
        param_types: Option<&[scoop2_hir::ty::TypeId]>,
    ) -> crate::mir::CallKind {
        let stk = if let Some(pts) = param_types {
            // 用 HIR 携带的 param_types 构建 overload_sig（不再查 HIR 表）。
            let overload_sig = crate::mir::stable_id::build_overload_sig(
                &self.types,
                &self.hir.interner,
                pts,
            );
            // type_params 仍需从 hir.type_constraints 查（这是声明信息，非 resolution）。
            let fqn_sym = self.hir.interner.get(&callee_fqn);
            let type_params: Vec<String> = if let Some(fqn) = fqn_sym {
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
            Some(crate::mir::stable_id::make_stable_template_key(
                crate::mir::stable_id::StableHashScope::Dump,
                &callee_fqn,
                &type_params,
                &overload_sig,
            ))
        } else {
            self.stable_template_key_for(&callee_fqn)
        };
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
            callee_fqn: callee_fqn.clone(),
            type_args: type_args.clone(),
            is_intrinsic,
            stable_template_key: stk,
            stable_instance_key: sik,
            generic_type_args: type_args,
            generic_eff_args: vec![],
            intrinsic_name: self.lookup_intrinsic_name_for_fqn(&callee_fqn),
        }
    }

    /// 从 callee FQN 查找 @Intrinsic 注解名（从 hir declarations 表）。
    fn lookup_intrinsic_name_for_fqn(&self, callee_fqn: &str) -> Option<String> {
        // callee_fqn 是 owner.method 形式的 FQN。
        // intrinsic_name 存储在 MIR FunDecl 中，但此处只有 FQN 字符串。
        // 通过 hir interner 查找 Symbol，然后在 declarations 表中匹配。
        let fqn_sym = self.hir.interner.get(callee_fqn)?;
        // hir 不直接暴露 declarations，但 intrinsic 信息在 MIR module items 中。
        // 实际上 intrinsic_name 在 MIR lower 阶段已存入 FunDecl。
        // 此处我们无法直接访问——需要在 lower 阶段填充。
        // 简化：返回 None，由 LIR 层从 LirDeclaration 填充。
        let _ = fqn_sym;
        None
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
    pub fn value_transport_boxed(
        &mut self,
        source_ty: TypeId,
        target_ty: TypeId,
    ) -> crate::mir::ValueTransportMetadata {
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
            && matches!(
                self.types.kind(result_ty),
                scoop2_hir::ty::TypeKind::Value(_)
            ) {
            // 值类型结果：可能需要 boxing 到 Any（取决于调用点期望类型，
            // 当前无目标类型信息，保守不 box——但 boxing intent 仍从类型结构精确计算）。
            crate::mir::transport::value_transport(&self.types, &self.enum_fqns, result_ty)
        } else {
            crate::mir::transport::value_transport(&self.types, &self.enum_fqns, result_ty)
        };
        // aggregate_return：若返回类型是聚合（tuple/struct/enum），标记。
        let aggregate_return =
            if crate::mir::transport::mir_is_aggregate_transport_ty(&self.types, result_ty) {
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
        use scoop2_hir::ty::{RefTypeKind, TypeKind, ValueTypeKind};
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
                | scoop2_hir::ty::TypeKind::Ref(scoop2_hir::ty::RefTypeKind::Nominal(n)) => {
                    Some(n.fqn)
                }
                _ => None,
            };
            if let Some(fqn) = owner_fqn {
                if self
                    .hir
                    .members
                    .get(&fqn)
                    .and_then(|m| m.get(&sym))
                    .is_some()
                {
                    Some(crate::mir::transport::MemberTarget::Value {
                        fqn: format!("{}.{}", self.hir.interner.resolve(fqn), name),
                    })
                } else if self
                    .hir
                    .member_funs
                    .get(&fqn)
                    .and_then(|m| m.get(&sym))
                    .is_some()
                {
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

/// 查成员函数的 TypedSignature。
///
/// 优先按声明 owner 精确查找；未命中时回退到全表搜索（按 owner FQN 文本排序，
/// 保证确定性——不得直接依赖 HashMap 迭代序，否则跨 owner 同名方法
/// （String.compareTo / Int.compareTo / Char.compareTo …）会随机选错签名）。
/// 同一 (owner, method) 可能有重载：优先选参数个数与 AST 声明一致的签名。
fn lookup_member_sig<'h>(
    hir: &'h TypedHir,
    member_owner: Option<scoop2_base::Symbol>,
    method_sym: scoop2_base::Symbol,
    arity: usize,
) -> Option<&'h scoop2_hir::hir::TypedSignature> {
    let pick = |sigs: &'h [scoop2_hir::hir::TypedSignature]| {
        sigs.iter()
            .find(|s| s.param_types.len() == arity)
            .or(sigs.first())
    };
    if let Some(owner) = member_owner {
        if let Some(sig) = hir
            .member_funs
            .get(&owner)
            .and_then(|methods| methods.get(&method_sym))
            .and_then(|sigs| pick(sigs))
        {
            return Some(sig);
        }
    }
    let mut owners: Vec<&scoop2_base::Symbol> = hir.member_funs.keys().collect();
    owners.sort_by(|a, b| hir.interner.resolve(**a).cmp(hir.interner.resolve(**b)));
    owners
        .into_iter()
        .filter_map(|o| {
            hir.member_funs
                .get(o)
                .and_then(|methods| methods.get(&method_sym))
        })
        .find_map(|sigs| pick(sigs))
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
    // 成员函数：使用 owner-qualified FQN（owner.method）以区分不同 owner 的同名方法。
    // 非成员函数（顶层函数）：使用 package.method FQN。
    let owner_fqn = if let Some(owner_sym) = member_owner {
        let owner_text = hir.interner.resolve(owner_sym);
        let method_text = hir.interner.resolve(d.name.symbol);
        format!("{}.{}", owner_text, method_text)
    } else {
        super::fqn_of(package_prefix, d.name.symbol, hir)
    };
    // builder 私有 store（从 base 克隆；TypeId 在 base 范围内一致）。
    let mut types = base_types.clone();
    // 函数类型：从签名构造（用 store.function）。
    // 参数类型优先从 TypedSignature 获取（顶层函数或成员函数）。
    let sig_param_tys: Option<Vec<TypeId>> = {
        let fqn_sym = hir.interner.get(&owner_fqn);
        let from_top = fqn_sym
            .and_then(|s| hir.top_level_funs.get(&s))
            .and_then(|sigs| sigs.first())
            .map(|sig| sig.param_types.clone());
        if from_top.is_some() {
            from_top
        } else {
            // 成员函数：按声明 owner 精确查 member_funs（跨 owner 同名方法
            // 如 String.compareTo/Int.compareTo 签名不同，不得混用）。
            lookup_member_sig(hir, member_owner, d.name.symbol, d.params.len())
                .map(|sig| sig.param_types.clone())
        }
    };
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
    // 返回类型：优先从 TypedSignature 获取（顶层函数或成员函数）。
    let return_ty = {
        let fqn_sym = hir.interner.get(&owner_fqn);
        // 1. 顶层函数签名。
        let from_top = fqn_sym
            .and_then(|s| hir.top_level_funs.get(&s))
            .and_then(|sigs| sigs.first())
            .map(|sig| sig.return_ty);
        // 2. 成员函数签名：按声明 owner 精确查 member_funs（同参数类型的查找）。
        let from_member = if from_top.is_none() {
            lookup_member_sig(hir, member_owner, d.name.symbol, d.params.len())
                .map(|sig| sig.return_ty)
        } else {
            None
        };
        from_top.or(from_member).unwrap_or_else(|| {
            d.return_ty
                .as_ref()
                .and_then(|t| {
                    hir.expr_type(file_id, t.id)
                        .or_else(|| hir_type_ref(t, hir))
                })
                .unwrap_or_else(|| types.unit())
        })
    };
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
        instance_symbol: None,
        effect_abi: None,
        intrinsic_name: extract_intrinsic_name(d, hir),
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
    let mut builder = FnLowering::new(
        hir, types, file_id, owner_fqn, return_ty, effect_row, errors,
    );
    // 成员函数：receiver（`this`）作为隐式首参前置（与调用侧 receiver prepend 对齐）。
    if let Some(owner_sym) = member_owner {
        let this_ty = resolve_member_receiver_ty(hir, &mut builder.types, owner_sym);
        let this_lid = builder.alloc_named("<this>".to_string(), this_ty, d.name.span);
        builder.this_local = Some(this_lid);
        // 注册 `this` 名（成员函数体内裸 `this` 解析到此 local）。
        if let Some(this_sym) = hir.interner.get("this") {
            builder.symbol_locals.insert(this_sym, this_lid);
        }
        fd.params.insert(
            0,
            Param {
                span: d.name.span,
                name: "<this>".to_string(),
                ty: this_ty,
                local: this_lid,
            },
        );
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
            let tail = crate::mir::lower::stmt::lower_block(&mut builder, b);
            // 块尾表达式是函数的隐式返回值：若尾块尚未终结（没有显式
            // return / 其他终结符）且尾值非 Unit，补 `Return(tail)`。
            // 尾值为 Unit 时保持旧行为（finish 补 Return(None)）——避免给
            // 非 Unit 返回类型的死尾块（如循环后的 merge 块）接上类型不符的
            // `ret i8 0`。
            let tail_is_unit = matches!(
                tail,
                crate::mir::Operand::Const(crate::mir::ConstValue::Unit)
            );
            let bb = builder.current_bb;
            if !tail_is_unit
                && matches!(
                    builder.body.blocks[bb.0 as usize].terminator.kind,
                    TerminatorKind::Unreachable
                )
            {
                builder.terminate(
                    Terminator {
                        span: b.span,
                        kind: TerminatorKind::Return { value: Some(tail) },
                    },
                    bb,
                );
            }
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
                span: d.ty.as_ref().map(|t| t.span).unwrap_or_else(Span::default),
                fqn,
                ty,
                file: file_id,
            })),
            probe_store,
        );
    };
    let effect_row = EffectRow::pure();
    let owner_fqn = format!("{}#init", fqn);
    let mut builder = FnLowering::new(
        hir,
        probe_store,
        file_id,
        owner_fqn,
        ty,
        effect_row.clone(),
        errors,
    );
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
    type_decl: Option<&scoop2_syntax::ast::TypeDecl>,
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
        // secondary ctor：合成 `<Class>.$ctor.<key>` callable（执行 delegation + body）。
        if let TypeMemberKind::SecondaryCtor(sc) = &m.kind
            && let Some(td) = type_decl
            && let Some((ctor_fd, nested, ctor_store)) = lower_secondary_ctor_callable(
                file_id,
                sc,
                td,
                hir,
                base_types,
                errors,
                owner_fqn_sym,
            )
        {
            out.push((Item::Fun(ctor_fd), ctor_store));
            for nf in nested {
                out.push((Item::Fun(nf), base_types.clone()));
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
                Some(td),
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
                    None,
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

/// 为 class 合成初始化 callable `<Class>.$init`，按 Kotlin 顺序执行：
///   1. super 委托（`: Super(args)`）——递归初始化超类（同一 `this`）；
///   2. 主构造器 `val/var` 属性参数 → 字段赋值；
///   3. 类型体内 property initializer 与 `init {}` 块按源码顺序交错执行。
///
/// 仅当类**确实有初始化体**（init 块 / 属性初始化器 / 超类委托）时返回 Some；
/// 纯字段-参数类（无 init 块、无属性初始化器、无超类）返回 None（codegen 直接
/// 按参数序写字段即可）。
///
/// `this` 是 class 引用类型（GC ptr）；init callable 不返回值（Unit）。
/// 调用方（codegen）在 `scoop_alloc_typed` 之后调用此 callable。
pub fn lower_class_init_callable(
    file_id: FileId,
    d: &scoop2_syntax::ast::TypeDecl,
    hir: &TypedHir,
    package_prefix: &str,
    base_types: &TypeStore,
    errors: &mut Vec<MirLowerError>,
    owner_fqn_sym: scoop2_base::Symbol,
) -> Option<(FunDecl, Vec<FunDecl>, TypeStore)> {
    use scoop2_syntax::ast::TypeMemberKind;

    // 仅 class 需要初始化 callable（struct 无继承/init 块语义）。
    if !matches!(d.kind, scoop2_syntax::ast::TypeKind::Class) {
        return None;
    }
    let body = d.body.as_ref()?;
    let owner_fqn = hir.interner.resolve(owner_fqn_sym).to_string();

    // 判定是否有任何初始化体（init 块 / 属性初始化器 / 超类委托）。
    let has_init_block = body
        .members
        .iter()
        .any(|m| matches!(m.kind, TypeMemberKind::InitBlock(_)));
    let has_property_init = body.members.iter().any(|m| {
        matches!(
            &m.kind,
            TypeMemberKind::Property(p) if p.init.is_some()
        )
    });
    let has_super = hir.super_ctor_delegations.contains_key(&owner_fqn_sym);
    if !(has_init_block || has_property_init || has_super) {
        return None;
    }

    let mut types = base_types.clone();
    let unit_ty = types.unit();
    let effect_row = EffectRow::pure();

    // 构造 this 参数类型：class 引用（GC ptr → Ref Nominal）。
    let this_ty = resolve_member_receiver_ty(hir, &mut types, owner_fqn_sym);

    // 主构造器参数（来自 class_ctor_params）：含 is_property 标记。
    let ctor_params: Vec<ClassCtorParamInfo> = hir
        .class_ctor_params
        .get(&owner_fqn_sym)
        .cloned()
        .unwrap_or_default();
    let _ = package_prefix; // owner_fqn_sym 已携带完整 FQN。

    // 参数类型序列：[this, ctor_param0, ctor_param1, ...]。
    let mut param_tys: Vec<TypeId> = vec![this_ty];
    param_tys.extend(ctor_params.iter().map(|p| p.ty));

    let fn_ty = types.function(scoop2_hir::ty::FunctionType {
        receiver: None,
        params: param_tys,
        return_ty: unit_ty,
        effects: effect_row.clone(),
        closed: false,
    });

    let init_fqn = format!("{}.$init", owner_fqn);
    let mut fd = FunDecl {
        span: d.name.span,
        fqn: init_fqn.clone(),
        name: "$init".to_string(),
        ty: fn_ty,
        params: Vec::new(),
        return_ty: unit_ty,
        effect_row: effect_row.clone(),
        type_params: d
            .type_params
            .as_ref()
            .map(|tp| tp.params.iter().map(|p| p.name.symbol).collect())
            .unwrap_or_default(),
        body: None,
        file: file_id,
        stable_template_key: None,
        instance_symbol: None,
        effect_abi: None,
        intrinsic_name: None,
    };

    let mut builder = FnLowering::new(
        hir, types, file_id, init_fqn, unit_ty, effect_row, errors,
    );

    // 分配 this local 并注册为参数（首参）。
    let this_lid = builder.alloc_named("<this>".to_string(), this_ty, d.name.span);
    builder.this_local = Some(this_lid);
    if let Some(this_sym) = hir.interner.get("this") {
        builder.symbol_locals.insert(this_sym, this_lid);
    }
    fd.params.push(crate::mir::Param {
        span: d.name.span,
        name: "<this>".to_string(),
        ty: this_ty,
        local: this_lid,
    });

    // 分配 ctor 参数 local 并注册符号（init 块/属性初始化器可引用构造参数名）。
    // primary_ctor AST 参数（若存在）提供名字；否则用 class_ctor_params 的 name。
    let primary_param_names: Vec<scoop2_base::Symbol> = d
        .primary_ctor
        .as_ref()
        .map(|pc| pc.params.iter().map(|p| p.name.symbol).collect())
        .unwrap_or_default();
    for (i, cp) in ctor_params.iter().enumerate() {
        let name_sym = primary_param_names.get(i).copied().unwrap_or(cp.name);
        let name_text = builder.hir.interner.resolve(name_sym).to_string();
        let lid = builder.alloc_named(name_text.clone(), cp.ty, d.name.span);
        builder.symbol_locals.insert(name_sym, lid);
        fd.params.push(crate::mir::Param {
            span: d.name.span,
            name: name_text,
            ty: cp.ty,
            local: lid,
        });
    }

    // ---- 执行初始化步骤（Kotlin 顺序）----
    // 1. super 委托：`: Super(args)`。对同一 this 调用超类的 $init。
    //    实参表达式从 d.supertypes[base_index].args 直接 lower（任意表达式）。
    if let Some(super_del) = hir.super_ctor_delegations.get(&owner_fqn_sym) {
        let base_args: &[scoop2_syntax::ast::CallArg] = d
            .supertypes
            .get(super_del.base_index)
            .map(|st| st.args.as_slice())
            .unwrap_or(&[]);
        emit_super_init_call(&mut builder, this_lid, this_ty, super_del, base_args, d.name.span);
    }

    // 2 + 3. 按源码顺序交错执行：
    //   - 主构造器 `val/var` 属性参数 → this.field = param（在首个 property/init-block 之前一次性发出）；
    //   - property initializer（`val x = expr`）→ this.x = expr；
    //   - init block（`init { ... }`）→ 执行块语句。
    // Kotlin 语义：property-param 赋值发生在「第一个 property initializer / init block」之前；
    // 若无任何 property initializer / init block，property-param 赋值仍在末尾发生（保证字段已初始化）。
    let mut emitted_param_props = false;

    // 预计算 property-param → (field_name, param_lid, field_ty) 列表，避免在闭包里
    // 反复 borrow builder.symbol_locals（与 push_stmt 的可变 borrow 冲突）。
    let param_prop_assigns: Vec<(String, LocalId, TypeId)> = ctor_params
        .iter()
        .enumerate()
        .filter(|(_, cp)| cp.is_property)
        .filter_map(|(i, cp)| {
            let param_lid = primary_param_names
                .get(i)
                .and_then(|s| builder.symbol_locals.get(s).copied())
                .unwrap_or(this_lid);
            let field_name = builder.hir.interner.resolve(cp.name).to_string();
            Some((field_name, param_lid, cp.ty))
        })
        .collect();

    let mut emit_param_props = |builder: &mut FnLowering| {
        if emitted_param_props {
            return;
        }
        emitted_param_props = true;
        for (field_name, param_lid, field_ty) in &param_prop_assigns {
            builder.push_stmt(crate::mir::Statement {
                span: d.name.span,
                kind: crate::mir::StatementKind::StoreMember {
                    receiver: crate::mir::Operand::Local(this_lid),
                    member: builder.member_access_metadata(field_name, this_ty),
                    value: crate::mir::Operand::Local(*param_lid),
                    value_ty: *field_ty,
                    continuation_route:
                        crate::mir::transport::StoredContinuationRoutePublication::None,
                },
            });
        }
    };

    for m in &body.members {
        match &m.kind {
            TypeMemberKind::Property(p) => {
                if let Some(init_expr) = &p.init {
                    // 第一个初始化步骤前先发 property-param 赋值。
                    emit_param_props(&mut builder);
                    // property initializer：this.field = init_expr。
                    let field_name = builder
                        .hir
                        .interner
                        .resolve(p.name.symbol)
                        .to_string();
                    let val = crate::mir::lower::expr::lower_expr(&mut builder, init_expr);
                    // property 字段类型：从 HIR members 查（owner.member）。
                    let field_ty = builder
                        .hir
                        .members
                        .get(&owner_fqn_sym)
                        .and_then(|mm| mm.get(&p.name.symbol))
                        .copied()
                        .unwrap_or_else(|| builder.types.nothing());
                    builder.push_stmt(crate::mir::Statement {
                        span: p.name.span,
                        kind: crate::mir::StatementKind::StoreMember {
                            receiver: crate::mir::Operand::Local(this_lid),
                            member: builder.member_access_metadata(&field_name, this_ty),
                            value: val,
                            value_ty: field_ty,
                            continuation_route:
                                crate::mir::transport::StoredContinuationRoutePublication::None,
                        },
                    });
                }
            }
            TypeMemberKind::InitBlock(ib) => {
                emit_param_props(&mut builder);
                // 执行 init block 语句（副作用；尾值丢弃）。
                let _ = crate::mir::lower::stmt::lower_block(&mut builder, &ib.body);
            }
            _ => {}
        }
    }
    // 无 property initializer / init block 时，仍需发 property-param 赋值（super-only 类）。
    emit_param_props(&mut builder);

    // 函数尾：return Unit。
    let (mir_body, nested, types_out) = builder.finish();
    fd.body = Some(mir_body);
    Some((fd, nested, types_out))
}

/// 发出 super 委托初始化调用：`<SuperClass>.$init(this, super_args...)`。
///
/// 实参解析（SuperCtorArg）：CtorParam 引用本类参数 local；Const 是字面量。
/// 递归地让超类在同一 this 上执行其初始化体（属性参数 / 初始化器 / init 块 / 其 super）。
fn emit_super_init_call(
    builder: &mut FnLowering,
    this_lid: LocalId,
    this_ty: TypeId,
    super_del: &SuperCtorDelegation,
    base_args: &[scoop2_syntax::ast::CallArg],
    span: scoop2_base::Span,
) {
    // 超类引用类型（this 的静态类型即子类引用，super init 接收同一 this）。
    let super_fqn_text = builder.hir.interner.resolve(super_del.super_fqn).to_string();
    // 构造调用实参：[this, ...填充后的参数]。
    let mut args: Vec<crate::mir::CallArg> = Vec::new();
    args.push(crate::mir::CallArg {
        name: None,
        is_spread: false,
        value: crate::mir::Operand::Local(this_lid),
        value_ty: this_ty,
    });
    // 查超类 ctor 签名（用于默认参数填充 + 命名实参排序）。
    let super_sig = builder
        .hir
        .ctor_signatures
        .get(&super_del.super_fqn)
        .and_then(|sigs| {
            let n_args = base_args.len();
            sigs.iter().find(|s| {
                let min_arity = s.has_defaults.iter().position(|d| *d).unwrap_or(s.param_types.len());
                n_args >= min_arity && n_args <= s.param_types.len()
            }).or_else(|| sigs.first())
        });
    // 用 lower_delegation_args 填充默认值 + 排序命名实参。
    // base_args 是 d.supertypes[base_index].args（AST CallArg 列表）。
    // 构造一个临时的 CtorDelegation 来复用 lower_delegation_args。
    let temp_del = scoop2_syntax::ast::CtorDelegation {
        span,
        kind: scoop2_syntax::ast::CtorDelegationKind::Super,
        args: base_args.to_vec(),
    };
    let filled = lower_delegation_args(builder, &temp_del, super_sig);
    args.extend(filled);
    let callee_fqn = format!("{}.$init", super_fqn_text);
    let unit_ty = builder.types.unit();
    let tmp = builder.alloc_temp(unit_ty, span);
    builder.push_stmt(crate::mir::Statement {
        span,
        kind: crate::mir::StatementKind::Assign {
            target: tmp,
            value: crate::mir::Rvalue::Call {
                site_id: None,
                kind: crate::mir::CallKind::Direct {
                    callee_fqn,
                    type_args: vec![],
                    is_intrinsic: false,
                    stable_template_key: None,
                    stable_instance_key: None,
                    generic_type_args: vec![],
                    generic_eff_args: vec![],
                    intrinsic_name: None,
                },
                args,
                transport: crate::mir::transport::CallTransportMetadata::plain_no_outward(
                    unit_ty,
                    crate::mir::transport::MirTransportKind::Scalar,
                ),
            },
        },
    });
}

/// 为 secondary ctor 合成 callable `<Class>.$ctor.<span_hash>`。
///
/// 签名：`(this, secondary_params...) -> Unit`。
/// body 执行顺序（Kotlin 语义）：
///   - delegation 为 `this(args)`：lower delegation 实参 → 调 primary `$init`（或另一个
///     secondary ctor callable，按 ctor_selections 解析）→ lower 自己的 body。
///     （property-param 赋值 / initializer / init-block 由被委托的 ctor 负责，此处不重复。）
///   - delegation 为 `super(args)`：emit super $init + property-param 赋值 + initializer +
///     init-block（复用 primary `$init` 机器）→ lower 自己的 body。
///   - 无 delegation（无 primary ctor 的类）：直接 lower body。
pub fn lower_secondary_ctor_callable(
    file_id: FileId,
    sc: &scoop2_syntax::ast::SecondaryCtorDecl,
    d: &scoop2_syntax::ast::TypeDecl,
    hir: &TypedHir,
    base_types: &TypeStore,
    errors: &mut Vec<MirLowerError>,
    owner_fqn_sym: scoop2_base::Symbol,
) -> Option<(FunDecl, Vec<FunDecl>, TypeStore)> {
    use scoop2_syntax::ast::CtorDelegationKind;

    let owner_fqn = hir.interner.resolve(owner_fqn_sym).to_string();
    let mut types = base_types.clone();
    let unit_ty = types.unit();
    let effect_row = EffectRow::pure();
    let this_ty = resolve_member_receiver_ty(hir, &mut types, owner_fqn_sym);

    // secondary ctor 参数类型（从 ctor_signatures 按 span 匹配）。
    let sig_params: Vec<TypeId> = hir
        .ctor_signatures
        .get(&owner_fqn_sym)
        .and_then(|sigs| sigs.iter().find(|s| s.decl_span == sc.span))
        .map(|s| s.param_types.clone())
        .unwrap_or_default();

    let mut param_tys: Vec<TypeId> = vec![this_ty];
    param_tys.extend(sig_params.iter().copied());

    let fn_ty = types.function(scoop2_hir::ty::FunctionType {
        receiver: None,
        params: param_tys,
        return_ty: unit_ty,
        effects: effect_row.clone(),
        closed: false,
    });

    // callable FQN：`<Class>.$ctor.<span_hash>`（用 span.start 区分同类的多个 secondary ctor）。
    let span_key = format!("s{}", sc.span.start);
    let ctor_fqn = format!("{}.$ctor.{}", owner_fqn, span_key);

    let mut fd = FunDecl {
        span: sc.span,
        fqn: ctor_fqn.clone(),
        name: "$ctor".to_string(),
        ty: fn_ty,
        params: Vec::new(),
        return_ty: unit_ty,
        effect_row: effect_row.clone(),
        type_params: sc
            .type_params
            .as_ref()
            .map(|tp| tp.params.iter().map(|p| p.name.symbol).collect())
            .unwrap_or_default(),
        body: None,
        file: file_id,
        stable_template_key: None,
        instance_symbol: None,
        effect_abi: None,
        intrinsic_name: None,
    };

    let mut builder = FnLowering::new(
        hir, types, file_id, ctor_fqn, unit_ty, effect_row, errors,
    );

    // 分配 this local（首参）。
    let this_lid = builder.alloc_named("<this>".to_string(), this_ty, sc.span);
    builder.this_local = Some(this_lid);
    if let Some(this_sym) = hir.interner.get("this") {
        builder.symbol_locals.insert(this_sym, this_lid);
    }
    fd.params.push(crate::mir::Param {
        span: sc.span,
        name: "<this>".to_string(),
        ty: this_ty,
        local: this_lid,
    });

    // 分配 secondary ctor 参数 local。
    for (i, p) in sc.params.iter().enumerate() {
        let pty = sig_params.get(i).copied().unwrap_or_else(|| builder.types.nothing());
        let name_text = builder.hir.interner.resolve(p.name.symbol).to_string();
        let lid = builder.alloc_named(name_text.clone(), pty, p.name.span);
        builder.symbol_locals.insert(p.name.symbol, lid);
        fd.params.push(crate::mir::Param {
            span: p.name.span,
            name: name_text,
            ty: pty,
            local: lid,
        });
    }

    // primary ctor 参数布局 + 名字（供 emit_class_init_steps 发射 property-param 赋值）。
    let ctor_params: Vec<ClassCtorParamInfo> = hir
        .class_ctor_params
        .get(&owner_fqn_sym)
        .cloned()
        .unwrap_or_default();
    let primary_param_names: Vec<scoop2_base::Symbol> = d
        .primary_ctor
        .as_ref()
        .map(|pc| pc.params.iter().map(|p| p.name.symbol).collect())
        .unwrap_or_default();

    // ---- 执行 delegation + body ----
    let primary_init_fqn = format!("{}.$init", owner_fqn);
    match &sc.delegation {
        Some(del) => {
            // lower delegation 实参（含默认参数填充 + 命名实参排序）。
            // 目标 ctor 签名：this → 本类 ctor_signatures；super → 超类 ctor_signatures。
            let (target_fqn, target_sig_owner) = match del.kind {
                CtorDelegationKind::This => {
                    let n_args = del.args.len();
                    let fqn = resolve_this_delegation_target(hir, owner_fqn_sym, &owner_fqn, n_args);
                    (fqn, owner_fqn_sym)
                }
                CtorDelegationKind::Super => {
                    let sd = hir.super_ctor_delegations.get(&owner_fqn_sym);
                    let super_fqn_text = sd
                        .map(|sd| hir.interner.resolve(sd.super_fqn).to_string())
                        .unwrap_or_default();
                    let fqn = format!("{}.$init", super_fqn_text);
                    let super_sym = sd.map(|sd| sd.super_fqn).unwrap_or_default();
                    (fqn, super_sym)
                }
            };
            // 查目标 ctor 签名（按参数数匹配）。
            let target_sig = hir
                .ctor_signatures
                .get(&target_sig_owner)
                .and_then(|sigs| {
                    // 找参数数匹配（考虑默认值：n_args <= n_params）的签名。
                    let n_args = del.args.len();
                    sigs.iter().find(|s| {
                        let min_arity = s.has_defaults.iter().position(|d| *d).unwrap_or(s.param_types.len());
                        n_args >= min_arity && n_args <= s.param_types.len()
                    }).or_else(|| sigs.first())
                });
            // 构建实参列表（this + 填充后的参数）。
            let mut del_ops: Vec<crate::mir::CallArg> = Vec::new();
            del_ops.push(crate::mir::CallArg {
                name: None,
                is_spread: false,
                value: crate::mir::Operand::Local(this_lid),
                value_ty: this_ty,
            });
            let filled_args = lower_delegation_args(&mut builder, del, target_sig);
            del_ops.extend(filled_args);
            match del.kind {
                CtorDelegationKind::This => {
                    emit_plain_init_call(&mut builder, &target_fqn, del_ops, sc.span);
                }
                CtorDelegationKind::Super => {
                    emit_plain_init_call(&mut builder, &target_fqn, del_ops, sc.span);
                    // 本类 property-param + initializer + init-block（从 d.body.members 发射）。
                    emit_class_init_steps(&mut builder, d, hir, owner_fqn_sym, this_lid, this_ty, &ctor_params, &primary_param_names, &owner_fqn, sc.span);
                }
            }
        }
        None => {
            // 无 delegation：调 primary $init（只传 this，无额外实参）。
            let ops = vec![crate::mir::CallArg {
                name: None,
                is_spread: false,
                value: crate::mir::Operand::Local(this_lid),
                value_ty: this_ty,
            }];
            emit_plain_init_call(&mut builder, &primary_init_fqn, ops, sc.span);
        }
    }

    // lower secondary ctor body。
    let _ = crate::mir::lower::stmt::lower_block(&mut builder, &sc.body);

    let (mir_body, nested, types_out) = builder.finish();
    fd.body = Some(mir_body);
    Some((fd, nested, types_out))
}

/// 解析 `this(args)` 委托的目标 ctor callable FQN。
///
/// 按 delegation 实参数在 ctor_signatures 中找参数数匹配的 ctor：
/// - primary（ctors[0]，有 primary_ctor 时）→ `<Class>.$init`；
/// - secondary（ctors[i]，decl_span = constructor 关键字 span）→ `<Class>.$ctor.s<span.start>`。
fn resolve_this_delegation_target(
    hir: &TypedHir,
    owner_fqn_sym: scoop2_base::Symbol,
    owner_fqn: &str,
    n_args: usize,
) -> String {
    let primary_init_fqn = format!("{}.$init", owner_fqn);
    let Some(sigs) = hir.ctor_signatures.get(&owner_fqn_sym) else {
        return primary_init_fqn;
    };
    // 找参数数匹配（考虑默认值：args <= params）的 ctor。
    let has_primary = hir.class_ctor_params.contains_key(&owner_fqn_sym);
    for (i, sig) in sigs.iter().enumerate() {
        let applicable = n_args <= sig.param_types.len()
            && sig.param_types.len() - n_args <= sig.has_defaults.iter().skip(n_args).filter(|d| **d).count();
        if !applicable {
            continue;
        }
        if i == 0 && has_primary {
            // primary（ctors[0] 且类有 primary_ctor）。
            return primary_init_fqn;
        }
        // secondary：用 decl_span.start 构造 callable FQN。
        return format!("{}.$ctor.s{}", owner_fqn, sig.decl_span.start);
    }
    primary_init_fqn
}

/// 发射本类的 property-param 赋值 + property initializer + init block（不含 super 委托）。
/// 供 primary `$init`（在 super 之后）和 secondary `super(args)` 路径复用。
fn emit_class_init_steps(
    builder: &mut FnLowering,
    d: &scoop2_syntax::ast::TypeDecl,
    hir: &TypedHir,
    owner_fqn_sym: scoop2_base::Symbol,
    this_lid: LocalId,
    this_ty: TypeId,
    ctor_params: &[ClassCtorParamInfo],
    primary_param_names: &[scoop2_base::Symbol],
    owner_fqn: &str,
    span: scoop2_base::Span,
) {
    use scoop2_syntax::ast::TypeMemberKind;
    let body = match &d.body {
        Some(b) => b,
        None => return,
    };
    let mut emitted_param_props = false;
    let param_prop_assigns: Vec<(String, LocalId, TypeId)> = ctor_params
        .iter()
        .enumerate()
        .filter(|(_, cp)| cp.is_property)
        .filter_map(|(i, cp)| {
            let param_lid = primary_param_names
                .get(i)
                .and_then(|s| builder.symbol_locals.get(s).copied())
                .unwrap_or(this_lid);
            let field_name = builder.hir.interner.resolve(cp.name).to_string();
            Some((field_name, param_lid, cp.ty))
        })
        .collect();
    let mut emit_param_props = |builder: &mut FnLowering| {
        if emitted_param_props {
            return;
        }
        emitted_param_props = true;
        for (field_name, param_lid, field_ty) in &param_prop_assigns {
            builder.push_stmt(crate::mir::Statement {
                span,
                kind: crate::mir::StatementKind::StoreMember {
                    receiver: crate::mir::Operand::Local(this_lid),
                    member: builder.member_access_metadata(field_name, this_ty),
                    value: crate::mir::Operand::Local(*param_lid),
                    value_ty: *field_ty,
                    continuation_route:
                        crate::mir::transport::StoredContinuationRoutePublication::None,
                },
            });
        }
    };
    for m in &body.members {
        match &m.kind {
            TypeMemberKind::Property(p) => {
                if let Some(init_expr) = &p.init {
                    emit_param_props(builder);
                    let field_name = builder.hir.interner.resolve(p.name.symbol).to_string();
                    let val = crate::mir::lower::expr::lower_expr(builder, init_expr);
                    let field_ty = builder
                        .hir
                        .members
                        .get(&owner_fqn_sym)
                        .and_then(|mm| mm.get(&p.name.symbol))
                        .copied()
                        .unwrap_or_else(|| builder.types.nothing());
                    builder.push_stmt(crate::mir::Statement {
                        span: p.name.span,
                        kind: crate::mir::StatementKind::StoreMember {
                            receiver: crate::mir::Operand::Local(this_lid),
                            member: builder.member_access_metadata(&field_name, this_ty),
                            value: val,
                            value_ty: field_ty,
                            continuation_route:
                                crate::mir::transport::StoredContinuationRoutePublication::None,
                        },
                    });
                }
            }
            TypeMemberKind::InitBlock(ib) => {
                emit_param_props(builder);
                let _ = crate::mir::lower::stmt::lower_block(builder, &ib.body);
            }
            _ => {}
        }
    }
    emit_param_props(builder);
    let _ = owner_fqn;
}

/// lower delegation 实参（含默认参数填充 + 命名实参排序）。
///
/// 与 HIR 的 fill_resolved_args 对称：按目标 ctor 签名的参数位置，
/// 将命名实参映射到正确位置 + 填充默认值。
fn lower_delegation_args(
    builder: &mut FnLowering,
    del: &scoop2_syntax::ast::CtorDelegation,
    target_sig: Option<&scoop2_hir::hir::TypedSignature>,
) -> Vec<crate::mir::CallArg> {
    let args = &del.args;
    // 无签名或全位置实参（参数数 == 签名参数数）→ 直接 lower。
    let sig = match target_sig {
        Some(s) => s,
        None => {
            return args
                .iter()
                .map(|a| {
                    let v = crate::mir::lower::expr::lower_expr(builder, &a.value);
                    let ty = crate::mir::lower::stmt::operand_ty(builder, &v);
                    crate::mir::CallArg {
                        name: None,
                        is_spread: a.is_spread,
                        value: v,
                        value_ty: ty,
                    }
                })
                .collect();
        }
    };
    // 若全位置且参数数匹配 → 直接 lower（常见路径）。
    let all_positional = args.iter().all(|a| a.name.is_none());
    if all_positional && args.len() == sig.param_types.len() {
        return args
            .iter()
            .map(|a| {
                let v = crate::mir::lower::expr::lower_expr(builder, &a.value);
                let ty = crate::mir::lower::stmt::operand_ty(builder, &v);
                crate::mir::CallArg {
                    name: None,
                    is_spread: a.is_spread,
                    value: v,
                    value_ty: ty,
                }
            })
            .collect();
    }
    // 按签名参数位置排序 + 填充默认值。
    let n_params = sig.param_types.len();
    let mut out: Vec<crate::mir::CallArg> = Vec::with_capacity(n_params);
    let mut positional_iter = args
        .iter()
        .filter(|a| a.name.is_none())
        .map(|a| &a.value);
    for (param_idx, &pname) in sig.param_names.iter().enumerate() {
        // 先查命名实参。
        let named = args
            .iter()
            .find(|a| a.name.as_ref().is_some_and(|n| n.symbol == pname));
        if let Some(a) = named {
            let v = crate::mir::lower::expr::lower_expr(builder, &a.value);
            let ty = crate::mir::lower::stmt::operand_ty(builder, &v);
            out.push(crate::mir::CallArg {
                name: None,
                is_spread: false,
                value: v,
                value_ty: ty,
            });
        } else if let Some(expr) = positional_iter.next() {
            let v = crate::mir::lower::expr::lower_expr(builder, expr);
            let ty = crate::mir::lower::stmt::operand_ty(builder, &v);
            out.push(crate::mir::CallArg {
                name: None,
                is_spread: false,
                value: v,
                value_ty: ty,
            });
        } else {
            // 默认值。
            if let Some(Some(default_expr)) = sig.default_exprs.get(param_idx) {
                let v = crate::mir::lower::expr::lower_expr(builder, default_expr);
                let ty = crate::mir::lower::stmt::operand_ty(builder, &v);
                out.push(crate::mir::CallArg {
                    name: None,
                    is_spread: false,
                    value: v,
                    value_ty: ty,
                });
            }
            // body-field 参数（无默认表达式）→ 跳过（不传）。
        }
    }
    out
}

/// 发出一个对 `<Class>.$init` 的普通调用（this + 实参）。
fn emit_plain_init_call(
    builder: &mut FnLowering,
    callee_fqn: &str,
    args: Vec<crate::mir::CallArg>,
    span: scoop2_base::Span,
) {
    let unit_ty = builder.types.unit();
    let tmp = builder.alloc_temp(unit_ty, span);
    builder.push_stmt(crate::mir::Statement {
        span,
        kind: crate::mir::StatementKind::Assign {
            target: tmp,
            value: crate::mir::Rvalue::Call {
                site_id: None,
                kind: crate::mir::CallKind::Direct {
                    callee_fqn: callee_fqn.to_string(),
                    type_args: vec![],
                    is_intrinsic: false,
                    stable_template_key: None,
                    stable_instance_key: None,
                    generic_type_args: vec![],
                    generic_eff_args: vec![],
                    intrinsic_name: None,
                },
                args,
                transport: crate::mir::transport::CallTransportMetadata::plain_no_outward(
                    unit_ty,
                    crate::mir::transport::MirTransportKind::Scalar,
                ),
            },
        },
    });
}

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

/// 从 `@Intrinsic("name")` 注解中提取 intrinsic 名（无参 `@Intrinsic` 类型级注解
/// 不在此处理——它只标记「类型是内建标量」，具体方法名由方法级 `@Intrinsic("xxx")` 给出）。
///
/// 返回 None 表示：无 `@Intrinsic` 注解，或注解无字符串字面量实参（无参 `@Intrinsic`）。
fn extract_intrinsic_name(
    d: &scoop2_syntax::ast::FunDecl,
    hir: &TypedHir,
) -> Option<String> {
    use scoop2_syntax::ast::{AnnotationUse, ExprKind};
    let intrinsic_sym = hir.interner.get("Intrinsic")?;
    fn is_intrinsic_ann(ann: &AnnotationUse, intrinsic_sym: scoop2_base::Symbol) -> bool {
        ann.path
            .segments
            .last()
            .is_some_and(|s| s.symbol == intrinsic_sym)
    }
    let ann = d
        .annotations
        .iter()
        .find(|a| is_intrinsic_ann(a, intrinsic_sym))?;
    // 取首个位置实参（字符串字面量）：`@Intrinsic("int_plus")`。
    let arg = ann.args.iter().find(|a| a.name.is_none())?;
    if let ExprKind::StringLit(sl) = &arg.value.kind {
        Some(sl.value.clone())
    } else {
        None
    }
}
