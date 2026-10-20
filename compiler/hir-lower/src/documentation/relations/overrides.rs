use super::*;

impl Lowerer {
    pub(super) fn documentation_overrides(&mut self, function: hir::FunctionId) -> Vec<Relation> {
        let Some(method) = self.functions[function].method else {
            return Vec::new();
        };
        let mut targets = Vec::new();
        match method.dispatch {
            hir::MethodDispatch::Direct => {}
            hir::MethodDispatch::Virtual(family) | hir::MethodDispatch::FinalOverride(family) => {
                match &self.virtual_method_roots[&family] {
                    crate::persistent_dispatch::VirtualMethodRoot::Local(root)
                        if *root != function =>
                    {
                        targets.push(Symbol::Current(self.documentation_function_target(*root)))
                    }
                    crate::persistent_dispatch::VirtualMethodRoot::Imported(slot) => {
                        let origin = match slot.key().owner() {
                            scoop_identity::DispatchDeclarationOwner::Function(id) => {
                                scoop_identity::CallableTemplateOrigin::Function(id)
                            }
                            scoop_identity::DispatchDeclarationOwner::Accessor(id) => {
                                scoop_identity::CallableTemplateOrigin::Accessor(id)
                            }
                        };
                        targets.push(Symbol::Imported(hir::AnnotationTargetV1::Callable(
                            hir::AnnotationCallableV1::Declaration(origin),
                        )));
                    }
                    _ => {}
                }
            }
            hir::MethodDispatch::Interface(id) => {
                for reference in &self.interface_method_entities[id].overrides {
                    targets.push(match reference {
                        hir::InterfaceMethodReference::Local(id) => {
                            Symbol::Current(self.documentation_function_target(
                                self.interface_method_entities[*id].function,
                            ))
                        }
                        hir::InterfaceMethodReference::Imported { owner, slot } => {
                            let owner = self
                                .imported_nominal_owner(*owner)
                                .expect("imported interface owner");
                            let candidate = self
                                .dependencies
                                .as_ref()
                                .unwrap()
                                .callable_for_slot(owner, *slot)
                                .expect("checked imported slot")
                                .expect("imported slot declaration");
                            Symbol::Imported(hir::AnnotationTargetV1::Callable(
                                hir::AnnotationCallableV1::Declaration(
                                    candidate.interface().declaration(),
                                ),
                            ))
                        }
                    });
                }
            }
        }
        targets.extend(self.documentation_implemented_slots(function));
        let mut seen = HashSet::new();
        targets.retain(|target| seen.insert(*target));
        targets
            .into_iter()
            .map(|target| self.documentation_override(target))
            .collect()
    }

    pub(super) fn documentation_property_override(
        &self,
        reference: hir::PropertyReference,
    ) -> Relation {
        let target = match reference {
            hir::PropertyReference::Local(id) => Symbol::Current(Source::Property(id).into()),
            hir::PropertyReference::Imported { declaration, .. } => {
                Symbol::Imported(hir::AnnotationTargetV1::Property(declaration))
            }
        };
        self.documentation_override(target)
    }

    fn documentation_override(&self, target: Symbol) -> Relation {
        let (signature, origin) = match target {
            Symbol::Current(Target::Declaration(Source::Property(id))) => {
                (self.properties[id].name.clone(), String::new())
            }
            Symbol::Current(Target::Declaration(Source::Callable(Callable::Function(id)))) => (
                self.source_function_name(id).to_owned(),
                self.function_owner[&id].describe_name(self),
            ),
            Symbol::Imported(hir::AnnotationTargetV1::Callable(
                hir::AnnotationCallableV1::Declaration(origin),
            )) => {
                let member = self
                    .dependencies
                    .as_ref()
                    .unwrap()
                    .callable_declaration(origin)
                    .expect("checked overridden declaration");
                (member.name().into(), String::new())
            }
            Symbol::Imported(hir::AnnotationTargetV1::Property(id)) => {
                let dependencies = self.dependencies.as_ref().unwrap();
                let property = dependencies
                    .property_declaration(scoop_identity::PropertyOwner::Property(id))
                    .expect("checked overridden property");
                let getter = dependencies
                    .callable_declaration(scoop_identity::CallableTemplateOrigin::Accessor(
                        property.accessors().getter(),
                    ))
                    .expect("checked overridden getter");
                (getter.name().into(), String::new())
            }
            _ => unreachable!("an override targets a method or property"),
        };
        Relation {
            kind: RelationKind::Overrides,
            target,
            signature,
            origin,
            visibility: hir::DeclaredVisibility::Public,
            hidden: matches!(target, Symbol::Imported(target) if self.documentation_imported_hidden(target)),
        }
    }
}
