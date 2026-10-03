use super::*;

impl Lowerer {
    pub(super) fn release_call(
        &self,
        target: hir::CallableTarget,
        span: hir::Span,
        values: &ReleaseValueFacts,
        facts: &mut BodyFacts,
    ) {
        match target {
            hir::CallableTarget::Local(callable) => {
                let function = self.callable_function_id(callable);
                if self.functions[function]
                    .method
                    .is_some_and(|method| !matches!(method.dispatch, hir::MethodDispatch::Direct))
                {
                    facts.requirements = None;
                    return;
                }
                facts
                    .calls
                    .push(self.generic_call(callable, span).unwrap_or(GenericCall {
                        callee: GenericCallable::Function(function),
                        arguments: Vec::new(),
                        span,
                    }));
            }
            hir::CallableTarget::Application(application) => {
                let call = self.imported_body_generic_call(application, span);
                let template = self.imported_generic_applications[application].template;
                if matches!(self.imported_generic_templates[template].declaration,
                    hir::ImportedCallableTemplateOrigin::Nominal { dispatch, .. }
                    if !matches!(dispatch, hir::DeclaredMethodDispatch::Direct))
                {
                    facts.requirements = None;
                    return;
                }
                self.release_imported_call(
                    &self.imported_generic_templates[template].release_callability,
                    &call.arguments,
                    values,
                    facts,
                );
            }
            hir::CallableTarget::Dependency(id) => {
                let dependency = self.imported_dependency_callables[id];
                if dependency.dispatch() != hir::ImportedDependencyDispatch::Direct {
                    facts.requirements = None;
                    return;
                }
                let declaration = self
                    .dependencies
                    .as_ref()
                    .expect("a dependency call retains its declaration view")
                    .resolve_callable(dependency.reference())
                    .expect("a selected dependency callable is present");
                let effect = declaration.interface().effects();
                if !matches!(effect.release_callability(),
                    hir::CallableReleaseCallabilityV1::NoTransition { requirements }
                    if requirements.is_empty())
                {
                    facts.requirements = None;
                }
            }
        }
    }

    pub(super) fn release_constructor_call(
        &self,
        application: hir::StructConstructorApplicationId,
        span: hir::Span,
        values: &ReleaseValueFacts,
        facts: &mut BodyFacts,
    ) {
        let target = self.struct_constructor_applications[application].constructor;
        match target {
            hir::StructConstructorDefinition::Local(id) => {
                if matches!(
                    self.struct_constructors[id].kind,
                    hir::StructConstructorKind::Primary
                ) {
                    return;
                }
                facts.calls.push(
                    self.generic_struct_constructor_call(application, span)
                        .unwrap_or(GenericCall {
                            callee: GenericCallable::StructConstructor(id),
                            arguments: Vec::new(),
                            span,
                        }),
                );
            }
            hir::StructConstructorDefinition::Template(id) => {
                let template = &self.imported_constructor_templates[id];
                let constructor = &template.initialization.constructors()[template.constructor];
                if matches!(
                    constructor.kind(),
                    hir::ExportConstructorInitializationKindV1::StructPrimary
                ) {
                    return;
                }
                let hir::CallableReleaseCallabilityV1::NoTransition { requirements } =
                    template.signature.effects.release_callability()
                else {
                    facts.requirements = None;
                    return;
                };
                let owner = self.struct_constructor_applications[application].owner;
                let arguments = &self.struct_applications[owner].arguments;
                facts.require(values.values(
                    self,
                    requirements.iter().map(|requirement| {
                        assert_eq!(
                            requirement.depth, 0,
                            "constructor binders belong to their owner"
                        );
                        arguments[requirement.index as usize]
                    }),
                ));
            }
        }
    }

    fn release_imported_call(
        &self,
        effect: &hir::ReleaseCallability,
        arguments: &[(hir::TypeParamId, hir::TypeId)],
        values: &ReleaseValueFacts,
        facts: &mut BodyFacts,
    ) {
        let hir::ReleaseCallability::NoTransition { requirements } = effect else {
            facts.requirements = None;
            return;
        };
        facts.require(values.values(
            self,
            requirements.iter().map(|parameter| {
                arguments
                    .iter()
                    .find_map(|(key, ty)| (key == parameter).then_some(*ty))
                    .expect("an imported call binds every declared requirement")
            }),
        ));
    }
}
