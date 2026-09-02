use super::*;

impl Lowerer {
    pub(crate) fn array_type(&mut self, kind: ArrayKind, element: TypeId) -> TypeId {
        let intrinsic = match kind {
            ArrayKind::Immutable => hir::IntrinsicTypeKind::Array,
            ArrayKind::Mutable => hir::IntrinsicTypeKind::MutableArray,
        };
        let &(owner, _) = self
            .intrinsic_type_owners
            .get(&intrinsic)
            .expect("the intrinsic core contract is validated before type resolution");
        let IntrinsicTypeOwner::Class(template) = owner else {
            unreachable!("the intrinsic registry fixes array declarations as classes")
        };
        self.class_application(template, vec![element])
    }

    /// Return the exact array family application carried by a class type.
    /// Ordinary classes and fixed intrinsic classes return `None`.
    pub(crate) fn array_type_info(&self, ty: TypeId) -> Option<ArrayType> {
        let Type::Class(application) = self.types[ty] else {
            return None;
        };
        match self.class_applications[application].representation {
            hir::ClassApplicationRepresentation::Intrinsic(
                hir::IntrinsicTypeRepresentation::Array { element },
            ) => Some(ArrayType {
                kind: ArrayKind::Immutable,
                element,
            }),
            hir::ClassApplicationRepresentation::Intrinsic(
                hir::IntrinsicTypeRepresentation::MutableArray { element },
            ) => Some(ArrayType {
                kind: ArrayKind::Mutable,
                element,
            }),
            hir::ClassApplicationRepresentation::Declared
            | hir::ClassApplicationRepresentation::Intrinsic(
                hir::IntrinsicTypeRepresentation::Int
                | hir::IntrinsicTypeRepresentation::UInt
                | hir::IntrinsicTypeRepresentation::Boolean
                | hir::IntrinsicTypeRepresentation::String,
            ) => None,
        }
    }

    pub(crate) fn array_element_ty(&self, ty: TypeId) -> Option<TypeId> {
        self.array_type_info(ty).map(|array| array.element)
    }
}
