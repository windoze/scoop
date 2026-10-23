use super::*;
use crate::editing::ObservedBody;

impl Lowerer {
    pub(crate) fn editing_default_sources(&self) -> Vec<hir::EditingSourceBody> {
        self.default_preparation
            .recipes
            .iter()
            .flat_map(|(owner, recipe)| {
                recipe.sources.iter().enumerate().filter_map(|(index, _)| {
                    self.editing_default_source(SourceDefaultKey::new(*owner, index))
                })
            })
            .collect()
    }

    pub(crate) fn editing_default_source(
        &self,
        key: SourceDefaultKey,
    ) -> Option<hir::EditingSourceBody> {
        let recipe = self.default_preparation.recipes.get(&key.owner)?;
        if !recipe.observe_source {
            return None;
        }
        let parameter = recipe.sources.get(key.position as usize)?;
        let expression = match &parameter.calling {
            FnParamCalling::Default { expression }
            | FnParamCalling::Vararg {
                omission: crate::FnVarargOmission::Default { expression },
                ..
            } => expression,
            _ => return None,
        };
        Some(hir::EditingSourceBody {
            id: hir::EditingSourceBodyId::Default(key),
            owner: hir::EditingSourceBodyOwner::Default(source_callee(key.owner)),
            origin: hir::SemanticSourceLocation {
                source: self.source_identity(recipe.file).clone(),
                span: expression.span(),
            },
        })
    }

    pub(crate) fn editing_parameter_at_origin(
        &self,
        origin: &hir::SemanticSourceLocation,
    ) -> Option<(hir::EditingCallee, usize)> {
        self.default_preparation
            .recipes
            .iter()
            .find_map(|(owner, recipe)| {
                if !recipe.observe_source || self.source_identity(recipe.file) != &origin.source {
                    return None;
                }
                let index = recipe
                    .sources
                    .iter()
                    .position(|parameter| parameter.name.span == origin.span)?;
                Some((source_callee(*owner), index))
            })
    }

    pub(crate) fn editing_source_parameter_origin(
        &self,
        owner: SourceParameterOwner,
        index: usize,
    ) -> Option<hir::SemanticSourceLocation> {
        let recipe = self.default_preparation.recipes.get(&owner)?;
        Some(hir::SemanticSourceLocation {
            source: self.source_identity(recipe.file).clone(),
            span: recipe.sources.get(index)?.name.span,
        })
    }

    pub(crate) fn lower_editing_default(&mut self, key: SourceDefaultKey) -> Option<ObservedBody> {
        let origin = self.editing_default_source(key)?.origin;
        let id = hir::EditingSourceBodyId::Default(key);
        if !self.editing_prepared_bodies.as_ref()?.contains_key(&id) {
            self.prepare_default(key, origin.span);
        }
        self.editing_prepared_bodies.as_mut()?.remove(&id)
    }

    pub(super) fn observe_default_preparation(
        &mut self,
        key: SourceDefaultKey,
        recipe: &ParameterRecipe,
        operation: impl FnOnce(&mut Self) -> Option<DefaultExprTemplateRef>,
    ) -> Option<DefaultExprTemplateRef> {
        if !recipe.observe_source {
            return operation(self);
        }
        self.observe_prepared_body(
            hir::EditingSourceBodyId::Default(key),
            recipe.context.definition_root,
            recipe.file,
            operation,
        )
    }
}

fn source_callee(owner: SourceParameterOwner) -> hir::EditingCallee {
    match owner {
        SourceParameterOwner::Function(function) => hir::SemanticCallable::Current(function).into(),
        SourceParameterOwner::ClassConstructor(constructor) => {
            hir::EditingCallee::Constructor(hir::SemanticConstructor::CurrentClass(constructor))
        }
        SourceParameterOwner::StructConstructor(constructor) => {
            hir::EditingCallee::Constructor(hir::SemanticConstructor::CurrentStruct(constructor))
        }
        SourceParameterOwner::VariantConstructor(variant) => {
            hir::EditingCallee::CurrentVariant(variant)
        }
    }
}
