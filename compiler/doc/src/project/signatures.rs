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

    fn type_link(&self, ty: hir::TypeId) -> Result<Option<SymbolKey>> {
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
            T::Tuple(_) | T::Function(_) | T::Param(_) | T::Companion(_) => None,
        };
        nominal
            .map(|owner| {
                SymbolKey::declaration(hir::AnnotationTargetV1::Nominal(owner))
                    .map_err(ProjectionError::from)
            })
            .transpose()
    }

    fn intrinsic(&self, kind: hir::IntrinsicTypeKind) -> Option<hir::SourceNominalId> {
        self.export
            .imported_intrinsic_types
            .get(&kind)
            .map(|value| value.declaration.owner())
    }
}
