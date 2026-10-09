use super::*;

impl Lowerer {
    pub(crate) fn maybe_uninit_value_type(&self, ty: TypeId) -> Option<TypeId> {
        let Type::Struct(application) = self.types[ty] else {
            return None;
        };
        match self.struct_applications[application].representation {
            hir::StructApplicationRepresentation::Intrinsic(
                hir::IntrinsicTypeRepresentation::MaybeUninit { value },
            ) => Some(value),
            _ => None,
        }
    }
}
