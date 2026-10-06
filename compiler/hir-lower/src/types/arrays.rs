use super::*;

impl Lowerer {
    pub(crate) fn array_class(&self, kind: ArrayKind) -> hir::ClassId {
        let intrinsic = match kind {
            ArrayKind::Immutable => hir::IntrinsicTypeKind::Array,
            ArrayKind::Mutable => hir::IntrinsicTypeKind::MutableArray,
        };
        let &(owner, _) = self
            .intrinsic_type_owners
            .get(&intrinsic)
            .expect("the intrinsic core contract is validated before type resolution");
        let IntrinsicTypeOwner::Class(class) = owner else {
            unreachable!("the intrinsic registry fixes array declarations as classes")
        };
        class
    }

    pub(crate) fn array_class_kind(&self, class: hir::ClassId) -> Option<ArrayKind> {
        match self.classes[class].representation {
            hir::ClassRepresentation::Intrinsic(hir::IntrinsicTypeKind::Array) => {
                Some(ArrayKind::Immutable)
            }
            hir::ClassRepresentation::Intrinsic(hir::IntrinsicTypeKind::MutableArray) => {
                Some(ArrayKind::Mutable)
            }
            hir::ClassRepresentation::Declared
            | hir::ClassRepresentation::Intrinsic(
                hir::IntrinsicTypeKind::Unit
                | hir::IntrinsicTypeKind::Integer(_)
                | hir::IntrinsicTypeKind::Char
                | hir::IntrinsicTypeKind::Boolean
                | hir::IntrinsicTypeKind::String
                | hir::IntrinsicTypeKind::Ptr
                | hir::IntrinsicTypeKind::FunPtr,
            ) => None,
        }
    }

    pub(crate) fn array_type(&mut self, kind: ArrayKind, element: TypeId) -> TypeId {
        if let crate::CoreLoweringAuthority::Imported(protocols) = &self.core {
            let fundamental = protocols.fundamental_types();
            let owner = match kind {
                ArrayKind::Immutable => fundamental.array(),
                ArrayKind::Mutable => fundamental.mutable_array(),
            };
            return self
                .imported_nominal_application(
                    hir::SourceNominalId::GenericTemplate(owner.persistent()),
                    vec![element],
                )
                .expect("the checked array protocol has a complete dependency declaration");
        }
        let template = self.array_class(kind);
        self.class_application(template, vec![element])
    }

    /// Return the exact array family application carried by a class type.
    /// Ordinary classes and fixed intrinsic classes return `None`.
    pub(crate) fn array_type_info(&self, ty: TypeId) -> Option<ArrayType> {
        let application = self.nominal_application(ty)?;
        let kind = match self.nominal_intrinsic_kind(application.template)? {
            hir::IntrinsicTypeKind::Array => ArrayKind::Immutable,
            hir::IntrinsicTypeKind::MutableArray => ArrayKind::Mutable,
            _ => return None,
        };
        let [element] = application.arguments.as_slice() else {
            unreachable!("a checked array application has one element type")
        };
        Some(ArrayType {
            kind,
            element: *element,
        })
    }

    pub(crate) fn array_element_ty(&self, ty: TypeId) -> Option<TypeId> {
        self.array_type_info(ty).map(|array| array.element)
    }
}
