//! Lexical state for constructor defaults, delegation, and initialization.

use super::*;

impl Lowerer {
    pub(crate) fn with_constructor_expression_context<T>(
        &mut self,
        source: impl Into<ConstructorSource>,
        context: &str,
        safety: hir::Safety,
        lower: impl FnOnce(&mut Self, &mut Vec<hir::Statement>) -> Option<T>,
    ) -> Option<LoweredConstructorExpression<T>> {
        let source = source.into();
        let definition_paths = self
            .constructor_definition_paths
            .remove(&source)
            .unwrap_or_default();
        let outer_definition_paths =
            std::mem::replace(&mut self.definition_paths, definition_paths);
        let definition_root = match source {
            ConstructorSource::Class(constructor) => {
                hir::LexicalDefinitionRoot::ClassConstructor(constructor)
            }
            ConstructorSource::Struct(constructor) => {
                hir::LexicalDefinitionRoot::StructConstructor(constructor)
            }
        };
        let outer_definition_root = self.definition_root.replace(definition_root);
        let (parameters, type_parameters, owner, owner_name) = match source {
            ConstructorSource::Class(constructor) => {
                let declaration = &self.class_constructors[constructor];
                let class = declaration.owner;
                if let Some(&object) = self.object_by_backing_class.get(&class) {
                    (
                        declaration.parameters.clone(),
                        Vec::new(),
                        Owner::Object(object),
                        self.objects[object].name.clone(),
                    )
                } else {
                    (
                        declaration.parameters.clone(),
                        self.classes[class].type_params.clone(),
                        Owner::Class(class),
                        self.classes[class].name.clone(),
                    )
                }
            }
            ConstructorSource::Struct(constructor) => {
                let declaration = &self.struct_constructors[constructor];
                let owner = declaration.owner;
                (
                    declaration.parameters.clone(),
                    self.structs[owner].type_params.clone(),
                    Owner::Struct(owner),
                    self.structs[owner].name.clone(),
                )
            }
        };
        let outer_type_parameters =
            std::mem::replace(&mut self.type_params_in_scope, type_parameters);
        let outer_constructor_parameters = std::mem::replace(
            &mut self.constructor_params_in_scope,
            parameters
                .iter()
                .map(|parameter| {
                    (
                        parameter.name.clone(),
                        (
                            parameter.id,
                            parameter.ty,
                            self.constructor_parameter_bindings[&parameter.id],
                        ),
                    )
                })
                .collect(),
        );
        let outer_locals = std::mem::take(&mut self.locals);
        let outer_return_ty = std::mem::replace(&mut self.current_return_ty, self.unit);
        let outer_fn_name = std::mem::replace(
            &mut self.current_fn_name,
            format!("<init {owner_name}: {context}>"),
        );
        let outer_loop_targets = std::mem::take(&mut self.loop_targets);
        let outer_owner = self.current_owner.replace(owner);
        let outer_this = self.current_this.take();
        let outer_source_context = self.current_source_context;
        self.set_source_context(hir::SourceContextSubject::Constructor(match source {
            ConstructorSource::Class(constructor) => {
                hir::SourceContextConstructor::Class(constructor)
            }
            ConstructorSource::Struct(constructor) => {
                hir::SourceContextConstructor::Struct(constructor)
            }
        }));
        self.push_scope();
        self.push_suspension_context(SuspensionContext::Forbidden(
            if self.initialization_context.is_some() {
                ForbiddenSuspendContext::ConstructorInitialization
            } else {
                ForbiddenSuspendContext::ConstructorDelegation
            },
        ));

        self.push_safety_context(safety);
        let mut statements = Vec::new();
        let value = lower(self, &mut statements);
        let locals = std::mem::take(&mut self.locals);

        self.pop_safety_context();
        self.pop_suspension_context();
        self.pop_scope();
        self.current_source_context = outer_source_context;
        self.current_this = outer_this;
        self.current_owner = outer_owner;
        self.current_fn_name = outer_fn_name;
        self.current_return_ty = outer_return_ty;
        self.locals = outer_locals;
        debug_assert!(self.loop_targets.is_empty());
        self.loop_targets = outer_loop_targets;
        self.constructor_params_in_scope = outer_constructor_parameters;
        self.type_params_in_scope = outer_type_parameters;
        let definition_paths =
            std::mem::replace(&mut self.definition_paths, outer_definition_paths);
        self.definition_root = outer_definition_root;
        assert!(
            self.constructor_definition_paths
                .insert(source, definition_paths)
                .is_none(),
            "a constructor definition-path context is checked out only once"
        );

        value.map(|value| LoweredConstructorExpression {
            locals,
            statements,
            value,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum ConstructorSource {
    Class(hir::ClassConstructorId),
    Struct(hir::StructConstructorId),
}

impl From<hir::ClassConstructorId> for ConstructorSource {
    fn from(value: hir::ClassConstructorId) -> Self {
        Self::Class(value)
    }
}

impl From<hir::StructConstructorId> for ConstructorSource {
    fn from(value: hir::StructConstructorId) -> Self {
        Self::Struct(value)
    }
}

pub(crate) struct LoweredConstructorExpression<T> {
    pub(crate) locals: la_arena::Arena<hir::Local>,
    pub(crate) statements: Vec<hir::Statement>,
    pub(crate) value: T,
}
