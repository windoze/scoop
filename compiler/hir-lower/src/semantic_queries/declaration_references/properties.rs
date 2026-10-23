use super::*;
use hir::{AnnotationCallableV1 as Callable, SourceAnnotationCallable as SourceCallable};
use scoop_identity::{CallableTemplateOrigin as Declaration, PropertyOwner};

impl DeclarationReferences {
    pub(super) fn properties(&mut self, lowerer: &Lowerer, data: &body_projection::ProjectionData) {
        for (property, declaration) in lowerer.properties.iter() {
            let owner = data.properties[property].property_owner();
            let reference = match owner {
                PropertyOwner::Property(id) => Target::Property(id),
                PropertyOwner::ExtensionProperty(id) => Target::ExtensionProperty(id),
            };
            let file = lowerer
                .property_definition_file(property)
                .expect("source property has its definition file");
            let location = |span| hir::SemanticSourceLocation {
                source: lowerer.source_identity(file).clone(),
                span,
            };
            self.current(
                reference,
                Source::Property(property),
                location(declaration.span),
            );
            let has_receiver = !matches!(declaration.owner, hir::PropertyOwner::TopLevel);
            if has_receiver {
                self.current(
                    Target::PropertyReceiver(owner),
                    Source::Receiver(hir::SourceAnnotationReceiver::Property(property)),
                    location(declaration.span),
                );
            }
            let getter = declaration.capability.getter();
            self.accessor(
                Callable::Declaration(Declaration::Accessor(data.accessors[getter].id())),
                SourceCallable::Getter(getter),
                has_receiver,
                location(lowerer.property_getters[getter].span),
            );
            if let Some(setter) = declaration.capability.setter() {
                let callable =
                    Callable::Declaration(Declaration::Accessor(data.accessors[setter].id()));
                let source = SourceCallable::Setter(setter);
                let origin = location(lowerer.property_setters[setter].span);
                self.accessor(callable, source, has_receiver, origin.clone());
                let reference = Target::Parameter { callable, index: 0 };
                self.declarations
                    .entry(reference.into())
                    .or_insert(hir::SemanticDeclaration {
                        reference,
                        annotations: hir::SemanticAnnotationTarget::Current(Source::Parameter {
                            callable: source,
                            index: 0,
                        }),
                        origin,
                    });
            }
            for site in [
                scoop_ast::AnnotationUseSiteTarget::Field,
                scoop_ast::AnnotationUseSiteTarget::Delegate,
            ] {
                if let Some(target @ Source::PropertyStorage { role, .. }) =
                    lowerer.property_annotation_site(property, site)
                {
                    self.current(
                        Target::PropertyStorage {
                            property: owner,
                            role,
                        },
                        target,
                        location(declaration.span),
                    );
                }
            }
        }
    }

    fn accessor(
        &mut self,
        callable: Callable,
        source: SourceCallable,
        receiver: bool,
        origin: hir::SemanticSourceLocation,
    ) {
        self.current(
            Target::Callable(callable),
            Source::Callable(source),
            origin.clone(),
        );
        if receiver {
            self.current(
                Target::CallableReceiver(callable),
                Source::Receiver(hir::SourceAnnotationReceiver::Callable(source)),
                origin,
            );
        }
    }
}
