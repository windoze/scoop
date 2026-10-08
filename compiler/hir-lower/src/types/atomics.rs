use super::*;

impl Lowerer {
    pub(crate) fn atomic_value_type(
        &mut self,
        ty: TypeId,
    ) -> Option<(hir::AtomicValueKind, TypeId)> {
        let application = self.nominal_application(ty)?;
        let hir::IntrinsicTypeKind::Atomic(kind) =
            self.nominal_intrinsic_kind(application.template)?
        else {
            return None;
        };
        let value = match kind {
            hir::AtomicValueKind::Int => {
                self.intern_type(Type::Integer(hir::IntegerKind::SIGNED_32))
            }
            hir::AtomicValueKind::Long => {
                self.intern_type(Type::Integer(hir::IntegerKind::SIGNED_64))
            }
            hir::AtomicValueKind::Boolean => self.intern_type(Type::Boolean),
            hir::AtomicValueKind::Reference => {
                let [value] = application.arguments.as_slice() else {
                    unreachable!("an AtomicRef application has one reference argument")
                };
                *value
            }
        };
        Some((kind, value))
    }
}
