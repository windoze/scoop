//! Descriptions use the established type graph and ordinary substitution.

use super::*;

impl Lowerer {
    pub(super) fn describe_semantic_type(
        &mut self,
        ty: hir::TypeId,
    ) -> Result<hir::SemanticTypeDescription, hir::StaticShapeUnboundParameter> {
        use hir::SemanticTypeStructure as Shape;
        let structure = match self.types[ty].clone() {
            hir::Type::Unit => Shape::Unit,
            hir::Type::Any => Shape::Any,
            hir::Type::Tuple(elements) => Shape::Tuple(elements),
            hir::Type::Function(function) => Shape::Function(self.function_types[function].clone()),
            hir::Type::Ptr(pointee) => Shape::Pointer(pointee),
            hir::Type::FunPtr(function) => {
                Shape::NativeFunctionPointer(self.function_types[function].clone())
            }
            hir::Type::Param(parameter) => Shape::Parameter(self.query_parameter(parameter)?),
            hir::Type::Companion(parameter) => {
                let host = self.query_parameter(parameter)?;
                let bounds = host
                    .companion
                    .clone()
                    .ok_or(hir::StaticShapeUnboundParameter(parameter))?;
                Shape::Companion { host, bounds }
            }
            hir::Type::Integer(_)
            | hir::Type::Boolean
            | hir::Type::String
            | hir::Type::Struct(_)
            | hir::Type::Class(_)
            | hir::Type::Enum(_)
            | hir::Type::Interface(_) => Shape::Nominal(self.describe_semantic_nominal(ty)),
        };
        Ok(hir::SemanticTypeDescription {
            ty,
            structure,
            name: self.type_name(ty),
            parents: self.direct_nominal_supertypes(ty),
        })
    }

    fn query_parameter(
        &self,
        parameter: hir::TypeParamId,
    ) -> Result<hir::TypeParamDecl, hir::StaticShapeUnboundParameter> {
        self.type_params_in_scope
            .iter()
            .find(|decl| decl.id == parameter)
            .cloned()
            .ok_or(hir::StaticShapeUnboundParameter(parameter))
    }

    fn describe_semantic_nominal(&mut self, ty: hir::TypeId) -> hir::SemanticNominalDescription {
        let (declaration, arguments) = if let Some(application) = self.nominal_application(ty) {
            let declaration = self
                .nominal_target_for_type(ty)
                .map_or(application.template, |target| {
                    self.nominal_identity(target.owner()).declaration_id()
                });
            (declaration, application.arguments)
        } else {
            let declaration = self
                .nominal_target_for_type(ty)
                .map(|target| self.nominal_identity(target.owner()).declaration_id())
                .or_else(|| self.imported_nominal_owner(ty))
                .expect("resolved primitive types retain their core declaration");
            (declaration, Vec::new())
        };
        let parameters = if matches!(
            self.types[ty],
            hir::Type::Integer(_) | hir::Type::Boolean | hir::Type::String
        ) {
            Vec::new()
        } else {
            self.nominal_declaration_parameters(declaration)
                .expect("a resolved nominal retains complete declaration parameters")
        };
        let kind = if let Some(intrinsic) = self.nominal_intrinsic_kind(declaration) {
            hir::StaticNominalKind::Intrinsic(intrinsic)
        } else {
            match self.types[ty] {
                hir::Type::Struct(_) => hir::StaticNominalKind::Struct,
                hir::Type::Enum(_) => hir::StaticNominalKind::Enum,
                hir::Type::Interface(_) => hir::StaticNominalKind::Interface,
                hir::Type::Class(_) => {
                    let is_object = matches!(
                        self.nominal_target_for_type(ty),
                        Some(crate::NominalTarget::Object(_))
                    ) || self
                        .dependencies
                        .as_ref()
                        .and_then(|dependencies| dependencies.nominal_declaration(declaration))
                        .is_some_and(|decl| {
                            decl.interface.kind() == hir::PublicNominalKindV1::Object
                        });
                    if is_object {
                        hir::StaticNominalKind::Object
                    } else {
                        hir::StaticNominalKind::Class
                    }
                }
                _ => unreachable!("primitive declarations carry an intrinsic kind"),
            }
        };
        let companion = self.declared_companion_type(ty).map(|companion| {
            self.apply_nominal_type(companion, arguments.clone())
                .expect("a companion shares its resolved host arguments")
        });
        hir::SemanticNominalDescription {
            declaration,
            kind,
            arguments,
            parameters,
            companion,
        }
    }
}
