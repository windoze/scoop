//! Inherited defaults retain the mapping from their original provider scope.

use super::*;
use crate::imported_core::ImportedTypeBindings;

impl Lowerer {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::defaults) fn instantiate_inherited_dependency_default(
        &mut self,
        template: &hir::ExportDefaultTemplateV1,
        type_arguments: Vec<hir::TypeId>,
        receiver: Option<&hir::Expr>,
        value_parameters: &[hir::Expr],
        span: ast::Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let declaration = match self
            .dependencies
            .as_ref()
            .expect("an inherited dependency default retains its declaration catalog")
            .callable_declaration(template.key().owner())
        {
            Ok(declaration) => declaration,
            Err(error) => {
                self.error(span, error.to_string());
                return None;
            }
        };
        let prepared = match self.prepare_imported_default_with_arguments(
            &declaration,
            template,
            type_arguments,
        ) {
            Ok(prepared) => prepared,
            Err(error) => {
                self.error(span, error.to_string());
                return None;
            }
        };
        match self.materialize_imported_default(&prepared, receiver, value_parameters, span, sink) {
            Ok(value) => Some(value),
            Err(error) => {
                self.error(span, error.to_string());
                None
            }
        }
    }

    pub(super) fn inherited_default_type_arguments(
        &mut self,
        function: hir::FunctionId,
        owner: hir::TypeId,
        declaration: &hir::ImportedCallableDeclaration,
        template: &hir::ExportDefaultTemplateV1,
        span: ast::Span,
    ) -> Option<Vec<hir::TypeId>> {
        let mut arguments = self
            .dependency_nominal_application(owner)
            .expect("an inherited dependency default has its declaring application")
            .1
            .to_vec();
        let host_arity = arguments.len();
        let own_arity = declaration.interface().type_parameters().binders().len();
        let signature = &self.signatures[&function];
        let own_parameters = signature.type_params[signature.type_params.len() - own_arity..]
            .iter()
            .map(|parameter| parameter.id)
            .collect::<Vec<_>>();
        arguments.extend(
            own_parameters
                .into_iter()
                .map(|id| self.intern_type(hir::Type::Param(id))),
        );
        let shape =
            hir::DefaultTemplateProviderShapeV1::try_new(host_arity as u32, own_arity as u32)
                .expect("checked inherited callable has representable binder arities");
        let bindings = arguments
            .into_iter()
            .enumerate()
            .map(|(index, ty)| {
                (
                    shape
                        .identity_binder_at(index as u32)
                        .expect("the declared parameter has a signature position"),
                    ty,
                )
            })
            .collect::<ImportedTypeBindings>();
        let result = template
            .type_parameters()
            .arguments()
            .iter()
            .map(|ty| self.imported_generic_type(ty, &bindings))
            .collect::<Result<Vec<_>, _>>();
        match result {
            Ok(arguments) => Some(arguments),
            Err(error) => {
                self.error(span, error);
                None
            }
        }
    }
}
