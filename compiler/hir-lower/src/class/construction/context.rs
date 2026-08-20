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
        self.with_constructor_context(source.into(), context, safety, None, lower)
    }

    pub(crate) fn with_property_initializer_context<T>(
        &mut self,
        constructor: hir::ClassConstructorId,
        property: hir::PropertyId,
        context: &str,
        lower: impl FnOnce(&mut Self, &mut Vec<hir::Statement>) -> Option<T>,
    ) -> Option<LoweredConstructorExpression<T>> {
        let owner = self.class_constructors[constructor].owner;
        let ordinal = self.classes[owner]
            .properties
            .iter()
            .position(|id| *id == property)
            .expect("a property belongs to its constructor owner");
        let path = crate::definition_paths::DefinitionPathContext::default().at(
            scoop_identity::StructuralDefinitionSiteRole::Initializer,
            ordinal as u32,
        );
        let paths = crate::definition_paths::DefinitionPathContext::nested(&path);
        self.with_constructor_context(
            constructor.into(),
            context,
            hir::Safety::Safe,
            Some(paths),
            lower,
        )
    }

    fn with_constructor_context<T>(
        &mut self,
        source: ConstructorSource,
        context: &str,
        safety: hir::Safety,
        property_paths: Option<crate::definition_paths::DefinitionPathContext>,
        lower: impl FnOnce(&mut Self, &mut Vec<hir::Statement>) -> Option<T>,
    ) -> Option<LoweredConstructorExpression<T>> {
        let independent_paths = property_paths.is_some();
        let definition_paths = property_paths.unwrap_or_else(|| {
            self.constructor_definition_paths
                .remove(&source)
                .unwrap_or_default()
        });
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
                        self.classes[class].type_params.clone(),
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
        let outer_scopes = self
            .local_nominal_scopes(owner)
            .map(|scopes| std::mem::replace(&mut self.scopes, scopes));
        let outer_function_scope = self
            .local_nominal_functions(owner)
            .map(|functions| std::mem::replace(&mut self.local_function_scopes, functions));
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
        let outer_receivers = std::mem::take(&mut self.receiver_scopes);
        let outer_smart_casts = self.smart_casts.clone();
        self.receiver_scopes = self.local_type_receivers(owner);
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
        let mut capture_environment = None;
        if let ConstructorSource::Class(constructor) = source
            && let Some(outer) = self.class_constructors[constructor]
                .context_parameter
                .clone()
        {
            let value = hir::Expr {
                kind: hir::ExprKind::ConstructorParam(outer.id),
                ty: outer.ty,
                span: outer.definition.span,
                origin: self.expression_origin(outer.definition.span),
            };
            match outer {
                hir::ConstructorContext::EnclosingInstance(_) => {
                    self.bind_enclosing_receivers(value, &mut statements)
                }
                hir::ConstructorContext::CapturedEnvironment(_) => {
                    capture_environment = Some(value)
                }
            }
        }
        let outer_capture_scope = self.enter_local_type_captures(owner, capture_environment);
        let value = lower(self, &mut statements);
        let locals = std::mem::take(&mut self.locals).into_arena();

        self.pop_safety_context();
        self.pop_suspension_context();
        self.pop_scope();
        if let Some(scopes) = outer_scopes {
            self.scopes = scopes;
        }
        if let Some(functions) = outer_function_scope {
            self.local_function_scopes = functions;
        }
        self.current_source_context = outer_source_context;
        self.current_this = outer_this;
        self.receiver_scopes = outer_receivers;
        self.local_type_capture_scope = outer_capture_scope;
        self.smart_casts = outer_smart_casts;
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
        if !independent_paths {
            assert!(
                self.constructor_definition_paths
                    .insert(source, definition_paths)
                    .is_none(),
                "a constructor traversal owns its definition paths"
            );
        }

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
