use super::*;
use scoop_identity::{CallableTemplateOrigin, PersistentPropertyAccessorId};

#[derive(Clone, Copy)]
pub struct StaticAccessorShape<'a> {
    property: StaticPropertyShape<'a>,
    pub identity: PersistentPropertyAccessorId,
    pub visibility: DeclaredVisibilityV1,
    pub implementation: PropertyAccessorImplementationV1,
    annotation: StaticAnnotationTarget,
}

impl<'a> StaticAccessorShape<'a> {
    pub fn annotations(self, world: &ImportedSemanticWorld<'a>) -> StaticAnnotations<'a> {
        self.annotation.resolve(self.property.owner, world)
    }
}

impl<'a> StaticPropertyShape<'a> {
    pub fn overrides(self) -> Vec<scoop_identity::PersistentPropertyId> {
        match self.source {
            StaticPropertyDeclaration::Current { declaration, .. } => declaration
                .overrides
                .iter()
                .map(|reference| match reference {
                    PropertyReference::Local(id) => self.owner.module.property_identities[*id]
                        .ordinary_id()
                        .unwrap(),
                    PropertyReference::Imported { declaration, .. } => *declaration,
                })
                .collect(),
            StaticPropertyDeclaration::Dependency(declaration) => declaration.overrides().to_vec(),
        }
    }

    pub fn visibility(self) -> DeclaredVisibilityV1 {
        match self.source {
            StaticPropertyDeclaration::Current { declaration, .. } => {
                declaration.access.declared.into()
            }
            StaticPropertyDeclaration::Dependency(declaration) => declaration.declared_visibility(),
        }
    }

    pub fn value_type_source(self) -> StaticShapeTypeSource<'a> {
        match self.source {
            StaticPropertyDeclaration::Current { declaration, .. } => {
                StaticShapeTypeSource::Hir(declaration.ty)
            }
            StaticPropertyDeclaration::Dependency(declaration) => {
                StaticShapeTypeSource::Published(declaration.value_type())
            }
        }
    }

    pub fn type_annotations(self, world: &ImportedSemanticWorld<'a>) -> &'a [TypeAnnotationNode] {
        self.annotation_target()
            .type_annotations(self.owner, world, AnnotationTypeSlot::Value)
    }

    pub fn accessors(
        self,
        world: &ImportedSemanticWorld<'a>,
    ) -> (StaticAccessorShape<'a>, Option<StaticAccessorShape<'a>>) {
        let module = self.owner.module;
        match self.source {
            StaticPropertyDeclaration::Current { declaration, .. } => {
                let getter = declaration.capability.getter();
                let value = &module.property_getters[getter];
                let getter = StaticAccessorShape {
                    property: self,
                    identity: module
                        .property_accessor_identities
                        .get_getter(getter)
                        .unwrap()
                        .id(),
                    visibility: value.access.declared.into(),
                    implementation: PropertyAccessorImplementationV1::from_source(
                        value.implementation,
                    ),
                    annotation: StaticAnnotationTarget::Current(SourceAnnotationTarget::Callable(
                        SourceAnnotationCallable::Getter(getter),
                    )),
                };
                let setter = declaration.capability.setter().map(|setter| {
                    let value = &module.property_setters[setter];
                    StaticAccessorShape {
                        property: self,
                        identity: module
                            .property_accessor_identities
                            .get_setter(setter)
                            .unwrap()
                            .id(),
                        visibility: value.access.declared.into(),
                        implementation: PropertyAccessorImplementationV1::from_source(
                            value.implementation,
                        ),
                        annotation: StaticAnnotationTarget::Current(
                            SourceAnnotationTarget::Callable(SourceAnnotationCallable::Setter(
                                setter,
                            )),
                        ),
                    }
                });
                (getter, setter)
            }
            StaticPropertyDeclaration::Dependency(declaration) => {
                let interface = world
                    .nominal_source_provider(self.owner.declaration)
                    .unwrap()
                    .source_interface();
                let accessor = |source: PropertyAccessorSourceV1| {
                    let callable = CallableTemplateOrigin::Accessor(source.accessor());
                    StaticAccessorShape {
                        property: self,
                        identity: source.accessor(),
                        visibility: interface
                            .callable_interfaces()
                            .declaration(callable)
                            .unwrap()
                            .declared_visibility(),
                        implementation: source.implementation(),
                        annotation: StaticAnnotationTarget::Dependency(
                            AnnotationTargetV1::Callable(AnnotationCallableV1::Declaration(
                                callable,
                            )),
                        ),
                    }
                };
                (
                    accessor(declaration.accessors().getter_source()),
                    declaration.accessors().setter_source().map(accessor),
                )
            }
        }
    }
}
