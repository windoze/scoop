use super::*;
use scoop_identity::{DeclarationName, NominalDeclarationOwner, PersistentAnnotationId};

#[derive(Clone, Copy)]
pub struct StaticAnnotation<'a> {
    pub annotation: PersistentAnnotationId,
    pub arguments: &'a [CanonicalAnnotationValueV1],
}

impl<'a> StaticAnnotation<'a> {
    pub fn name(self, module: &'a Module, dependencies: &ImportedSemanticWorld<'a>) -> &'a str {
        let key = match module
            .annotations
            .declarations
            .iter()
            .find(|declaration| declaration.identity.id() == self.annotation)
        {
            Some(declaration) => declaration.identity.key(),
            None => dependencies
                .annotation_source(self.annotation)
                .expect("a resolved annotation retains its declaration"),
        };
        match key.name() {
            DeclarationName::Named(name) => name.as_str(),
            _ => unreachable!("annotation declarations have source names"),
        }
    }
}

#[derive(Clone, Copy)]
pub enum StaticAnnotations<'a> {
    Current(&'a [SourceAnnotationApplication]),
    Dependency(&'a [AnnotationApplicationV1]),
}

impl<'a> StaticAnnotations<'a> {
    /// Query any source declaration, accessor, receiver, or complete value parameter.
    pub fn current(module: &'a Module, target: SourceAnnotationTarget) -> Self {
        Self::Current(
            module
                .annotations
                .targets
                .iter()
                .find(|entry| entry.target == target)
                .map_or(&[], |entry| entry.annotations.as_slice()),
        )
    }

    pub fn published(annotations: &'a CanonicalAnnotationsV1, target: AnnotationTargetV1) -> Self {
        Self::Dependency(annotations.annotations(target))
    }

    pub fn len(self) -> usize {
        match self {
            Self::Current(values) => values.len(),
            Self::Dependency(values) => values.len(),
        }
    }

    pub fn is_empty(self) -> bool {
        self.len() == 0
    }

    pub fn iter(self) -> impl ExactSizeIterator<Item = StaticAnnotation<'a>> {
        (0..self.len()).map(move |index| match self {
            Self::Current(values) => StaticAnnotation {
                annotation: values[index].annotation,
                arguments: &values[index].arguments,
            },
            Self::Dependency(values) => StaticAnnotation {
                annotation: values[index].annotation,
                arguments: &values[index].arguments,
            },
        })
    }
}

impl StaticAnnotationTarget {
    pub(super) fn resolve<'a>(
        self,
        owner: StaticNominalShape<'a>,
        dependencies: &ImportedSemanticWorld<'a>,
    ) -> StaticAnnotations<'a> {
        match self {
            Self::Current(target) => StaticAnnotations::current(owner.module, target),
            Self::Dependency(target) => {
                let provider = dependencies
                    .nominal_source_provider(owner.declaration)
                    .expect("a loaded nominal retains its source provider");
                StaticAnnotations::published(provider.source_interface().annotations(), target)
            }
        }
    }
}

impl<'a> StaticNominalShape<'a> {
    pub fn annotations(self, dependencies: &ImportedSemanticWorld<'a>) -> StaticAnnotations<'a> {
        let target = match self.origin {
            StaticNominalOrigin::Current(owner) => {
                StaticAnnotationTarget::Current(SourceAnnotationTarget::Nominal(owner))
            }
            StaticNominalOrigin::Dependency(owner) => {
                let owner = match owner {
                    SourceNominalId::Concrete(id) => NominalDeclarationOwner::Concrete(id),
                    SourceNominalId::GenericTemplate(id) => {
                        NominalDeclarationOwner::GenericTemplate(id)
                    }
                };
                StaticAnnotationTarget::Dependency(AnnotationTargetV1::Nominal(owner))
            }
        };
        target.resolve(self, dependencies)
    }
}

impl<'a> StaticFieldShape<'a> {
    pub fn annotations(self, dependencies: &ImportedSemanticWorld<'a>) -> StaticAnnotations<'a> {
        self.annotation.resolve(self.owner, dependencies)
    }
}

impl<'a> StaticVariantShape<'a> {
    pub fn annotations(self, dependencies: &ImportedSemanticWorld<'a>) -> StaticAnnotations<'a> {
        let target = match self.owner.origin {
            StaticNominalOrigin::Current(NominalOwner::Enum(enumeration)) => {
                let variant = EnumVariantRef::checked(
                    &self.owner.module.enums,
                    enumeration,
                    self.index as u32,
                )
                .expect("the view iterates this enum's variants");
                StaticAnnotationTarget::Current(SourceAnnotationTarget::Variant(variant))
            }
            StaticNominalOrigin::Dependency(_) => {
                StaticAnnotationTarget::Dependency(AnnotationTargetV1::Variant(self.identity()))
            }
            _ => unreachable!("a variant belongs to an enum"),
        };
        target.resolve(self.owner, dependencies)
    }
}

impl Module {
    pub fn static_nominal_annotations<'a>(
        &'a self,
        declaration: SourceNominalId,
        world: &ImportedSemanticWorld<'a>,
    ) -> StaticAnnotations<'a> {
        match self.nominal_identities.declaration(declaration) {
            Some(owner) => StaticAnnotations::current(self, SourceAnnotationTarget::Nominal(owner)),
            None => world
                .static_nominal_annotations(declaration)
                .expect("a nominal query retains its source declaration"),
        }
    }
}
