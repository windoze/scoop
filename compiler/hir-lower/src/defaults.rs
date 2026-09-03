//! Definition-site lowering of exported source-parameter interfaces.

use la_arena::Idx;
use scoop_ast as ast;
use scoop_hir as hir;

use crate::scope::{LocalFunctionScopes, Scopes};
use crate::{
    FnParamCalling, FnVarargOmission, ForbiddenSuspendContext, Lowerer, Owner, SuspensionContext,
    Type,
};

mod instantiate;

#[derive(Clone)]
pub(crate) struct LocalDefaultExpr {
    pub(crate) body: hir::ExportDefaultExpr,
    pub(crate) captures: Vec<hir::Capture>,
}

pub(crate) type LocalDefaultExprId = Idx<LocalDefaultExpr>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DefaultExprTemplateRef {
    Local(LocalDefaultExprId),
    Export(hir::ExportDefaultSourceId),
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
enum InheritedDefaultSource {
    Local(LocalDefaultExprId),
    Export {
        expression: hir::ExportDefaultExprId,
        type_arguments: Vec<hir::TypeId>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum SourceParameterOwner {
    Function(hir::FunctionId),
    StructConstructor(hir::StructId),
    ClassConstructor(hir::ClassId),
    VariantConstructor {
        enumeration: hir::EnumId,
        variant: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourceParameterCalling {
    Required,
    Default(DefaultExprTemplateRef),
    Vararg {
        element_type: hir::TypeId,
        array_type: hir::TypeId,
        omission: SourceVarargOmission,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourceVarargOmission {
    EmptyArray,
    Default(DefaultExprTemplateRef),
}

#[derive(Clone)]
struct ParameterSource {
    name: ast::Ident,
    ty: hir::TypeId,
    calling: FnParamCalling,
}

#[derive(Clone)]
struct DefaultContext {
    type_parameters: Vec<hir::TypeParamDecl>,
    receiver: Option<(hir::TypeId, Option<Owner>)>,
    is_suspend: bool,
    safety: hir::Safety,
    callable_name: String,
}

impl Lowerer {
    pub(crate) fn lower_export_parameter_interfaces(
        &mut self,
        functions: &[(hir::FunctionId, &ast::FunctionDecl, usize)],
        methods: &[(hir::FunctionId, &ast::FunctionDecl, usize, Owner)],
        structs: &[(hir::StructId, &ast::StructDecl, usize)],
        classes: &[(hir::ClassId, &ast::ClassDecl, usize)],
        enums: &[(hir::EnumId, &ast::EnumDecl, usize)],
    ) {
        for &(function, _, file) in functions {
            self.current_file = file;
            self.lower_function_parameter_interface(function);
        }
        for &(function, declaration, file, _) in methods {
            self.current_file = file;
            if declaration.is_override && declaration.params.iter().any(has_explicit_default) {
                self.error(
                    declaration.name.span,
                    format!(
                        "override function `{}` cannot declare a new default expression",
                        declaration.name.text
                    ),
                );
            }
            self.lower_function_parameter_interface(function);
        }
        for &(structure, declaration, file) in structs {
            self.current_file = file;
            if matches!(
                self.structs[structure].representation,
                hir::StructRepresentation::Declared(_)
            ) {
                let callings = self.struct_parameter_calling[&structure].clone();
                let sources = declaration
                    .fields
                    .iter()
                    .zip(
                        self.structs[structure]
                            .semantic_fields()
                            .iter()
                            .zip(callings),
                    )
                    .map(|(source, (field, calling))| ParameterSource {
                        name: source.name.clone(),
                        ty: field.ty,
                        calling,
                    })
                    .collect::<Vec<_>>();
                let context = DefaultContext {
                    type_parameters: self.structs[structure].type_params.clone(),
                    receiver: None,
                    is_suspend: false,
                    safety: hir::Safety::Safe,
                    callable_name: self.structs[structure].name.clone(),
                };
                self.lower_parameter_interface(
                    hir::ExportParameterOwner::StructConstructor(structure),
                    &sources,
                    &context,
                );
            }
        }
        for &(class, declaration, file) in classes {
            self.current_file = file;
            if matches!(
                self.classes[class].representation,
                hir::ClassRepresentation::Declared(_)
            ) {
                let callings = self.class_parameter_calling[&class].clone();
                let sources = declaration
                    .constructor
                    .iter()
                    .zip(
                        self.classes[class]
                            .semantic_constructor()
                            .iter()
                            .zip(callings),
                    )
                    .map(|(source, (field, calling))| ParameterSource {
                        name: source.name.clone(),
                        ty: field.ty,
                        calling,
                    })
                    .collect::<Vec<_>>();
                let context = DefaultContext {
                    type_parameters: self.classes[class].type_params.clone(),
                    receiver: None,
                    is_suspend: false,
                    safety: hir::Safety::Safe,
                    callable_name: self.classes[class].name.clone(),
                };
                self.lower_parameter_interface(
                    hir::ExportParameterOwner::ClassConstructor(class),
                    &sources,
                    &context,
                );
            }
        }
        for &(enumeration, declaration, file) in enums {
            self.current_file = file;
            for (variant_index, source_variant) in declaration.variants.iter().enumerate() {
                let variant_index = variant_index as u32;
                let callings =
                    self.variant_parameter_calling[&(enumeration, variant_index)].clone();
                let source_fields = match &source_variant.kind {
                    ast::VariantDeclKind::Unit | ast::VariantDeclKind::Positional(_) => None,
                    ast::VariantDeclKind::Named(fields)
                    | ast::VariantDeclKind::Constructor(fields) => Some(fields.as_slice()),
                };
                let fields = &self.enums[enumeration].variants[variant_index as usize].fields;
                let sources = fields
                    .iter()
                    .zip(callings)
                    .enumerate()
                    .map(|(index, (field, calling))| ParameterSource {
                        name: source_fields.map_or_else(
                            || ast::Ident {
                                text: field.name.clone(),
                                span: source_variant.span,
                            },
                            |source| source[index].name.clone(),
                        ),
                        ty: field.ty,
                        calling,
                    })
                    .collect::<Vec<_>>();
                let context = DefaultContext {
                    type_parameters: self.enums[enumeration].type_params.clone(),
                    receiver: None,
                    is_suspend: false,
                    safety: hir::Safety::Safe,
                    callable_name: format!(
                        "{}.{}",
                        self.enums[enumeration].name, source_variant.name.text
                    ),
                };
                self.lower_parameter_interface(
                    hir::ExportParameterOwner::VariantConstructor {
                        enumeration,
                        variant: variant_index,
                    },
                    &sources,
                    &context,
                );
            }
        }
        self.inherit_override_parameter_interfaces(methods);
    }

    fn inherit_override_parameter_interfaces(
        &mut self,
        methods: &[(hir::FunctionId, &ast::FunctionDecl, usize, Owner)],
    ) {
        for &(function, declaration, file, _) in methods {
            if !declaration.is_override {
                continue;
            }
            self.current_file = file;
            let parameter_count = self.signatures[&function].params.len();
            for index in 0..parameter_count {
                if self
                    .default_templates
                    .contains_key(&(SourceParameterOwner::Function(function), index as u32))
                {
                    continue;
                }
                let mut visiting = Vec::new();
                let mut sources = self.inherited_default_sources(function, index, &mut visiting);
                sources.sort();
                sources.dedup();
                match sources.as_slice() {
                    [] => {}
                    [source] => {
                        let source = self.record_inherited_default_source(source.clone());
                        self.default_templates.insert(
                            (SourceParameterOwner::Function(function), index as u32),
                            source,
                        );
                        self.patch_export_override_parameter(function, index, source);
                    }
                    _ => {
                        self.error(
                            declaration.params[index].name.span,
                            format!(
                                "override function `{}` inherits conflicting default expressions for parameter `{}`",
                                declaration.name.text, declaration.params[index].name.text
                            ),
                        );
                    }
                }
            }
        }
    }

    fn inherited_default_sources(
        &mut self,
        function: hir::FunctionId,
        parameter: usize,
        visiting: &mut Vec<hir::FunctionId>,
    ) -> Vec<InheritedDefaultSource> {
        if visiting.contains(&function) {
            return Vec::new();
        }
        if let Some(&source) = self
            .default_templates
            .get(&(SourceParameterOwner::Function(function), parameter as u32))
        {
            return vec![self.inherited_default_source(source)];
        }
        visiting.push(function);
        let parents = self
            .override_sources
            .get(&function)
            .into_iter()
            .flatten()
            .copied()
            .collect::<Vec<_>>();
        let mut sources = Vec::new();
        for parent in parents {
            let parent_parameters = self.signatures[&parent]
                .type_params
                .iter()
                .map(|parameter| parameter.id)
                .collect::<Vec<_>>();
            let parent_arguments =
                self.override_default_type_arguments[&(function, parent)].clone();
            let bindings = parent_parameters
                .into_iter()
                .zip(parent_arguments)
                .collect::<Vec<_>>();
            for source in self.inherited_default_sources(parent, parameter, visiting) {
                sources.push(match source {
                    InheritedDefaultSource::Local(local) => InheritedDefaultSource::Local(local),
                    InheritedDefaultSource::Export {
                        expression,
                        type_arguments,
                    } => InheritedDefaultSource::Export {
                        expression,
                        type_arguments: type_arguments
                            .into_iter()
                            .map(|argument| self.instantiate_method_ty(argument, &bindings))
                            .collect(),
                    },
                });
            }
        }
        visiting.pop();
        sources
    }

    fn inherited_default_source(&self, source: DefaultExprTemplateRef) -> InheritedDefaultSource {
        match source {
            DefaultExprTemplateRef::Local(local) => InheritedDefaultSource::Local(local),
            DefaultExprTemplateRef::Export(source) => {
                let source = &self.export_default_sources[source];
                InheritedDefaultSource::Export {
                    expression: source.expression,
                    type_arguments: source.type_arguments.clone(),
                }
            }
        }
    }

    fn record_inherited_default_source(
        &mut self,
        source: InheritedDefaultSource,
    ) -> DefaultExprTemplateRef {
        match source {
            InheritedDefaultSource::Local(local) => DefaultExprTemplateRef::Local(local),
            InheritedDefaultSource::Export {
                expression,
                type_arguments,
            } => DefaultExprTemplateRef::Export(self.export_default_sources.alloc(
                hir::ExportDefaultSource {
                    expression,
                    type_arguments,
                },
            )),
        }
    }

    fn patch_export_override_parameter(
        &mut self,
        function: hir::FunctionId,
        index: usize,
        source: DefaultExprTemplateRef,
    ) {
        let DefaultExprTemplateRef::Export(source) = source else {
            unreachable!("exported overrides inherit only exported default templates")
        };
        let interface = self
            .source_parameter_interfaces
            .iter_mut()
            .find(|interface| interface.owner == hir::ExportParameterOwner::Function(function))
            .expect("every exported method has a source parameter interface");
        let parameter = &mut interface.parameters[index];
        parameter.calling = match parameter.calling {
            hir::ExportParameterCalling::Required { value_type } => {
                hir::ExportParameterCalling::Default { value_type, source }
            }
            hir::ExportParameterCalling::Vararg {
                parameter_type,
                omission: hir::ExportVarargOmission::EmptyArray,
            } => hir::ExportParameterCalling::Vararg {
                parameter_type,
                omission: hir::ExportVarargOmission::Default(source),
            },
            calling @ (hir::ExportParameterCalling::Default { .. }
            | hir::ExportParameterCalling::Vararg {
                omission: hir::ExportVarargOmission::Default(_),
                ..
            }) => calling,
        };
    }

    fn lower_function_parameter_interface(&mut self, function: hir::FunctionId) {
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

    fn lower_parameter_interface(
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
                    .lower_export_default(expression, source, index, sources, context)
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
                            .lower_export_default(expression, source, index, sources, context)
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
        .map(|(body, captures)| {
            debug_assert!(captures.is_empty());
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
        let origin = self.definition_origin(expression.span());
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
            format!(
                "{} default `{}`",
                context.callable_name, parameter.name.text
            ),
        );
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
        self.push_suspension_context(if context.is_suspend {
            SuspensionContext::SuspendFunction
        } else {
            SuspensionContext::Forbidden(ForbiddenSuspendContext::Function)
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
        let value = self.lower_expr(expression, &mut statements, Some(parameter.ty));
        let value = value.and_then(|value| {
            if self.is_subtype(value.ty, parameter.ty) {
                Some(self.adapt_to(value, parameter.ty))
            } else {
                self.error(
                    expression.span(),
                    format!(
                        "default value of parameter `{}` in `{}` must be of type {}, found {}",
                        parameter.name.text,
                        context.callable_name,
                        self.type_name(parameter.ty),
                        self.type_name(value.ty)
                    ),
                );
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
        self.current_this = outer_this;
        self.current_owner = outer_owner;
        self.constructor_params_in_scope = outer_constructor_parameters;
        self.smart_casts = outer_smart_casts;
        value.map(|value| {
            (
                hir::ExportDefaultExpr {
                    locals,
                    statements,
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
                captures,
            )
        })
    }

    fn definition_origin(&self, span: ast::Span) -> hir::DefinitionOrigin {
        hir::DefinitionOrigin {
            provider: self.current_intrinsic_provider(),
            file: u32::try_from(self.current_file).expect("source file index exceeds u32"),
            span,
        }
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

fn source_owner(owner: hir::ExportParameterOwner) -> SourceParameterOwner {
    match owner {
        hir::ExportParameterOwner::Function(function) => SourceParameterOwner::Function(function),
        hir::ExportParameterOwner::StructConstructor(structure) => {
            SourceParameterOwner::StructConstructor(structure)
        }
        hir::ExportParameterOwner::ClassConstructor(class) => {
            SourceParameterOwner::ClassConstructor(class)
        }
        hir::ExportParameterOwner::VariantConstructor {
            enumeration,
            variant,
        } => SourceParameterOwner::VariantConstructor {
            enumeration,
            variant,
        },
    }
}

fn has_explicit_default(parameter: &ast::Param) -> bool {
    matches!(
        parameter.syntax,
        ast::ParameterSyntax::Default { .. }
            | ast::ParameterSyntax::Vararg {
                default: ast::VarargDefaultSyntax::Expression { .. },
                ..
            }
    )
}
