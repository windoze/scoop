//! Type inference（类型推断）基础设施：约束表示 + 最小求解器骨架。
//!
//! 对齐 TODO：T0501（spec §14.9）。
//!
//! 当前阶段目标：
//! - 表达“相等约束”（`τ1 = τ2`）与“子类型约束”（`τ1 <: τ2`，先只做数据结构占位）
//! - 支持推断变量（inference variables）
//! - 提供最小可用的 unify：能把变量绑定到具体 `TypeId`，并在冲突时返回错误
//!
//! 非目标（后续任务逐步补齐）：
//! - 完整 Kotlin-like subtyping（当前只覆盖 T0506 的最小子集）
//! - LUB、lambda expected type 传播、泛型实参推断等（见 T0502+）

use crate::ty::{BuiltinTypes, RefTypeKind, TypeId, TypeKind, TypeStore, ValueTypeKind};
use thiserror::Error;

/// 推断变量的 ID（仅在一次推断会话内有效）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct InferVarId(u32);

impl InferVarId {
    pub fn as_u32(self) -> u32 {
        self.0
    }
}

/// 推断阶段使用的“类型项”。
///
/// 说明：
/// - 当前阶段只区分“已知类型 `TypeId`”与“未知推断变量”；
/// - 后续若需要让 constraint 能表达结构类型内部的未知量，可扩展为更丰富的 `InferTypeKind`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InferTerm {
    Ty(TypeId),
    Var(InferVarId),
}

/// 推断约束。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Constraint {
    /// `τ1 = τ2`
    Eq(InferTerm, InferTerm),
    /// `τ1 <: τ2`
    Subtype(InferTerm, InferTerm),
}

/// 推断错误（当前阶段只覆盖相等约束求解）。
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum InferError {
    #[error("type conflict: {left:?} vs {right:?}")]
    TypeConflict { left: TypeId, right: TypeId },

    #[error("type is not a subtype: {sub:?} <: {sup:?}")]
    SubtypeNotSatisfied { sub: TypeId, sup: TypeId },

    #[error("inference bounds are incompatible: {lower:?} !<: {upper:?}")]
    IncompatibleBounds { lower: TypeId, upper: TypeId },

    #[error("unsupported constraint: {constraint:?}")]
    UnsupportedConstraint { constraint: Constraint },
}

/// 求解器：收集约束并求解。
///
/// 当前阶段实现的是“相等约束 + union-find + 可选的 concrete binding”：
/// - `T = Int` 会把 `T` 绑定到 `Int`
/// - `T = Int` 且 `T = String` 会产生 `TypeConflict`
#[derive(Debug, Default)]
pub struct Solver {
    vars: Vec<VarState>,
    constraints: Vec<Constraint>,
}

#[derive(Debug, Clone)]
struct VarState {
    parent: InferVarId,
    rank: u8,
    binding: Option<TypeId>,
    lower_bounds: Vec<TypeId>,
    upper_bounds: Vec<TypeId>,
    /// `self <: edge`。
    subtype_out: Vec<InferVarId>,
}

impl Solver {
    pub fn new() -> Self {
        Self::default()
    }

    /// 创建一个新的推断变量。
    pub fn new_var(&mut self) -> InferVarId {
        let id = InferVarId(u32::try_from(self.vars.len()).expect("too many inference variables"));
        self.vars.push(VarState {
            parent: id,
            rank: 0,
            binding: None,
            lower_bounds: Vec::new(),
            upper_bounds: Vec::new(),
            subtype_out: Vec::new(),
        });
        id
    }

    /// 追加一个约束。
    pub fn add_constraint(&mut self, constraint: Constraint) {
        self.constraints.push(constraint);
    }

    /// 追加一个相等约束：`a = b`。
    pub fn eq(&mut self, a: InferTerm, b: InferTerm) {
        self.add_constraint(Constraint::Eq(a, b));
    }

    /// 追加一个子类型约束：`a <: b`。
    pub fn subtype(&mut self, a: InferTerm, b: InferTerm) {
        self.add_constraint(Constraint::Subtype(a, b));
    }

    /// 执行求解。
    ///
    /// 当前阶段（T0506）只支持一小部分子类型规则：
    /// - `Nothing <: T`
    /// - `T <: Any`
    /// - `Option<T>`、tuple、function 的结构性子类型递归（含函数参数逆变、返回协变、effect subset）
    pub fn solve(&mut self, types: &TypeStore, builtins: BuiltinTypes) -> Result<(), InferError> {
        let constraints = std::mem::take(&mut self.constraints);
        for constraint in constraints {
            match constraint {
                Constraint::Eq(a, b) => self.unify(a, b)?,
                Constraint::Subtype(a, b) => self.add_subtype_constraint(a, b, types, builtins)?,
            }
        }

        self.propagate_subtype_edges();
        self.finalize_bindings(types, builtins)?;
        Ok(())
    }

    /// 查询变量在当前解中的绑定（若已绑定到具体类型）。
    pub fn binding_of(&mut self, var: InferVarId) -> Option<TypeId> {
        let root = self.find(var);
        self.vars[root.0 as usize].binding
    }

    fn add_subtype_constraint(
        &mut self,
        sub: InferTerm,
        sup: InferTerm,
        types: &TypeStore,
        builtins: BuiltinTypes,
    ) -> Result<(), InferError> {
        match (sub, sup) {
            (InferTerm::Ty(sub), InferTerm::Ty(sup)) => {
                if is_subtype_of(sub, sup, types, builtins) {
                    Ok(())
                } else {
                    Err(InferError::SubtypeNotSatisfied { sub, sup })
                }
            }
            (InferTerm::Var(var), InferTerm::Ty(sup)) => {
                let root = self.find(var);
                push_unique(&mut self.vars[root.0 as usize].upper_bounds, sup);
                Ok(())
            }
            (InferTerm::Ty(sub), InferTerm::Var(var)) => {
                let root = self.find(var);
                push_unique(&mut self.vars[root.0 as usize].lower_bounds, sub);
                Ok(())
            }
            (InferTerm::Var(a), InferTerm::Var(b)) => {
                let root_a = self.find(a);
                let root_b = self.find(b);
                if root_a == root_b {
                    return Ok(());
                }
                push_unique_var(&mut self.vars[root_a.0 as usize].subtype_out, root_b);
                Ok(())
            }
        }
    }

    fn propagate_subtype_edges(&mut self) {
        // 固定点传播：
        // - `a <: b` 使得 `a` 继承 `b` 的 upper bounds
        // - `a <: b` 使得 `b` 继承 `a` 的 lower bounds
        //
        // 当前阶段 edges 数量很小，因此用朴素 fixed-point 即可。
        loop {
            let mut changed = false;

            let mut roots: Vec<InferVarId> = Vec::new();
            for id in 0..self.vars.len() {
                let v = InferVarId(id as u32);
                if self.find(v) == v {
                    roots.push(v);
                }
            }

            for a in roots {
                let a_idx = a.0 as usize;
                let edges = self.vars[a_idx].subtype_out.clone();
                for raw_b in edges {
                    let b = self.find(raw_b);
                    if a == b {
                        continue;
                    }
                    let b_idx = b.0 as usize;

                    let (a_state, b_state) = self.state_pair_mut(a_idx, b_idx);

                    // a.upper += b.upper + b.binding
                    let b_upper = b_state.upper_bounds.clone();
                    for ub in b_upper {
                        if push_unique(&mut a_state.upper_bounds, ub) {
                            changed = true;
                        }
                    }
                    if let Some(ub) = b_state.binding {
                        if push_unique(&mut a_state.upper_bounds, ub) {
                            changed = true;
                        }
                    }

                    // b.lower += a.lower + a.binding
                    let a_lower = a_state.lower_bounds.clone();
                    for lb in a_lower {
                        if push_unique(&mut b_state.lower_bounds, lb) {
                            changed = true;
                        }
                    }
                    if let Some(lb) = a_state.binding {
                        if push_unique(&mut b_state.lower_bounds, lb) {
                            changed = true;
                        }
                    }
                }
            }

            if !changed {
                break;
            }
        }
    }

    fn finalize_bindings(&mut self, types: &TypeStore, builtins: BuiltinTypes) -> Result<(), InferError> {
        let mut roots: Vec<InferVarId> = Vec::new();
        for id in 0..self.vars.len() {
            let v = InferVarId(id as u32);
            if self.find(v) == v {
                roots.push(v);
            }
        }

        for root in roots {
            let idx = root.0 as usize;

            // 先根据 bounds 选一个 binding（若 아직没有）。
            if self.vars[idx].binding.is_none() {
                let candidate = if !self.vars[idx].lower_bounds.is_empty() {
                    let mut cur = self.vars[idx].lower_bounds[0];
                    for lb in self.vars[idx].lower_bounds.iter().copied().skip(1) {
                        cur = lub(cur, lb, types, builtins);
                    }
                    Some(cur)
                } else if !self.vars[idx].upper_bounds.is_empty() {
                    let mut cur = self.vars[idx].upper_bounds[0];
                    for ub in self.vars[idx].upper_bounds.iter().copied().skip(1) {
                        cur = glb(cur, ub, types, builtins);
                    }
                    Some(cur)
                } else {
                    None
                };

                if let Some(ty) = candidate {
                    self.vars[idx].binding = Some(ty);
                }
            }

            let Some(binding) = self.vars[idx].binding else {
                continue;
            };

            for lb in self.vars[idx].lower_bounds.clone() {
                if !is_subtype_of(lb, binding, types, builtins) {
                    return Err(InferError::IncompatibleBounds {
                        lower: lb,
                        upper: binding,
                    });
                }
            }
            for ub in self.vars[idx].upper_bounds.clone() {
                if !is_subtype_of(binding, ub, types, builtins) {
                    return Err(InferError::IncompatibleBounds {
                        lower: binding,
                        upper: ub,
                    });
                }
            }
        }

        Ok(())
    }

    fn state_pair_mut(&mut self, a: usize, b: usize) -> (&mut VarState, &mut VarState) {
        assert_ne!(a, b);
        if a < b {
            let (left, right) = self.vars.split_at_mut(b);
            (&mut left[a], &mut right[0])
        } else {
            let (left, right) = self.vars.split_at_mut(a);
            (&mut right[0], &mut left[b])
        }
    }

    fn unify(&mut self, a: InferTerm, b: InferTerm) -> Result<(), InferError> {
        match (a, b) {
            (InferTerm::Ty(left), InferTerm::Ty(right)) => {
                if left == right {
                    Ok(())
                } else {
                    Err(InferError::TypeConflict { left, right })
                }
            }
            (InferTerm::Var(var), InferTerm::Ty(ty)) | (InferTerm::Ty(ty), InferTerm::Var(var)) => {
                self.unify_var_with_type(var, ty)
            }
            (InferTerm::Var(a), InferTerm::Var(b)) => self.unify_vars(a, b),
        }
    }

    fn unify_var_with_type(&mut self, var: InferVarId, ty: TypeId) -> Result<(), InferError> {
        let root = self.find(var);
        let state = &mut self.vars[root.0 as usize];
        if let Some(bound) = state.binding {
            if bound == ty {
                return Ok(());
            }
            return Err(InferError::TypeConflict {
                left: bound,
                right: ty,
            });
        }
        state.binding = Some(ty);
        Ok(())
    }

    fn unify_vars(&mut self, a: InferVarId, b: InferVarId) -> Result<(), InferError> {
        let mut root_a = self.find(a);
        let mut root_b = self.find(b);
        if root_a == root_b {
            return Ok(());
        }

        // union-by-rank：保持路径压缩效率。
        let rank_a = self.vars[root_a.0 as usize].rank;
        let rank_b = self.vars[root_b.0 as usize].rank;
        if rank_a < rank_b {
            std::mem::swap(&mut root_a, &mut root_b);
        }

        let binding_a = self.vars[root_a.0 as usize].binding;
        let binding_b = self.vars[root_b.0 as usize].binding;
        if let (Some(left), Some(right)) = (binding_a, binding_b) {
            if left != right {
                return Err(InferError::TypeConflict { left, right });
            }
        }

        // 把 b 挂到 a 上，并合并 binding。
        self.vars[root_b.0 as usize].parent = root_a;
        if rank_a == rank_b {
            self.vars[root_a.0 as usize].rank = rank_a.saturating_add(1);
        }
        if binding_a.is_none() {
            self.vars[root_a.0 as usize].binding = binding_b;
        }

        // 合并 bounds 与 edges（保守：直接拼接，后续在 finalize 时再做一致性检查）。
        let lower_bounds_b = std::mem::take(&mut self.vars[root_b.0 as usize].lower_bounds);
        for lb in lower_bounds_b {
            push_unique(&mut self.vars[root_a.0 as usize].lower_bounds, lb);
        }
        let upper_bounds_b = std::mem::take(&mut self.vars[root_b.0 as usize].upper_bounds);
        for ub in upper_bounds_b {
            push_unique(&mut self.vars[root_a.0 as usize].upper_bounds, ub);
        }
        let edges_b = std::mem::take(&mut self.vars[root_b.0 as usize].subtype_out);
        for e in edges_b {
            push_unique_var(&mut self.vars[root_a.0 as usize].subtype_out, e);
        }

        Ok(())
    }

    fn find(&mut self, var: InferVarId) -> InferVarId {
        let idx = var.0 as usize;
        let parent = self.vars[idx].parent;
        if parent == var {
            return var;
        }
        let root = self.find(parent);
        self.vars[idx].parent = root;
        root
    }
}

fn push_unique(into: &mut Vec<TypeId>, ty: TypeId) -> bool {
    if into.contains(&ty) {
        return false;
    }
    into.push(ty);
    true
}

fn push_unique_var(into: &mut Vec<InferVarId>, var: InferVarId) -> bool {
    if into.contains(&var) {
        return false;
    }
    into.push(var);
    true
}

fn lub(a: TypeId, b: TypeId, types: &TypeStore, builtins: BuiltinTypes) -> TypeId {
    if is_subtype_of(a, b, types, builtins) {
        return b;
    }
    if is_subtype_of(b, a, types, builtins) {
        return a;
    }
    builtins.any
}

fn glb(a: TypeId, b: TypeId, types: &TypeStore, builtins: BuiltinTypes) -> TypeId {
    if is_subtype_of(a, b, types, builtins) {
        return a;
    }
    if is_subtype_of(b, a, types, builtins) {
        return b;
    }
    builtins.nothing
}

fn is_subtype_of(sub: TypeId, sup: TypeId, types: &TypeStore, builtins: BuiltinTypes) -> bool {
    if sub == sup {
        return true;
    }

    // `Nothing <: T`（bottom type）
    if sub == builtins.nothing {
        return true;
    }

    // `T <: Any`（top type；当前阶段 value types 通过 boxing 视为可上转）
    if sup == builtins.any {
        return true;
    }

    match (types.kind(sub), types.kind(sup)) {
        (
            TypeKind::Value(ValueTypeKind::Option(sub_inner)),
            TypeKind::Value(ValueTypeKind::Option(sup_inner)),
        ) => is_subtype_of(*sub_inner, *sup_inner, types, builtins),
        (
            TypeKind::Value(ValueTypeKind::Tuple(sub_elems)),
            TypeKind::Value(ValueTypeKind::Tuple(sup_elems)),
        ) => {
            if sub_elems.len() != sup_elems.len() {
                return false;
            }
            sub_elems
                .iter()
                .copied()
                .zip(sup_elems.iter().copied())
                .all(|(a, b)| is_subtype_of(a, b, types, builtins))
        }
        (
            TypeKind::Ref(RefTypeKind::Function(sub_fun)),
            TypeKind::Ref(RefTypeKind::Function(sup_fun)),
        ) => {
            if !sub_fun.effects.is_subset_of(&sup_fun.effects) {
                return false;
            }

            if !is_subtype_of(sub_fun.return_ty, sup_fun.return_ty, types, builtins) {
                return false;
            }

            if sub_fun.receiver.is_some() != sup_fun.receiver.is_some() {
                return false;
            }
            if sub_fun.params.len() != sup_fun.params.len() {
                return false;
            }

            // receiver function type：把 receiver 当作第一个参数参与逆变比较。
            if let (Some(sup_recv), Some(sub_recv)) = (sup_fun.receiver, sub_fun.receiver) {
                if !is_subtype_of(sup_recv, sub_recv, types, builtins) {
                    return false;
                }
            }

            for (sup_param, sub_param) in sup_fun
                .params
                .iter()
                .copied()
                .zip(sub_fun.params.iter().copied())
            {
                if !is_subtype_of(sup_param, sub_param, types, builtins) {
                    return false;
                }
            }

            true
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ty::{EffectRow, TypeStore};

    #[test]
    fn unify_detects_conflict_when_var_is_bound_to_two_different_types() {
        let mut tys = TypeStore::new();
        let builtins = tys.intern_builtins();

        let mut solver = Solver::new();
        let t = solver.new_var();

        solver.eq(InferTerm::Var(t), InferTerm::Ty(builtins.int));
        solver.eq(InferTerm::Var(t), InferTerm::Ty(builtins.string));

        let err = solver.solve(&tys, builtins).unwrap_err();
        assert!(matches!(err, InferError::TypeConflict { .. }));
    }

    #[test]
    fn subtype_constraint_allows_value_to_any_but_rejects_any_to_value() {
        let mut tys = TypeStore::new();
        let builtins = tys.intern_builtins();

        let mut ok = Solver::new();
        ok.subtype(InferTerm::Ty(builtins.int), InferTerm::Ty(builtins.any));
        ok.solve(&tys, builtins).expect("Int <: Any should hold");

        let mut err = Solver::new();
        err.subtype(InferTerm::Ty(builtins.any), InferTerm::Ty(builtins.int));
        let e = err.solve(&tys, builtins).unwrap_err();
        assert!(matches!(e, InferError::SubtypeNotSatisfied { .. }));
    }

    #[test]
    fn subtype_bounds_pick_lub_for_lower_bounds() {
        let mut tys = TypeStore::new();
        let builtins = tys.intern_builtins();

        let mut solver = Solver::new();
        let t = solver.new_var();

        // Int <: T, Bool <: T  =>  T = Any（最小实现：无共同超类型时退化为 Any）
        solver.subtype(InferTerm::Ty(builtins.int), InferTerm::Var(t));
        solver.subtype(InferTerm::Ty(builtins.bool_), InferTerm::Var(t));

        solver.solve(&tys, builtins).unwrap();
        assert_eq!(solver.binding_of(t), Some(builtins.any));
    }

    #[test]
    fn subtype_supports_option_tuple_and_function_structures() {
        let mut tys = TypeStore::new();
        let builtins = tys.intern_builtins();

        // Option：covariant
        let opt_int = tys.ty_option(builtins.int);
        let opt_any = tys.ty_option(builtins.any);
        let mut ok = Solver::new();
        ok.subtype(InferTerm::Ty(opt_int), InferTerm::Ty(opt_any));
        ok.solve(&tys, builtins).unwrap();

        let mut err = Solver::new();
        err.subtype(InferTerm::Ty(opt_any), InferTerm::Ty(opt_int));
        assert!(matches!(
            err.solve(&tys, builtins).unwrap_err(),
            InferError::SubtypeNotSatisfied { .. }
        ));

        // tuple：逐元素协变
        let tup1 = tys.ty_tuple(vec![builtins.int, builtins.bool_]);
        let tup2 = tys.ty_tuple(vec![builtins.any, builtins.any]);
        let mut ok = Solver::new();
        ok.subtype(InferTerm::Ty(tup1), InferTerm::Ty(tup2));
        ok.solve(&tys, builtins).unwrap();

        // function：参数逆变、返回协变
        let f_sub = tys.ty_function(
            None,
            vec![builtins.any],
            builtins.int,
            EffectRow::pure(),
            false,
        );
        let f_sup = tys.ty_function(
            None,
            vec![builtins.int],
            builtins.any,
            EffectRow::pure(),
            false,
        );
        let mut ok = Solver::new();
        ok.subtype(InferTerm::Ty(f_sub), InferTerm::Ty(f_sup));
        ok.solve(&tys, builtins).unwrap();
    }
}
