use super::*;
use crate::HirExpressionTypeRoleV1 as Role;

impl Expr {
    /// A machine operation whose signature or receiver needs shared declarations.
    pub fn shared_representation_type(&self, types: &Arena<Type>) -> Option<TypeId> {
        match &self.kind {
            ExprKind::Call {
                callee: CallableTarget::Imported(_),
                receiver: crate::SourceCallReceiver::Receiver { static_type },
                ..
            } => Some(*static_type),
            ExprKind::Lambda(_)
            | ExprKind::AnonymousFunction(_)
            | ExprKind::CallableReference(_)
            | ExprKind::FunctionCoercion { .. } => Some(self.ty),
            ExprKind::IsInstance { check_ty, .. } | ExprKind::Cast { check_ty, .. }
                if matches!(types[*check_ty].kind, TypeKind::Function(_)) =>
            {
                Some(*check_ty)
            }
            _ => None,
        }
    }

    /// All exact types named at this expression position. Child expressions
    /// have separate positions in the common executable traversal.
    pub fn type_uses(&self) -> impl Iterator<Item = (Role, TypeId)> {
        let operand = match &self.kind {
            ExprKind::SingletonValue(_) => Some((Role::SingletonValue, self.ty)),
            ExprKind::SizeOf(ty) => Some((Role::SizeOf, *ty)),
            ExprKind::AlignOf(ty) => Some((Role::AlignOf, *ty)),
            ExprKind::IsInstance { check_ty, .. } | ExprKind::Cast { check_ty, .. } => {
                Some((Role::TypeTest, *check_ty))
            }
            ExprKind::Box(operand) => Some((Role::BoxedValue, operand.ty)),
            ExprKind::Unbox(_) => Some((Role::BoxedValue, self.ty)),
            ExprKind::ArrayAssembly(assembly) => Some((Role::ArrayElement, assembly.element_type)),
            _ => None,
        };
        std::iter::once((Role::Value, self.ty)).chain(operand)
    }
}
