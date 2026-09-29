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
                hir::IntrinsicTypeKind::Integer(_)
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
        if let Type::ImportedClass(class) = &self.types[ty] {
            let hir::NominalSourceShapeV1::Intrinsic(representation) =
                class.declaration.interface.source_shape()
            else {
                return None;
            };
            let kind = match representation.family() {
                hir::IntrinsicTypeKind::Array => ArrayKind::Immutable,
                hir::IntrinsicTypeKind::MutableArray => ArrayKind::Mutable,
                _ => return None,
            };
            let [element] = class.arguments.as_slice() else {
                unreachable!("a checked array application has one element type")
            };
            return Some(ArrayType {
                kind,
                element: *element,
            });
        }
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
                hir::IntrinsicTypeRepresentation::Integer(_)
                | hir::IntrinsicTypeRepresentation::Boolean
                | hir::IntrinsicTypeRepresentation::String
                | hir::IntrinsicTypeRepresentation::Ptr { .. }
                | hir::IntrinsicTypeRepresentation::FunPtr { .. },
            ) => None,
        }
    }

    pub(crate) fn array_element_ty(&self, ty: TypeId) -> Option<TypeId> {
        self.array_type_info(ty).map(|array| array.element)
    }
}
