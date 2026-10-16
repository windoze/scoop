//! Missing inspect requirements become ordinary, lazy member readers.

use super::coding::{CodingContext, CodingDirection};
use super::interfaces::InterfaceMemberInstance;
use super::*;
use crate::CoreLoweringAuthority;
use ast::Span;

mod classes;
mod declarations;
mod expressions;
mod records;
mod tuples;
mod variants;

#[derive(Clone, Copy)]
pub(super) enum InspectionTypeFactory {
    Current(FunctionId),
    Imported(hir::ImportedGenericCallableTemplateId),
}

impl Lowerer {
    pub(crate) fn lower_derived_inspection_bodies(&mut self, defined: Option<FunctionId>) {
        if self.derived_inspection_methods.is_empty() {
            return;
        }
        let factory = match &self.core {
            CoreLoweringAuthority::Defined => InspectionTypeFactory::Current(
                defined.expect("core inspection uses its checked type reference factory"),
            ),
            CoreLoweringAuthority::Imported(core) => {
                let hir::ImportedCoreProtocolCallableDefinition::GenericFunction(factory) =
                    core.type_reference().definition()
                else {
                    unreachable!("the type reference factory is generic")
                };
                let declaration = self.dependencies.as_ref().unwrap().callable_declaration(
                    scoop_identity::CallableTemplateOrigin::GenericFunction(factory.persistent()),
                );
                let result = declaration
                    .map_err(|error| error.to_string())
                    .and_then(|declaration| self.request_imported_generic_template(declaration));
                let template = match result {
                    Ok(template) => template,
                    Err(error) => {
                        let function = self.derived_inspection_methods[0].0;
                        self.current_file = self.function_files[&function];
                        self.error(self.functions[function].span, error);
                        return;
                    }
                };
                InspectionTypeFactory::Imported(template)
            }
        };
        for (function, interface) in std::mem::take(&mut self.derived_inspection_methods) {
            self.current_file = self.function_files[&function];
            let context = CodingContext {
                owner: self.function_owner[&function],
                target: self.signatures[&function].params[0].ty,
                interface,
                direction: CodingDirection::Inspect(factory),
            };
            let span = self.functions[function].span;
            let body = self.lower_synthesized_body(function, |state| {
                let parameter = &state.functions[function].params[1];
                let value = state.coding_expr(hir::ExprKind::Local(parameter.local), parameter.ty, span);
                let before = state.diagnostics.len();
                let mut statements = Vec::new();
                if let Some(value) = state.inspect_target(context, context.target, value, span, &mut statements) {
                    statements.push(hir::Statement { kind: hir::StatementKind::Return { value }, span });
                } else if state.diagnostics.len() == before {
                    state.error(span, "automatic inspect could not resolve the required core view or field access".into());
                }
                statements
            });
            self.functions[function].kind = hir::FunctionKind::User(body);
        }
    }

    pub(in crate::class::inheritance) fn inspect_target(
        &mut self,
        context: CodingContext,
        target: TypeId,
        value: hir::Expr,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        self.push_scope();
        let result = self.inspection_view(context, target, value, span, sink);
        self.pop_scope();
        result
    }

    fn inspection_view(
        &mut self,
        context: CodingContext,
        target: TypeId,
        value: hir::Expr,
        span: Span,
        sink: &mut Vec<hir::Statement>,
    ) -> Option<hir::Expr> {
        let reference = self.inspection_reference(context, target, span)?;
        let reference = self.inspection_bind("__inspection_type", reference, span, sink);
        let value = self.inspection_bind("__inspection_value", value, span, sink);
        let identity_ty = self.option_type(self.any);
        let identity = if matches!(self.types[target], Type::Class(_)) {
            let object = self.adapt_to(value, self.any);
            self.coding_expr(hir::ExprKind::SomeWrap(Box::new(object)), identity_ty, span)
        } else {
            self.coding_expr(hir::ExprKind::NoneLiteral, identity_ty, span)
        };
        let shape_ty = self.inspection_type("ValueShape", span)?;
        let shape = self.lower_synthesized_lambda(false, &[], shape_ty, span, |state, _| {
            let value = state.inspection_capture("__inspection_value", span)?;
            let reference = state.inspection_capture("__inspection_type", span)?;
            let mut statements = Vec::new();
            let shape = match state.types[target].clone() {
                Type::Struct(_) => state.inspect_struct(context, target, value, reference, span, &mut statements),
                Type::Class(_) => state.inspect_class(context, target, value, reference, span, &mut statements),
                Type::Enum(application) => state.inspect_enum(context, application, value, reference, span, &mut statements),
                Type::Tuple(elements) => state.inspect_tuple(context, &elements, value, span, &mut statements),
                _ => {
                    state.error(span, format!("automatic inspect requires a struct, enum, tuple, or final class without a class base; target {} requires an explicit implementation", state.type_name(target)));
                    None
                }
            }?;
            Some(crate::stmt::ValueBlock { statements, value: Some(shape) })
        })?;
        let observation = self.inspection_construct(
            "ValueObservation",
            vec![reference, identity, shape],
            span,
            sink,
        )?;
        let view = self.inspection_type("ValueView", span)?;
        Some(self.adapt_to(observation, view))
    }
}
