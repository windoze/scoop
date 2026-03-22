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
//! - 真实 subtyping 求解（见 T0506）
//! - LUB、lambda expected type 传播、泛型实参推断等（见 T0502+）

use crate::ty::TypeId;
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
    /// `τ1 <: τ2`（当前阶段仅占位，不参与求解）
    Subtype(InferTerm, InferTerm),
}

/// 推断错误（当前阶段只覆盖相等约束求解）。
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum InferError {
    #[error("type conflict: {left:?} vs {right:?}")]
    TypeConflict { left: TypeId, right: TypeId },

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

    /// 执行求解。
    ///
    /// 注意：当前阶段遇到 `Subtype` 会直接返回 `UnsupportedConstraint`。
    pub fn solve(&mut self) -> Result<(), InferError> {
        let constraints = std::mem::take(&mut self.constraints);
        for constraint in constraints {
            match constraint {
                Constraint::Eq(a, b) => self.unify(a, b)?,
                Constraint::Subtype(..) => {
                    return Err(InferError::UnsupportedConstraint { constraint });
                }
            }
        }
        Ok(())
    }

    /// 查询变量在当前解中的绑定（若已绑定到具体类型）。
    pub fn binding_of(&mut self, var: InferVarId) -> Option<TypeId> {
        let root = self.find(var);
        self.vars[root.0 as usize].binding
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ty::TypeStore;

    #[test]
    fn unify_detects_conflict_when_var_is_bound_to_two_different_types() {
        let mut tys = TypeStore::new();
        let builtins = tys.intern_builtins();

        let mut solver = Solver::new();
        let t = solver.new_var();

        solver.eq(InferTerm::Var(t), InferTerm::Ty(builtins.int));
        solver.eq(InferTerm::Var(t), InferTerm::Ty(builtins.string));

        let err = solver.solve().unwrap_err();
        assert!(matches!(err, InferError::TypeConflict { .. }));
    }
}

