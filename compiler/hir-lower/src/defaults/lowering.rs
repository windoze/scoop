use super::*;
use crate::scope::{LocalFunctionScopes, Scopes};
use crate::{FnVarargOmission, ForbiddenSuspendContext, SuspensionContext, Type};

impl Lowerer {
    pub(super) fn lower_export_default(
        &mut self,
        owner: hir::ExportParameterOwner,
        expression: &ast::Expr,
        parameter: &ParameterSource,
        parameter_index: usize,
        sources: &[ParameterSource],
        context: &DefaultContext,
    ) -> Option<hir::ExportDefaultSourceId> {
        self.lower_default_template(
            expression,
            parameter,
            parameter_index,
            sources,
            context,
            None,
        )
        .map(|(mut body, captures)| {
            debug_assert!(captures.is_empty());
            body.references = self.collect_export_default_references(owner, &body);
            let type_arguments = body
                .type_parameters
                .iter()
                .map(|&parameter| self.intern_type(Type::Param(parameter)))
                .collect();
            let expression = self.export_default_exprs.alloc(body);
            self.export_default_sources
                .alloc(hir::ExportDefaultSource::Declared {
                    expression,
                    type_arguments,
                })
        })
    }

    pub(super) fn lower_local_default(
        &mut self,
        expression: &ast::Expr,
        parameter: &ParameterSource,
        parameter_index: usize,
        sources: &[ParameterSource],
        context: &DefaultContext,
        environment: &super::preparation::LocalDefaultEnvironment,
    ) -> Option<LocalDefaultExprId> {
        self.lower_default_template(
            expression,
            parameter,
            parameter_index,
            sources,
            context,
            Some(environment),
        )
        .map(|(body, captures)| {
            self.local_default_exprs
                .alloc(LocalDefaultExpr { body, captures })
        })
    }

    fn lower_default_template(
        &mut self,
        expression: &ast::Expr,
        parameter: &ParameterSource,
        parameter_index: usize,
        sources: &[ParameterSource],
        context: &DefaultContext,
        environment: Option<&super::preparation::LocalDefaultEnvironment>,
    ) -> Option<(hir::ExportDefaultExpr, Vec<hir::Capture>)> {
        let default_ordinal = sources[..parameter_index]
            .iter()
            .filter(|source| parameter_has_default_expression(source))
            .count();
        let default_ordinal = u32::try_from(default_ordinal)
            .expect("one source declaration cannot contain more than u32::MAX defaults");
        let definition_path = self.definition_paths.at(
            scoop_identity::StructuralDefinitionSiteRole::DefaultValue,
            default_ordinal,
        );
        let outer_definition_paths = std::mem::replace(
            &mut self.definition_paths,
            crate::definition_paths::DefinitionPathContext::nested(&definition_path),
        );
        let outer_definition_root = self.definition_root.replace(context.definition_root);
        let capture_environment = environment.map(|environment| environment.available.clone());
        let outer_locals = std::mem::take(&mut self.locals);
        let outer_scopes = std::mem::replace(&mut self.scopes, Scopes::new());
        let template_local_functions = environment
            .map_or_else(LocalFunctionScopes::new, |environment| {
                environment.functions.clone()
            });
        let outer_local_functions =
            std::mem::replace(&mut self.local_function_scopes, template_local_functions);
        let outer_type_parameters = std::mem::replace(
            &mut self.type_params_in_scope,
            context.type_parameters.clone(),
        );
        let outer_return_ty = self.current_return_ty;
        let outer_return_inference = self.return_inference.take();
        let outer_fn_name = std::mem::replace(
            &mut self.current_fn_name,
            format!("`{}` of `{}`", parameter.name.text, context.callable_name),
        );
        let outer_loop_targets = std::mem::take(&mut self.loop_targets);
        let outer_source_context = self.current_source_context;
        let outer_this = self.current_this.take();
        let outer_owner = self.current_owner;
        let outer_constructor_parameters = std::mem::take(&mut self.constructor_params_in_scope);
        let outer_smart_casts = std::mem::take(&mut self.smart_casts);
        if let Some(available) = capture_environment {
            self.capture_contexts.push(crate::CaptureContext {
                available,
                captures: Vec::new(),
                by_binding: std::collections::HashMap::new(),
            });
        }

        self.current_return_ty = parameter.ty;
        self.current_owner = context.receiver.and_then(|(_, owner)| owner);
        self.set_source_context(context.source_context.clone());
        let origin = self.definition_origin(expression.span());
        self.push_suspension_context(if context.is_suspend {
            SuspensionContext::SuspendFunction
        } else {
            SuspensionContext::Forbidden(ForbiddenSuspendContext::DefaultExpression)
        });
        self.push_safety_context(context.safety);
        self.push_scope();

        let receiver = context.receiver.map(|(ty, _)| {
            let local = self.alloc_this_local(ty, expression.span());
            self.scopes.declare("this".to_string(), local);
            self.current_this = Some((local, ty));
            hir::ExportDefaultReceiver { local, ty }
        });
        let mut value_parameters = Vec::with_capacity(parameter_index);
        for (position, source) in sources.iter().take(parameter_index).enumerate() {
            let local = self.alloc_parameter_local(
                source.name.text.clone(),
                source.ty,
                position,
                source.name.span,
            );
            self.scopes.declare(source.name.text.clone(), local);
            value_parameters.push(hir::ExportDefaultValueParameter {
                position: position as u32,
                local,
            });
        }

        let mut statements = Vec::new();
        let outer_default_template = std::mem::replace(&mut self.lowering_default_template, true);
        let value = self.lower_expr(expression, &mut statements, Some(parameter.ty));
        self.lowering_default_template = outer_default_template;
        let value = value.and_then(|value| {
            if self.is_subtype(value.ty, parameter.ty) {
                Some(self.adapt_to(value, parameter.ty))
            } else {
                let message = self.with_nominal_invariance_detail(
                    format!(
                        "default value of parameter `{}` in `{}` must be of type {}, found {}",
                        parameter.name.text,
                        context.callable_name,
                        self.type_name(parameter.ty),
                        self.type_name(value.ty)
                    ),
                    value.ty,
                    parameter.ty,
                );
                self.error(expression.span(), message);
                None
            }
        });
        let locals = std::mem::take(&mut self.locals);
        let captures = if environment.is_some() {
            let captures = self.finish_current_captures(hir::ExpressionOrigin::Definition(origin));
            self.capture_contexts.pop();
            captures
        } else {
            Vec::new()
        };

        self.pop_scope();
        self.pop_safety_context();
        self.pop_suspension_context();
        self.locals = outer_locals;
        self.scopes = outer_scopes;
        self.local_function_scopes = outer_local_functions;
        self.type_params_in_scope = outer_type_parameters;
        self.current_return_ty = outer_return_ty;
        self.return_inference = outer_return_inference;
        self.current_fn_name = outer_fn_name;
        self.current_source_context = outer_source_context;
        self.definition_paths = outer_definition_paths;
        self.definition_root = outer_definition_root;
        self.current_this = outer_this;
        self.current_owner = outer_owner;
        self.constructor_params_in_scope = outer_constructor_parameters;
        self.smart_casts = outer_smart_casts;
        debug_assert!(self.loop_targets.is_empty());
        self.loop_targets = outer_loop_targets;
        value.map(|value| {
            self.default_local_value_scopes
                .alloc(super::PendingDefaultLocalScope {
                    definition_root: super::DefaultScopeRoot::Declared(context.definition_root),
                    definition_path: definition_path.clone(),
                    values: locals
                        .iter()
                        .map(|(_, local)| hir::DefaultLocalValueDefinition {
                            binding: local.binding,
                            selector: local.selector.clone(),
                            definition: local.definition,
                        })
                        .collect(),
                });
            (
                hir::ExportDefaultExpr {
                    definition_root: context.definition_root,
                    definition_path,
                    allows_suspend: context.is_suspend,
                    expression: hir::DefaultExpression {
                        body: hir::Body { locals, statements },
                        value,
                        result_type: parameter.ty,
                        type_parameters: context
                            .type_parameters
                            .iter()
                            .map(|parameter| parameter.id)
                            .collect(),
                        receiver,
                        value_parameters,
                        origin,
                    },
                    references: hir::ExportDefaultReferences::default(),
                },
                captures,
            )
        })
    }

    pub(crate) fn definition_origin(&self, span: ast::Span) -> hir::DefinitionOrigin {
        hir::DefinitionOrigin {
            provider: self.current_intrinsic_provider(),
            file: u32::try_from(self.current_file).expect("source file index exceeds u32"),
            span,
            context: self.source_context_for_current_file(),
        }
    }

    pub(crate) fn expression_origin(&self, span: ast::Span) -> hir::ExpressionOrigin {
        self.derived_expression_origin
            .unwrap_or_else(|| hir::ExpressionOrigin::Definition(self.definition_origin(span)))
    }
}

fn parameter_has_default_expression(source: &ParameterSource) -> bool {
    matches!(
        source.calling,
        FnParamCalling::Default { .. }
            | FnParamCalling::Vararg {
                omission: FnVarargOmission::Default { .. },
                ..
            }
    )
}

pub(super) fn source_owner(owner: hir::ExportParameterOwner) -> SourceParameterOwner {
    match owner {
        hir::ExportParameterOwner::Function(function) => SourceParameterOwner::Function(function),
        hir::ExportParameterOwner::StructConstructor(structure) => {
            SourceParameterOwner::StructConstructor(structure)
        }
        hir::ExportParameterOwner::ClassConstructor(class) => {
            SourceParameterOwner::ClassConstructor(class)
        }
        hir::ExportParameterOwner::VariantConstructor(variant) => {
            SourceParameterOwner::VariantConstructor(variant)
        }
    }
}

pub(super) fn has_explicit_default(parameter: &ast::Param) -> bool {
    matches!(
        parameter.syntax,
        ast::ParameterSyntax::Default { .. }
            | ast::ParameterSyntax::Vararg {
                default: ast::VarargDefaultSyntax::Expression { .. },
                ..
            }
    )
}
