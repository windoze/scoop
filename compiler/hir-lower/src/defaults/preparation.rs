//! Declaration recipes are distinct from fully lowered, kind-typed templates.
use super::*;
use std::collections::HashMap;
use std::rc::Rc;

mod editing;
mod interfaces;
mod resolve;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DefaultArgumentSource {
    Ready(DefaultExprTemplateRef),
    Parameter(SourceDefaultKey),
}

#[derive(Clone)]
pub(super) struct LocalDefaultEnvironment {
    pub types: crate::scope::Scopes,
    pub available: HashMap<crate::CaptureKey, crate::AvailableCapture>,
    pub receivers: Vec<crate::ReceiverScope>,
    pub functions: crate::scope::LocalFunctionScopes,
}
#[derive(Clone)]
enum RecipeKind {
    Export(hir::ExportParameterOwner),
    Local(LocalDefaultEnvironment),
}
#[derive(Clone)]
struct ParameterRecipe {
    sources: Vec<ParameterSource>,
    context: DefaultContext,
    file: usize,
    paths: crate::definition_paths::DefinitionPathContext,
    kind: RecipeKind,
    // Shared initializers can register another executable copy of a local callable.
    observe_source: bool,
}
#[derive(Clone, Copy)]
enum PreparationState {
    Active,
    Failed,
}

#[derive(Clone, Default)]
pub(crate) struct DefaultPreparation {
    recipes: HashMap<SourceParameterOwner, Rc<ParameterRecipe>>,
    export_order: Vec<hir::ExportParameterOwner>,
    states: HashMap<SourceDefaultKey, PreparationState>,
}

impl Lowerer {
    pub(crate) fn complete_constructor_default_context(
        &mut self,
        class: hir::ClassId,
        ty: hir::TypeId,
    ) {
        for (owner, recipe) in &mut self.default_preparation.recipes {
            if let SourceParameterOwner::ClassConstructor(id) = owner
                && self.class_constructors[*id].owner == class
            {
                Rc::make_mut(recipe).context.receiver = Some((ty, None));
            }
        }
    }

    fn register_default_recipe(
        &mut self,
        owner: SourceParameterOwner,
        sources: Vec<ParameterSource>,
        context: DefaultContext,
        kind: RecipeKind,
    ) {
        let recipe = ParameterRecipe {
            sources,
            context,
            kind,
            file: self.current_file,
            paths: self.definition_paths.clone(),
            observe_source: self.editing_prepared_bodies.is_some(),
        };
        assert!(
            self.default_preparation
                .recipes
                .insert(owner, Rc::new(recipe))
                .is_none(),
            "each source parameter declaration is registered once"
        );
    }

    pub(super) fn register_export_parameter_interface(
        &mut self,
        owner: hir::ExportParameterOwner,
        sources: &[ParameterSource],
        context: &DefaultContext,
    ) {
        self.register_default_recipe(
            lowering::source_owner(owner),
            sources.to_vec(),
            context.clone(),
            RecipeKind::Export(owner),
        );
        self.default_preparation.export_order.push(owner);
    }

    pub(super) fn register_local_default_parameters(
        &mut self,
        function: hir::FunctionId,
        sources: Vec<ParameterSource>,
        context: DefaultContext,
        environment: LocalDefaultEnvironment,
    ) {
        let owner = SourceParameterOwner::Function(function);
        self.register_default_recipe(owner, sources, context, RecipeKind::Local(environment));
        self.prepare_parameter_defaults(owner);
    }

    fn prepare_parameter_defaults(&mut self, owner: SourceParameterOwner) {
        let recipe = self.default_preparation.recipes[&owner].clone();
        for (index, source) in recipe.sources.iter().enumerate() {
            if matches!(
                self.source_parameter_calling(owner, index, source.ty, &source.calling),
                ValueParameterCalling::Default(_)
                    | ValueParameterCalling::Vararg {
                        omission: VarargOmission::Default(_),
                        ..
                    }
            ) {
                self.prepare_default(SourceDefaultKey::new(owner, index), source.name.span);
            }
        }
    }
}
