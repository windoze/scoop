use super::*;

impl<'a> StaticNominalShape<'a> {
    /// Source constructors in declaration order; generated adapters are not declarations.
    pub fn constructors(
        self,
        world: &ImportedSemanticWorld<'a>,
    ) -> Vec<StaticConstructorShape<'a>> {
        let primary = self.primary_constructor(world);
        let mut result: Vec<_> = primary.into_iter().collect();
        match self.origin {
            StaticNominalOrigin::Current(NominalOwner::Struct(owner)) => {
                for &id in &self.module.structs[owner].constructors {
                    if matches!(
                        self.module.struct_constructors[id].kind,
                        StructConstructorKind::Primary
                    ) {
                        continue;
                    }
                    result.push(StaticConstructorShape::current(
                        self,
                        CallableTemplateOrigin::Constructor(
                            self.module.constructor_identities[id].id(),
                        ),
                        ExportParameterOwner::StructConstructor(id),
                        ParameterTargets::Plain,
                    ));
                }
            }
            StaticNominalOrigin::Current(NominalOwner::Class(owner)) => {
                for &id in &self.module.classes[owner].constructors {
                    let constructor = &self.module.class_constructors[id];
                    if constructor.identity_kind != ClassConstructorIdentityKind::Source
                        || matches!(constructor.kind, ClassConstructorKind::Primary { .. })
                    {
                        continue;
                    }
                    let identity = self.module.constructor_identities[id]
                        .source_record()
                        .unwrap()
                        .id();
                    result.push(StaticConstructorShape::current(
                        self,
                        CallableTemplateOrigin::Constructor(identity),
                        ExportParameterOwner::ClassConstructor(id),
                        ParameterTargets::Plain,
                    ));
                }
            }
            StaticNominalOrigin::Dependency(owner)
                if matches!(
                    self.kind(),
                    StaticNominalKind::Struct | StaticNominalKind::Class
                ) =>
            {
                let provider = world
                    .nominal_source_provider(owner)
                    .expect("a loaded nominal retains its provider");
                let interface = provider.source_interface();
                let nominal = interface.nominal_interfaces().declaration(owner).unwrap();
                let mut secondary = nominal
                    .declaration_details()
                    .constructors()
                    .values()
                    .iter()
                    .map(|&id| CallableTemplateOrigin::Constructor(id))
                    .filter(|&id| primary.is_none_or(|primary| primary.identity != id))
                    .map(|identity| {
                        let CallableTemplateOrigin::Constructor(id) = identity else {
                            unreachable!()
                        };
                        let position = provider
                            .source_foundation()
                            .definition_origin(
                                scoop_identity::DefinitionOriginSubject::Constructor(id),
                            )
                            .expect("a source constructor retains its origin")
                            .origin()
                            .span()
                            .start_byte();
                        (
                            position,
                            StaticConstructorShape::dependency(
                                self,
                                identity,
                                interface,
                                ParameterTargets::Plain,
                            ),
                        )
                    })
                    .collect::<Vec<_>>();
                secondary.sort_by_key(|(position, _)| *position);
                result.extend(secondary.into_iter().map(|(_, constructor)| constructor));
            }
            _ => {}
        }
        result
    }
}

impl<'a> StaticConstructorShape<'a> {
    pub const fn is_primary(self) -> bool {
        matches!(
            self.targets,
            ParameterTargets::Struct
                | ParameterTargets::Class(_)
                | ParameterTargets::ImportedClass(_)
        )
    }

    pub fn visibility(self) -> DeclaredVisibilityV1 {
        match self.source {
            ParameterSource::Current(source) => match source.owner {
                ExportParameterOwner::StructConstructor(id) => {
                    self.owner.module.struct_constructors[id]
                        .access
                        .declared
                        .into()
                }
                ExportParameterOwner::ClassConstructor(id) => self.owner.module.class_constructors
                    [id]
                    .access
                    .declared
                    .into(),
                ExportParameterOwner::VariantConstructor(_) => DeclaredVisibilityV1::Public,
                _ => unreachable!("constructor parameter owners are constructors"),
            },
            ParameterSource::Dependency { interface, .. } => {
                if matches!(self.identity, CallableTemplateOrigin::VariantConstructor(_)) {
                    DeclaredVisibilityV1::Public
                } else {
                    interface
                        .callable_interfaces()
                        .declaration(self.identity)
                        .expect("a constructor has a callable declaration")
                        .declared_visibility()
                }
            }
        }
    }

    fn annotation_target(self) -> StaticAnnotationTarget {
        match self.source {
            ParameterSource::Current(source) => StaticAnnotationTarget::Current(
                SourceAnnotationTarget::Callable(match source.owner {
                    ExportParameterOwner::StructConstructor(id) => {
                        SourceAnnotationCallable::StructConstructor(id)
                    }
                    ExportParameterOwner::ClassConstructor(id) => {
                        SourceAnnotationCallable::ClassConstructor(id)
                    }
                    ExportParameterOwner::VariantConstructor(id) => {
                        SourceAnnotationCallable::VariantConstructor(id)
                    }
                    _ => unreachable!("constructor parameter owners are constructors"),
                }),
            ),
            ParameterSource::Dependency { .. } => StaticAnnotationTarget::Dependency(
                AnnotationTargetV1::Callable(AnnotationCallableV1::Declaration(self.identity)),
            ),
        }
    }

    pub fn annotations(self, world: &ImportedSemanticWorld<'a>) -> StaticAnnotations<'a> {
        self.annotation_target().resolve(self.owner, world)
    }
}

impl<'a> StaticConstructorParameter<'a> {
    pub const fn index(self) -> usize {
        self.index
    }

    fn annotation_target(self) -> StaticAnnotationTarget {
        match self.constructor.annotation_target() {
            StaticAnnotationTarget::Current(SourceAnnotationTarget::Callable(callable)) => {
                StaticAnnotationTarget::Current(SourceAnnotationTarget::Parameter {
                    callable,
                    index: self.index as u32,
                })
            }
            StaticAnnotationTarget::Dependency(AnnotationTargetV1::Callable(callable)) => {
                StaticAnnotationTarget::Dependency(AnnotationTargetV1::Parameter {
                    callable,
                    index: self.index as u32,
                })
            }
            _ => unreachable!("a constructor has a callable annotation target"),
        }
    }

    pub fn annotations(self, world: &ImportedSemanticWorld<'a>) -> StaticAnnotations<'a> {
        self.annotation_target()
            .resolve(self.constructor.owner, world)
    }

    pub fn type_annotations(self, world: &ImportedSemanticWorld<'a>) -> &'a [TypeAnnotationNode] {
        self.annotation_target().type_annotations(
            self.constructor.owner,
            world,
            AnnotationTypeSlot::Value,
        )
    }
}
