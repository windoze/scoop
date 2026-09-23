use super::*;

impl Lowerer {
    pub(super) fn prepare_inherited_default(
        &mut self,
        key: SourceDefaultKey,
        span: ast::Span,
    ) -> Option<DefaultExprTemplateRef> {
        let SourceParameterOwner::Function(function) = key.owner else {
            unreachable!("only function declarations inherit parameter defaults")
        };
        let parents = self
            .override_sources
            .get(&function)
            .cloned()
            .unwrap_or_default();
        let mut sources = Vec::new();
        for parent in parents {
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
            let arguments = self.override_default_type_arguments[&(function, parent)].clone();
            let bindings = parameters.into_iter().zip(arguments).collect::<Vec<_>>();
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
        sources.sort();
        sources.dedup();
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
}
