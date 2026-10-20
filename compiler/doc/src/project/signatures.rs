use super::*;

impl Projection<'_> {
    pub(super) fn signature(
        &self,
        declaration: &hir::DocumentationDeclaration,
    ) -> Result<Vec<SignatureToken>> {
        let source = &self.export.source_files[declaration.origin.file as usize].source;
        declaration
            .signature
            .iter()
            .map(|part| {
                let (text, link) = match part {
                    hir::DocumentationSignaturePart::Text(text) => (text.clone(), None),
                    hir::DocumentationSignaturePart::Source(span) => {
                        (source[span.start as usize..span.end as usize].into(), None)
                    }
                    hir::DocumentationSignaturePart::Type(ty) => (
                        hir::type_name_with_params(self.export, *ty, &declaration.type_parameters),
                        self.type_link(*ty)?,
                    ),
                };
                Ok(SignatureToken { text, link })
            })
            .collect()
    }

    pub(super) fn type_link(&self, ty: hir::TypeId) -> Result<Option<SymbolKey>> {
        use hir::Type as T;
        let export = self.export;
        let nominal = match &export.types[ty] {
            T::Struct(id) => Some(export.struct_applications[*id].template),
            T::Enum(id) => Some(export.enum_applications[*id].template),
            T::Class(id) => Some(export.class_applications[*id].template),
            T::Interface(id) => Some(export.interface_applications[*id].template),
            T::Unit => self.intrinsic(hir::IntrinsicTypeKind::Unit),
            T::Integer(kind) => self.intrinsic(hir::IntrinsicTypeKind::Integer(*kind)),
            T::Boolean => self.intrinsic(hir::IntrinsicTypeKind::Boolean),
            T::String => self.intrinsic(hir::IntrinsicTypeKind::String),
            T::Any => self.intrinsic(hir::IntrinsicTypeKind::Any),
            T::Ptr(_) => self.intrinsic(hir::IntrinsicTypeKind::Ptr),
            T::FunPtr(_) => self.intrinsic(hir::IntrinsicTypeKind::FunPtr),
            T::Param(parameter) => {
                return self.declarations.keys().find(|target| {
                    matches!(target, Target::TypeParameter { parameter: candidate, .. } if candidate == parameter)
                }).map(|target| self.target(*target)).transpose();
            }
            T::Tuple(_) | T::Function(_) | T::Companion(_) => None,
        };
        nominal
            .map(|owner| {
                SymbolKey::declaration(hir::AnnotationTargetV1::Nominal(owner))
                    .map_err(ProjectionError::from)
            })
            .transpose()
    }

    fn intrinsic(&self, kind: hir::IntrinsicTypeKind) -> Option<hir::SourceNominalId> {
        match &self.export.core_protocols {
            hir::CoreProtocols::Imported(core) => {
                use hir::{IntrinsicTypeKind as K, SourceNominalId as N};
                let core = core.fundamental_types();
                Some(match kind {
                    K::Unit => N::Concrete(core.unit().persistent()),
                    K::Integer(kind) => N::Concrete(core.integer(kind).persistent()),
                    K::Boolean => N::Concrete(core.boolean().persistent()),
                    K::Char => N::Concrete(core.character().persistent()),
                    K::Float(kind) => N::Concrete(core.floating(kind).persistent()),
                    K::String => N::Concrete(core.string().persistent()),
                    K::Array => N::GenericTemplate(core.array().persistent()),
                    K::MutableArray => N::GenericTemplate(core.mutable_array().persistent()),
                    K::Ptr => N::GenericTemplate(core.ptr().persistent()),
                    K::FunPtr => N::GenericTemplate(core.fun_ptr().persistent()),
                    K::Any => N::Concrete(core.any().persistent()),
                    K::Nothing => N::Concrete(core.nothing().persistent()),
                    K::Atomic(_) | K::MaybeUninit => return None,
                })
            }
            hir::CoreProtocols::Defined(core) => {
                use hir::{IntrinsicTypeKind as K, NominalOwner as N};
                let core = &core.fundamental_types;
                let owner = match kind {
                    K::Unit => N::Struct(core.unit),
                    K::Integer(kind) => N::Struct(core.integers.owner(kind)),
                    K::Boolean => N::Struct(core.boolean),
                    K::Char => N::Struct(core.character),
                    K::Float(hir::FloatKind::F32) => N::Struct(core.float),
                    K::Float(hir::FloatKind::F64) => N::Struct(core.double),
                    K::String => N::Class(core.string),
                    K::Array => N::Class(core.array),
                    K::MutableArray => N::Class(core.mutable_array),
                    K::Ptr => N::Struct(core.ptr),
                    K::FunPtr => N::Struct(core.fun_ptr),
                    K::Any => N::Class(core.any),
                    K::Nothing => N::Class(core.nothing),
                    K::Atomic(_) | K::MaybeUninit => return None,
                };
                Some(match owner {
                    N::Struct(id) => self.export.nominal_identities[id].declaration_id(),
                    N::Class(id) => self.export.nominal_identities[id].declaration_id(),
                    _ => unreachable!("fundamental protocols identify structs and classes"),
                })
            }
        }
    }
}
