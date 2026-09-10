use super::*;
use crate::scope::{LocalFunctionScopes, Scopes};
use crate::{FnVarargOmission, ForbiddenSuspendContext, SuspensionContext, Type};

impl Lowerer {
    pub(super) fn lower_function_parameter_interface(&mut self, function: hir::FunctionId) {
        let signature = self.signatures[&function].clone();
        let sources = signature
            .params
            .iter()
            .map(|parameter| ParameterSource {
                name: parameter.name.clone(),
                ty: parameter.ty,
                calling: parameter.calling.clone(),
            })
            .collect::<Vec<_>>();
        let owner = self.function_owner.get(&function).copied();
        let receiver = owner
            .map(|owner| (self.owner_ty(owner), Some(owner)))
            .or_else(|| {
                self.extension_receivers
                    .get(&function)
                    .copied()
                    .map(|ty| (ty, None))
            });
        let context = DefaultContext {
            definition_root: self
                .definition_root
                .unwrap_or(hir::LexicalDefinitionRoot::Function(function)),
            type_parameters: signature.type_params,
            receiver,
            is_suspend: signature.is_suspend,
            safety: signature.attributes.safety,
            callable_name: self.functions[function].name.clone(),
        };
        self.lower_parameter_interface(
            hir::ExportParameterOwner::Function(function),
            &sources,
            &context,
        );
    }

    pub(crate) fn lower_local_parameter_interface(&mut self, function: hir::FunctionId) {
        let signature = self.signatures[&function].clone();
        let sources = signature
            .params
            .iter()
            .map(|parameter| ParameterSource {
                name: parameter.name.clone(),
                ty: parameter.ty,
                calling: parameter.calling.clone(),
            })
            .collect::<Vec<_>>();
        let context = DefaultContext {
            definition_root: self
                .definition_root
                .unwrap_or(hir::LexicalDefinitionRoot::Function(function)),
            type_parameters: signature.type_params,
            receiver: None,
            is_suspend: signature.is_suspend,
            safety: signature.attributes.safety,
            callable_name: self.functions[function].name.clone(),
        };
        let owner = SourceParameterOwner::Function(function);
        for (index, source) in sources.iter().enumerate() {
            let expression = match &source.calling {
                FnParamCalling::Default { expression } => Some(expression),
                FnParamCalling::Vararg {
                    omission: FnVarargOmission::Default { expression },
                    ..
                } => Some(expression),
                FnParamCalling::Required
                | FnParamCalling::Vararg {
                    omission: FnVarargOmission::EmptyArray,
                    ..
                } => None,
            };
            let Some(expression) = expression else {
                continue;
            };
            if let Some(template) =
                self.lower_local_default(expression, source, index, &sources, &context)
            {
                self.default_templates.insert(
                    (owner, index as u32),
                    DefaultExprTemplateRef::Local(template),
                );
            }
        }
    }

    pub(super) fn lower_parameter_interface(
        &mut self,
        owner: hir::ExportParameterOwner,
        sources: &[ParameterSource],
        context: &DefaultContext,
    ) {
        let mut parameters = Vec::with_capacity(sources.len());
        let mut complete = true;
        for (index, source) in sources.iter().enumerate() {
            let calling = match &source.calling {
                FnParamCalling::Required => Some(hir::ExportParameterCalling::Required {
                    value_type: source.ty,
                }),
                FnParamCalling::Default { expression } => self
                    .lower_export_default(owner, expression, source, index, sources, context)
                    .map(|default_source| hir::ExportParameterCalling::Default {
                        value_type: source.ty,
                        source: default_source,
                    }),
                FnParamCalling::Vararg {
                    element_ty,
                    omission,
                } => {
                    let parameter_type =
                        self.export_vararg_parameter_types
                            .alloc(hir::ExportVarargParameterType {
                                element_type: *element_ty,
                                array_type: source.ty,
                            });
                    let omission = match omission {
                        FnVarargOmission::EmptyArray => Some(hir::ExportVarargOmission::EmptyArray),
                        FnVarargOmission::Default { expression } => self
                            .lower_export_default(
                                owner, expression, source, index, sources, context,
                            )
                            .map(hir::ExportVarargOmission::Default),
                    };
                    omission.map(|omission| hir::ExportParameterCalling::Vararg {
                        parameter_type,
                        omission,
                    })
                }
            };
            let Some(calling) = calling else {
                complete = false;
                continue;
            };
            let template = match calling {
                hir::ExportParameterCalling::Default { source, .. } => {
                    Some(DefaultExprTemplateRef::Export(source))
                }
                hir::ExportParameterCalling::Vararg {
                    omission: hir::ExportVarargOmission::Default(source),
                    ..
                } => Some(DefaultExprTemplateRef::Export(source)),
                hir::ExportParameterCalling::Required { .. }
                | hir::ExportParameterCalling::Vararg {
                    omission: hir::ExportVarargOmission::EmptyArray,
                    ..
                } => None,
            };
            if let Some(template) = template {
                self.default_templates
                    .insert((source_owner(owner), index as u32), template);
            }
            parameters.push(hir::ExportValueParameter {
                name: source.name.text.clone(),
                calling,
                origin: self.definition_origin(source.name.span),
            });
        }
        if complete {
            self.source_parameter_interfaces
                .push(hir::ExportParameterInterface { owner, parameters });
        }
    }

    fn lower_export_default(
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
            false,
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
            self.export_default_sources.alloc(hir::ExportDefaultSource {
                expression,
                type_arguments,
            })
        })
    }

    fn lower_local_default(
        &mut self,
        expression: &ast::Expr,
        parameter: &ParameterSource,
        parameter_index: usize,
        sources: &[ParameterSource],
        context: &DefaultContext,
    ) -> Option<LocalDefaultExprId> {
        self.lower_default_template(
            expression,
            parameter,
            parameter_index,
            sources,
            context,
            true,
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
        lexical_captures: bool,
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
        let capture_environment = lexical_captures.then(|| self.capture_environment());
        let outer_locals = std::mem::take(&mut self.locals);
        let outer_scopes = std::mem::replace(&mut self.scopes, Scopes::new());
        let template_local_functions = if lexical_captures {
            self.local_function_scopes.clone()
        } else {
            LocalFunctionScopes::new()
        };
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
        self.set_source_context(context.callable_name.clone());
        let origin = self.definition_origin(expression.span());
        self.push_suspension_context(if context.is_suspend {
            SuspensionContext::SuspendFunction
        } else {
            SuspensionContext::Forbidden(ForbiddenSuspendContext::DefaultExpression)
        });
        self.push_safety_context(context.safety);
        self.push_scope();

        let receiver = context.receiver.map(|(ty, _)| {
            let local = self.alloc_local("this".to_string(), ty, false);
            self.scopes.declare("this".to_string(), local);
            self.current_this = Some((local, ty));
            hir::ExportDefaultReceiver { local, ty }
        });
        let mut value_parameters = Vec::with_capacity(parameter_index);
        for (position, source) in sources.iter().take(parameter_index).enumerate() {
            let local = self.alloc_local(source.name.text.clone(), source.ty, false);
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
        let captures = if lexical_captures {
            let captures = self.finish_current_captures();
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
            (
                hir::ExportDefaultExpr {
                    definition_root: context.definition_root,
                    definition_path,
                    locals,
                    statements,
                    value,
                    result_type: parameter.ty,
                    allows_suspend: context.is_suspend,
                    type_parameters: context
                        .type_parameters
                        .iter()
                        .map(|parameter| parameter.id)
                        .collect(),
                    receiver,
                    value_parameters,
                    references: hir::ExportDefaultReferences::default(),
                    origin,
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
            context: self.current_source_context,
        }
    }

    pub(crate) fn expression_origin(&self, span: ast::Span) -> hir::ExpressionOrigin {
        hir::ExpressionOrigin::Definition(self.definition_origin(span))
    }

    pub(crate) fn source_parameter_calling(
        &self,
        owner: SourceParameterOwner,
        index: usize,
        value_type: hir::TypeId,
        calling: &FnParamCalling,
    ) -> SourceParameterCalling {
        match calling {
            FnParamCalling::Required => self
                .default_templates
                .get(&(owner, index as u32))
                .copied()
                .map(SourceParameterCalling::Default)
                .unwrap_or(SourceParameterCalling::Required),
            FnParamCalling::Default { .. } => {
                SourceParameterCalling::Default(self.default_templates[&(owner, index as u32)])
            }
            FnParamCalling::Vararg {
                element_ty,
                omission,
            } => SourceParameterCalling::Vararg {
                element_type: *element_ty,
                array_type: value_type,
                omission: match omission {
                    FnVarargOmission::EmptyArray => self
                        .default_templates
                        .get(&(owner, index as u32))
                        .copied()
                        .map(SourceVarargOmission::Default)
                        .unwrap_or(SourceVarargOmission::EmptyArray),
                    FnVarargOmission::Default { .. } => SourceVarargOmission::Default(
                        self.default_templates[&(owner, index as u32)],
                    ),
                },
            },
        }
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

fn source_owner(owner: hir::ExportParameterOwner) -> SourceParameterOwner {
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
