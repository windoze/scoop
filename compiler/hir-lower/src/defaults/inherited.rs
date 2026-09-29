use super::*;
use hir::ImportedCallableSource;

impl Lowerer {
    pub(super) fn instantiate_inherited_dependency_default(
        &mut self,
        template: &hir::ExportDefaultTemplateV1,
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
        let prepared = match self.prepare_imported_default(&declaration, template.clone()) {
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

    pub(super) fn prepare_inherited_default(
        &mut self,
        key: SourceDefaultKey,
        span: ast::Span,
    ) -> Option<DefaultExprTemplateRef> {
        let SourceParameterOwner::Function(function) = key.owner else {
            unreachable!("only function declarations inherit parameter defaults")
        };
        let parents = self
            .override_default_sources
            .get(&function)
            .cloned()
            .unwrap_or_default();
        let mut sources = Vec::new();
        for parent in parents {
            let source = match parent {
                DefaultOverrideSource::Local {
                    function: parent,
                    type_arguments,
                } => {
                    if !self.function_parameter_has_default(parent, key.position as usize) {
                        continue;
                    }
                    let source = self.prepare_default(
                        SourceDefaultKey::new(
                            SourceParameterOwner::Function(parent),
                            key.position as usize,
                        ),
                        span,
                    )?;
                    let source = self.inherited_default_source(source);
                    let parameters = self.signatures[&parent]
                        .type_params
                        .iter()
                        .map(|parameter| parameter.id)
                        .collect::<Vec<_>>();
                    let bindings = parameters
                        .into_iter()
                        .zip(type_arguments)
                        .collect::<Vec<_>>();
                    match source {
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
                        source => source,
                    }
                }
                DefaultOverrideSource::Imported(declaration) => {
                    let declaration = self
                        .dependencies
                        .as_ref()
                        .expect("an imported override retains its dependency catalog")
                        .callable_declaration(declaration)
                        .expect("an override references an existing callable declaration");
                    let Some(template_key) = declaration
                        .source_interface()
                        .and_then(|source| {
                            source.parameters().parameters().get(key.position as usize)
                        })
                        .and_then(|parameter| parameter.calling().template())
                    else {
                        continue;
                    };
                    let template = declaration
                        .default_template(template_key)
                        .expect("an imported default parameter has its complete typed template");
                    let result = template.visit_definition_sources(
                        &mut |source| {
                            use crate::expr::imported_origins::ImportedDefinitionOriginError;
                            let imported = declaration.definition_source(source).ok_or(
                                ImportedDefinitionOriginError::MissingSource {
                                    context: source.origin().context(),
                                },
                            )?;
                            self.import_dependency_definition_origin(source, imported)
                                .map(|_| ())
                        },
                        &scoop_wire::WirePath::root(),
                    );
                    if let Err(error) = result {
                        self.error(span, error.to_string());
                        return None;
                    }
                    InheritedDefaultSource::Imported(std::sync::Arc::new(template.clone()))
                }
            };
            if !sources
                .iter()
                .any(|other: &InheritedDefaultSource| other.same_definition(&source))
            {
                sources.push(source);
            }
        }
        match sources.as_slice() {
            [source] => {
                if let InheritedDefaultSource::Export {
                    expression,
                    type_arguments,
                } = source
                {
                    self.check_inherited_default_access(function, *expression, type_arguments);
                }
                Some(self.record_inherited_default_source(source.clone()))
            }
            [] => None,
            _ => {
                let signature = &self.signatures[&function];
                self.error(span, format!("override function `{}` inherits conflicting default expressions for parameter `{}`",
                    self.functions[function].name.rsplit('.').next().unwrap_or(&self.functions[function].name), signature.params[key.position as usize].name.text));
                None
            }
        }
    }

    fn inherited_default_source(&self, source: DefaultExprTemplateRef) -> InheritedDefaultSource {
        match source {
            DefaultExprTemplateRef::Local(local) => InheritedDefaultSource::Local(local),
            DefaultExprTemplateRef::Export(source) => match &self.export_default_sources[source] {
                hir::ExportDefaultSource::Declared {
                    expression,
                    type_arguments,
                } => InheritedDefaultSource::Export {
                    expression: *expression,
                    type_arguments: type_arguments.clone(),
                },
                hir::ExportDefaultSource::Imported { template } => {
                    InheritedDefaultSource::Imported(template.clone())
                }
            },
        }
    }

    fn record_inherited_default_source(
        &mut self,
        source: InheritedDefaultSource,
    ) -> DefaultExprTemplateRef {
        let source = match source {
            InheritedDefaultSource::Local(local) => return DefaultExprTemplateRef::Local(local),
            InheritedDefaultSource::Export {
                expression,
                type_arguments,
            } => hir::ExportDefaultSource::Declared {
                expression,
                type_arguments,
            },
            InheritedDefaultSource::Imported(template) => {
                hir::ExportDefaultSource::Imported { template }
            }
        };
        DefaultExprTemplateRef::Export(self.export_default_sources.alloc(source))
    }
}

impl InheritedDefaultSource {
    fn same_definition(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Local(left), Self::Local(right)) => left == right,
            (
                Self::Export {
                    expression: left,
                    type_arguments: left_arguments,
                },
                Self::Export {
                    expression: right,
                    type_arguments: right_arguments,
                },
            ) => left == right && left_arguments == right_arguments,
            (Self::Imported(left), Self::Imported(right)) => {
                left.definition_root() == right.definition_root()
                    && left.definition_path() == right.definition_path()
                    && left.type_parameters() == right.type_parameters()
            }
            _ => false,
        }
    }
}
