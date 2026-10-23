//! Describe source constructors without exposing generated invocation adapters.

use super::*;
use scoop_identity::CallableTemplateOrigin;

mod identities;
mod imported;
mod source;
mod variants;

impl Lowerer {
    pub(super) fn query_nominal_constructors(
        &mut self,
        ty: hir::TypeId,
    ) -> Result<Option<Vec<hir::SemanticConstructorDescription>>, Diagnostic> {
        let Some(application) = self.nominal_application(ty) else {
            return Ok(None);
        };
        let parameters = self
            .nominal_declaration_parameters(application.template)
            .expect("a resolved nominal retains complete declaration parameters");
        let substitutions = parameters
            .iter()
            .zip(&application.arguments)
            .map(|(parameter, argument)| (parameter.id, *argument))
            .collect::<Vec<_>>();
        let declaration_arguments = parameters
            .iter()
            .map(|parameter| self.intern_type(hir::Type::Param(parameter.id)))
            .collect();
        let declaration_type = self
            .apply_nominal_type(application.template, declaration_arguments)
            .expect("the original declaration application is already resolved");
        let result = hir::SemanticTypeUse {
            declaration: declaration_type,
            applied: ty,
        };
        if let Some((dependency, _)) = self.dependency_nominal_application(ty) {
            let dependency = dependency.clone();
            return self
                .query_imported_constructors(result, &dependency, &parameters, &substitutions)
                .map(Some);
        }
        let mut constructors = match self.types[ty] {
            hir::Type::Struct(application) => {
                self.query_struct_constructors(application, result, &substitutions)?
            }
            hir::Type::Class(application) => {
                let class = self.class_id(self.class_applications[application].template);
                if self.object_by_backing_class.contains_key(&class) {
                    Vec::new()
                } else {
                    self.query_class_constructors(class, result, &substitutions)?
                }
            }
            hir::Type::Enum(application) => {
                self.query_variant_constructors(application, result, &substitutions)
            }
            _ => Vec::new(),
        };
        constructors.sort_by_key(|constructor| {
            (
                constructor.kind != hir::SemanticConstructorKind::Primary,
                constructor.origin.span.start,
            )
        });
        Ok(Some(constructors))
    }
}
