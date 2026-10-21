//! Read already-bound declaration metadata without producing new implementations.

use super::*;
use hir::ImportedCallableSource;

impl FrontendQueries<'_> {
    pub(super) fn query_annotation_declaration(
        &self,
        annotation: scoop_identity::PersistentAnnotationId,
    ) -> Option<hir::SemanticAnnotationDeclaration<'_>> {
        if let Some(declaration) = self
            .lowerer
            .annotation_metadata
            .declarations
            .iter()
            .find(|declaration| declaration.identity.id() == annotation)
        {
            return Some(hir::SemanticAnnotationDeclaration::Current(declaration));
        }
        self.lowerer
            .dependencies
            .as_ref()?
            .annotation_declaration(annotation)
            .map(hir::SemanticAnnotationDeclaration::Imported)
    }

    pub(super) fn query_annotations(
        &self,
        target: hir::SemanticAnnotationTarget,
    ) -> hir::SemanticAnnotations<'_> {
        match target {
            hir::SemanticAnnotationTarget::Current(target) => hir::SemanticAnnotations::Current(
                self.lowerer
                    .annotation_metadata
                    .targets
                    .iter()
                    .find(|entry| entry.target == target)
                    .map_or(&[], |entry| entry.annotations.as_slice()),
            ),
            hir::SemanticAnnotationTarget::Imported(target) => hir::SemanticAnnotations::Imported(
                self.lowerer
                    .dependencies
                    .as_ref()
                    .map_or(&[], |dependencies| dependencies.annotations(target)),
            ),
        }
    }

    pub(super) fn query_body(
        &self,
        callable: hir::SemanticCallable,
    ) -> Option<hir::SemanticBodyAvailability<hir::SemanticBody<'_>>> {
        use hir::SemanticBodyAvailability as Availability;
        match callable {
            hir::SemanticCallable::Current(function) => {
                if self.lowerer.generation.methods.contains_key(&function) {
                    return Some(Availability::PendingGeneration);
                }
                Some(match &self.lowerer.functions[function].kind {
                    hir::FunctionKind::User(body) => {
                        Availability::Available(hir::SemanticBody::Current(body))
                    }
                    hir::FunctionKind::Abstract { .. }
                    | hir::FunctionKind::Intrinsic(_)
                    | hir::FunctionKind::Extern(_)
                    | hir::FunctionKind::InitializationEnsure
                    | hir::FunctionKind::DerivedEquality => Availability::NoBody,
                })
            }
            hir::SemanticCallable::CurrentGetter(getter) => {
                self.query_accessor_body(self.lowerer.property_getters[getter].implementation)
            }
            hir::SemanticCallable::CurrentSetter(setter) => {
                self.query_accessor_body(self.lowerer.property_setters[setter].implementation)
            }
            hir::SemanticCallable::Imported(callable) => {
                let dependencies = self.lowerer.dependencies.as_ref()?;
                if let hir::DefaultCallableDeclarationV1::PropertyAccessor(accessor) = callable {
                    let property = dependencies.property_for_accessor(accessor)?;
                    let source = property.accessors().getter_source();
                    let source = if source.accessor() == accessor {
                        source
                    } else {
                        property
                            .accessors()
                            .setter_source()
                            .expect("a catalog accessor belongs to its property's capability")
                    };
                    if matches!(
                        source.implementation(),
                        hir::PropertyAccessorImplementationV1::Storage
                            | hir::PropertyAccessorImplementationV1::Constant
                            | hir::PropertyAccessorImplementationV1::AbstractSlot
                    ) {
                        return Some(Availability::NoBody);
                    }
                }
                if let Some(body) = dependencies.callable_body(callable) {
                    return Some(Availability::Available(hir::SemanticBody::Imported(body)));
                }
                use hir::DefaultCallableDeclarationV1 as Callable;
                use scoop_identity::CallableTemplateOrigin as Origin;
                let origin = match callable {
                    Callable::Function(id) => Origin::Function(id),
                    Callable::GenericFunction(id) => Origin::GenericFunction(id),
                    Callable::PropertyAccessor(id) => Origin::Accessor(id),
                    Callable::Generated(id) => {
                        dependencies.generated_callable_definition(id)?;
                        return Some(Availability::NotPublished);
                    }
                };
                let declaration = dependencies.callable_declaration(origin).ok()?;
                let interface = declaration.interface();
                Some(
                    if interface.modality() == hir::CallableModalityV1::Abstract
                        || interface.effects().implementation()
                            != hir::CallableImplementationV1::Scoop
                    {
                        Availability::NoBody
                    } else {
                        Availability::NotPublished
                    },
                )
            }
        }
    }

    fn query_accessor_body(
        &self,
        implementation: hir::PropertyAccessorImplementation,
    ) -> Option<hir::SemanticBodyAvailability<hir::SemanticBody<'_>>> {
        match implementation {
            hir::PropertyAccessorImplementation::Body(function)
            | hir::PropertyAccessorImplementation::StorageBody(function) => {
                self.query_body(hir::SemanticCallable::Current(function))
            }
            hir::PropertyAccessorImplementation::Storage
            | hir::PropertyAccessorImplementation::Constant
            | hir::PropertyAccessorImplementation::AbstractSlot(_) => {
                Some(hir::SemanticBodyAvailability::NoBody)
            }
        }
    }
}
