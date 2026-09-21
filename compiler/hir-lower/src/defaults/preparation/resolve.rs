use super::*;
use crate::FnVarargOmission;

impl Lowerer {
    pub(crate) fn resolve_default_argument(
        &mut self,
        source: DefaultArgumentSource,
        span: ast::Span,
    ) -> Option<DefaultExprTemplateRef> {
        match source {
            DefaultArgumentSource::Ready(template) => Some(template),
            DefaultArgumentSource::Parameter(key) => {
                let diagnostics = self.diagnostics.len();
                let template = self.prepare_default(key, span);
                if template.is_none() && self.diagnostics.len() == diagnostics {
                    let recipe = &self.default_preparation.recipes[&key.owner];
                    self.error(
                        span,
                        format!(
                            "cannot expand invalid default for parameter `{}` in `{}`",
                            recipe.sources[key.position as usize].name.text,
                            recipe.context.callable_name,
                        ),
                    );
                }
                template
            }
        }
    }

    pub(in crate::defaults) fn prepare_default(
        &mut self,
        key: SourceDefaultKey,
        span: ast::Span,
    ) -> Option<DefaultExprTemplateRef> {
        if let Some(template) = self.default_templates.get(&key.tuple()) {
            return Some(*template);
        }
        let recipe = self
            .default_preparation
            .recipes
            .get(&key.owner)
            .expect("source default parameters were registered before expression lowering")
            .clone();
        let parameter = &recipe.sources[key.position as usize];
        match self.default_preparation.states.get(&key) {
            Some(PreparationState::Active) => {
                self.error(
                    span,
                    format!(
                        "cyclic default expansion for parameter `{}` in `{}`",
                        parameter.name.text, recipe.context.callable_name
                    ),
                );
                return None;
            }
            Some(PreparationState::Failed) => return None,
            None => {}
        }
        self.default_preparation
            .states
            .insert(key, PreparationState::Active);
        let file = std::mem::replace(&mut self.current_file, recipe.file);
        let paths = std::mem::replace(&mut self.definition_paths, recipe.paths.clone());
        let expression = match &parameter.calling {
            FnParamCalling::Default { expression }
            | FnParamCalling::Vararg {
                omission: FnVarargOmission::Default { expression },
                ..
            } => Some(expression),
            FnParamCalling::Required
            | FnParamCalling::Vararg {
                omission: FnVarargOmission::EmptyArray,
                ..
            } => None,
        };
        let template = if let Some(expression) = expression {
            match &recipe.kind {
                RecipeKind::Export(owner) => {
                    let captures = std::mem::take(&mut self.capture_contexts);
                    let result = self
                        .lower_export_default(
                            *owner,
                            expression,
                            parameter,
                            key.position as usize,
                            &recipe.sources,
                            &recipe.context,
                        )
                        .map(DefaultExprTemplateRef::Export);
                    self.capture_contexts = captures;
                    result
                }
                RecipeKind::Local(environment) => self
                    .lower_local_default(
                        expression,
                        parameter,
                        key.position as usize,
                        &recipe.sources,
                        &recipe.context,
                        environment,
                    )
                    .map(DefaultExprTemplateRef::Local),
            }
        } else {
            self.prepare_inherited_default(key, parameter.name.span)
        };
        self.current_file = file;
        self.definition_paths = paths;
        if let Some(template) = template {
            self.default_templates.insert(key.tuple(), template);
            self.default_preparation.states.remove(&key);
        } else {
            self.default_preparation
                .states
                .insert(key, PreparationState::Failed);
        }
        template
    }
}
