use super::*;

mod origins;

impl Lowerer {
    pub(in crate::semantic_queries) fn describe_semantic_nominal(
        &mut self,
        ty: hir::TypeId,
    ) -> Result<Option<hir::SemanticNominalDescription>, Diagnostic> {
        let Some(declaration) = self.semantic_nominal_identity(ty) else {
            return Ok(None);
        };
        let target = self.nominal_owners.get(&declaration).copied();
        let application = self.nominal_application(ty);
        let arguments = application.map_or_else(
            || match self.types[ty] {
                hir::Type::FunPtr(function) => vec![self.function_types[function].canonical_type],
                _ => self.imported_owner_arguments(ty).to_vec(),
            },
            |application| application.arguments,
        );
        let parameters = if matches!(
            self.types[ty],
            hir::Type::Unit
                | hir::Type::Any
                | hir::Type::Integer(_)
                | hir::Type::Boolean
                | hir::Type::String
        ) {
            Vec::new()
        } else {
            self.nominal_declaration_parameters(declaration)
                .map_err(|error| {
                    Diagnostic::at(
                        Span::new(0, 0),
                        error.diagnostic("queried nominal declaration"),
                    )
                })?
        };
        let dependency = self
            .dependencies
            .as_ref()
            .and_then(|dependencies| dependencies.nominal_declaration(declaration));
        let origin = self.semantic_nominal_origin(declaration)?;
        let (annotations, is_object) = if let Some(owner) = target {
            (
                hir::SemanticAnnotationTarget::Current(hir::SourceAnnotationTarget::Nominal(
                    owner.as_nominal_owner(),
                )),
                matches!(owner, Owner::Object(_)),
            )
        } else {
            let dependency = dependency.expect("a dependency nominal retains its declaration");
            (
                hir::SemanticAnnotationTarget::Imported(hir::AnnotationTargetV1::Nominal(
                    declaration,
                )),
                dependency.interface.kind() == hir::PublicNominalKindV1::Object,
            )
        };
        let kind = if let Some(intrinsic) = self.nominal_intrinsic_kind(declaration) {
            hir::StaticNominalKind::Intrinsic(intrinsic)
        } else {
            match self.types[ty] {
                hir::Type::Struct(_) => hir::StaticNominalKind::Struct,
                hir::Type::Enum(_) => hir::StaticNominalKind::Enum,
                hir::Type::Interface(_) => hir::StaticNominalKind::Interface,
                hir::Type::Class(_) if is_object => hir::StaticNominalKind::Object,
                hir::Type::Class(_) => hir::StaticNominalKind::Class,
                _ => unreachable!("primitive declarations carry an intrinsic kind"),
            }
        };
        let companion = self.declared_companion_type(ty).map(|companion| {
            self.apply_nominal_type(companion, arguments.clone())
                .expect("a companion shares its resolved host arguments")
        });
        Ok(Some(hir::SemanticNominalDescription {
            declaration,
            kind,
            arguments,
            parameters,
            companion,
            annotations,
            origin,
        }))
    }
}
