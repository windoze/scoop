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
use crate::mir::{Operand, 
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

    /// 取某表达式的推断类型，缺失时回退到 Unit（非 Nothing）。
    /// 对合法程序不应触发（completeness gate 保证 expr_types 完整）。
    pub fn expr_ty(&mut self, expr_id: NodeId) -> TypeId {
        self.hir
            .expr_type(self.file_id, expr_id)
            .unwrap_or_else(|| self.types.unit())
    }

    /// Unit 类型。
    pub fn nothing_ty(&mut self) -> TypeId {
        self.types.unit()
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
        let array_fqn = self.hir.lang_items.array;
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
            let overload_sig =
                crate::mir::stable_id::build_overload_sig(&self.types, &self.hir.interner, pts);
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
            instance_symbol: None,
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
        // Option<T>：payload 为单个 inner 类型（走 FQN 判定）。
        let element_tys: Vec<TypeId> = if let Some(args) = self
            .types
            .nominal_args_of_fqn(aggregate_ty, self.types.option_fqn())
        {
            args.to_vec()
        } else {
            match self.types.kind(aggregate_ty) {
                TypeKind::Value(ValueTypeKind::Tuple(els)) => els.clone(),
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
            }
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

// ---------------------------------------------------------------------------
// operand / 类型工具（自 lower/stmt.rs 迁入——AST 路径删除后树路径的唯一入口）
// ---------------------------------------------------------------------------

/// 取一个 operand 的类型（best-effort）。
pub fn operand_ty_public(builder: &mut FnLowering, op: &Operand) -> scoop2_hir::ty::TypeId {
    operand_ty(builder, op)
}

/// 取一个 operand 的类型（best-effort）。
pub fn operand_ty(builder: &mut FnLowering, op: &Operand) -> scoop2_hir::ty::TypeId {
    match op {
        Operand::Local(l) => builder
            .body
            .locals
            .get(l.0 as usize)
            .map(|d| d.ty)
            .unwrap_or_else(|| builder.types.unit()),
        Operand::Const(c) => const_ty(builder, c),
    }
}

fn const_ty(builder: &mut FnLowering, c: &crate::mir::ConstValue) -> scoop2_hir::ty::TypeId {
    match c {
        crate::mir::ConstValue::Bool(_) => builder.types.bool(),
        crate::mir::ConstValue::Char(_) => builder.types.char(),
        crate::mir::ConstValue::Unit => builder.types.unit(),
        crate::mir::ConstValue::Int(_, _) => builder.types.int(),
        crate::mir::ConstValue::Float(_, None) => builder.types.float64(),
        crate::mir::ConstValue::Float(_, Some(_)) => builder.types.float32(),
        crate::mir::ConstValue::String(_) => builder.types.string(),
        crate::mir::ConstValue::Null => builder.types.any(),
    }
}

/// 类型 → 其方法分发的 owner FQN Symbol。
///
/// 覆盖内建类型（Bool/Char/Int*/UInt*/Float*/String）与 nominal
/// class/struct/interface。无法静态确定 owner（Any/Function/Union/类型参数等）
/// 时返回 `Symbol::default()`。注意 `Symbol::default()` 可能 resolve 出无关
/// 字符串（它是真实 Symbol(0)），调用方不得把它当作有效 owner 使用。
pub fn owner_fqn_of_type(builder: &FnLowering, ty: scoop2_hir::ty::TypeId) -> scoop2_base::Symbol {
    use scoop2_hir::ty::{RefTypeKind, TypeKind, ValueTypeKind};
    match builder.types.kind(ty) {
        TypeKind::Ref(RefTypeKind::Nominal(n)) => n.fqn,
        TypeKind::Value(ValueTypeKind::Nominal(n)) => n.fqn,
        TypeKind::Value(ValueTypeKind::Bool) => builder.hir.lang_items.bool_,
        TypeKind::Value(ValueTypeKind::Char) => builder.hir.lang_items.char_,
        TypeKind::Value(ValueTypeKind::Int) => builder.hir.lang_items.int,
        TypeKind::Value(ValueTypeKind::UInt) => builder.hir.lang_items.uint,
        TypeKind::Value(ValueTypeKind::IntN(bits)) => builder
            .hir
            .interner
            .get(&format!("scoop.core.Int{bits}"))
            .unwrap_or_default(),
        TypeKind::Value(ValueTypeKind::UIntN(bits)) => builder
            .hir
            .interner
            .get(&format!("scoop.core.UInt{bits}"))
            .unwrap_or_default(),
        TypeKind::Value(ValueTypeKind::Float32) => builder.hir.lang_items.float32,
        TypeKind::Value(ValueTypeKind::Float64) => builder.hir.lang_items.float64,
        _ => scoop2_base::Symbol::default(),
    }
}

/// 从 operand 的类型解析 owner FQN Symbol（用于区分 interface vs class 分发）。
///
/// 取 operand 的类型 → [`owner_fqn_of_type`]（含内建类型与常量 operand）。
/// 无法解析时返回 `Symbol::default()`。
pub fn resolve_owner_fqn_from_operand(
    builder: &FnLowering,
    op: &Operand,
) -> scoop2_base::Symbol {
    let ty = match op {
        Operand::Local(l) => builder.body.locals.get(l.0 as usize).map(|d| d.ty),
        // 常量 operand：按常量种类映射到内建 owner（lang-items 句柄）。
        Operand::Const(c) => {
            return match c {
                crate::mir::ConstValue::String(_) => builder.hir.lang_items.string,
                crate::mir::ConstValue::Bool(_) => builder.hir.lang_items.bool_,
                crate::mir::ConstValue::Char(_) => builder.hir.lang_items.char_,
                crate::mir::ConstValue::Int(_, _) => builder.hir.lang_items.int,
                crate::mir::ConstValue::Float(_, None) => builder.hir.lang_items.float64,
                crate::mir::ConstValue::Float(_, Some(_)) => builder.hir.lang_items.float32,
                crate::mir::ConstValue::Unit | crate::mir::ConstValue::Null => {
                    scoop2_base::Symbol::default()
                }
            };
        }
    };
    let Some(ty) = ty else {
        return scoop2_base::Symbol::default();
    };
    owner_fqn_of_type(builder, ty)
}

/// 整型字面量后缀（树枚举 → MIR 枚举）。
pub(crate) fn suffix_of(
    s: &Option<scoop2_hir::hir::tree::TreeIntSuffix>,
) -> Option<crate::mir::IntSuffix> {
    s.map(|s| match s {
        scoop2_hir::hir::tree::TreeIntSuffix::U => crate::mir::IntSuffix::U,
        scoop2_hir::hir::tree::TreeIntSuffix::L => crate::mir::IntSuffix::L,
        scoop2_hir::hir::tree::TreeIntSuffix::Ul => crate::mir::IntSuffix::UL,
    })
}
