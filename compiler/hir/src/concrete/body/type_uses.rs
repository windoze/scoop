use super::*;
use crate::HirExpressionTypeRoleV1 as Role;

impl Expr {
    /// All exact types named at this expression position. Child expressions
    /// have separate positions in the common executable traversal.
    pub fn type_uses(&self) -> impl Iterator<Item = (Role, TypeId)> {
        let operand = match &self.kind {
            ExprKind::SizeOf(ty) => Some((Role::SizeOf, *ty)),
            ExprKind::AlignOf(ty) => Some((Role::AlignOf, *ty)),
            ExprKind::IsInstance { check_ty, .. } => Some((Role::TypeTest, *check_ty)),
            ExprKind::ArrayAssembly(assembly) => Some((Role::ArrayElement, assembly.element_type)),
            _ => None,
        };
        std::iter::once((Role::Value, self.ty)).chain(operand)
    }
}
