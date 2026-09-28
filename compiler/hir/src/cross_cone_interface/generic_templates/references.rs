use scoop_identity::{CallableTemplateOrigin, PropertyOwner};
use scoop_wire::{WireError, WirePath};

use crate::{
    DefaultBodyReferenceOccurrenceV1, DefaultBodyReferenceTargetV1, DefaultBodyReferenceVisitorV1,
    DefaultBoundCallableSourceV1, DefaultCallableDeclarationV1,
    DefaultCallableReferenceTargetViewV1 as Callable, DefaultClassConstructorIdV1,
    DefaultConstructorRefV1, DefaultConstructorReferenceTargetViewV1 as Constructor,
    DefaultExpressionV1, DefaultFieldRefV1, DefaultFieldReferenceTargetViewV1 as Field,
    ExportGenericCallableBodyV1, ExternalHirTargetV1, SignatureNominalWalker,
};

impl ExportGenericCallableBodyV1 {
    pub(crate) fn visit_declaration_targets<V, E>(
        &self,
        visitor: &mut V,
        path: &WirePath,
    ) -> Result<(), E>
    where
        V: FnMut(ExternalHirTargetV1) -> Result<(), E>,
        E: From<WireError>,
    {
        self.visit_direct_references(&mut DeclarationTargets { visitor }, path)
    }
}

impl crate::ExportGenericNominalInitializationV1 {
    pub(crate) fn visit_declaration_targets<V, E>(
        &self,
        visitor: &mut V,
        path: &WirePath,
    ) -> Result<(), E>
    where
        V: FnMut(ExternalHirTargetV1) -> Result<(), E>,
        E: From<WireError>,
    {
        self.visit_direct_references(&mut DeclarationTargets { visitor }, path)
    }
}

struct DeclarationTargets<'a, V> {
    visitor: &'a mut V,
}

impl crate::ExportGenericDelegateTemplateV1 {
    pub(crate) fn visit_declaration_targets<V, E>(
        &self,
        visitor: &mut V,
        path: &WirePath,
    ) -> Result<(), E>
    where
        V: FnMut(ExternalHirTargetV1) -> Result<(), E>,
        E: From<WireError>,
    {
        self.visit_direct_references(&mut DeclarationTargets { visitor }, path)
    }
}

impl<'body, V, E> DefaultBodyReferenceVisitorV1<'body> for DeclarationTargets<'_, V>
where
    V: FnMut(ExternalHirTargetV1) -> Result<(), E>,
    E: From<WireError>,
{
    type Error = E;

    fn expression(&mut self, _: u32, _: &'body DefaultExpressionV1, _: &WirePath) -> Result<(), E> {
        Ok(())
    }

    fn reference(
        &mut self,
        occurrence: DefaultBodyReferenceOccurrenceV1<'body>,
        path: &WirePath,
    ) -> Result<(), E> {
        let target = match occurrence.target {
            DefaultBodyReferenceTargetV1::Type(signature) => {
                let mut walker = SignatureNominalWalker::new(signature, path)?;
                while let Some(nominal) = walker.next(path)? {
                    (self.visitor)(ExternalHirTargetV1::from(nominal))?;
                }
                return Ok(());
            }
            DefaultBodyReferenceTargetV1::Callable(callable) => match callable {
                Callable::Callable(callable) => declaration(callable.declaration()),
                Callable::FunctionAddress(owner) => declaration(owner),
                Callable::Bound(bound) => match bound.source() {
                    DefaultBoundCallableSourceV1::Class { callable, .. } => {
                        declaration(callable.declaration())
                    }
                    DefaultBoundCallableSourceV1::Interface { member, .. } => {
                        ExternalHirTargetV1::Callable(*member)
                    }
                },
                Callable::LocalFunction(owner) => ExternalHirTargetV1::Callable(owner),
                Callable::Lambda(body)
                | Callable::AnonymousFunction(body)
                | Callable::CallableReference(body) => ExternalHirTargetV1::GeneratedCallable(body),
                Callable::DerivedEquality(_) => return Ok(()),
            },
            DefaultBodyReferenceTargetV1::Constructor(constructor) => match constructor {
                Constructor::Constructor(constructor) => match constructor {
                    DefaultConstructorRefV1::Struct { declaration, .. } => {
                        ExternalHirTargetV1::Callable(CallableTemplateOrigin::Constructor(
                            *declaration,
                        ))
                    }
                    DefaultConstructorRefV1::Class { declaration, .. } => match declaration {
                        DefaultClassConstructorIdV1::Source(id) => {
                            ExternalHirTargetV1::Callable(CallableTemplateOrigin::Constructor(*id))
                        }
                        DefaultClassConstructorIdV1::Generated(id) => {
                            ExternalHirTargetV1::GeneratedCallable(*id)
                        }
                    },
                    DefaultConstructorRefV1::Variant { declaration, .. } => {
                        ExternalHirTargetV1::Callable(CallableTemplateOrigin::VariantConstructor(
                            *declaration,
                        ))
                    }
                },
                Constructor::Variant(variant) => ExternalHirTargetV1::Callable(
                    CallableTemplateOrigin::VariantConstructor(variant.declaration()),
                ),
            },
            DefaultBodyReferenceTargetV1::Global(property) => {
                ExternalHirTargetV1::Property(PropertyOwner::Property(property))
            }
            DefaultBodyReferenceTargetV1::GenericDelegate(reference) => {
                ExternalHirTargetV1::Property(PropertyOwner::ExtensionProperty(
                    reference.property(),
                ))
            }
            DefaultBodyReferenceTargetV1::Singleton(value) => {
                ExternalHirTargetV1::ObjectValue(value)
            }
            DefaultBodyReferenceTargetV1::Field(field) => match field {
                Field::Field(
                    DefaultFieldRefV1::Class { declaration, .. }
                    | DefaultFieldRefV1::Struct { declaration, .. },
                ) => ExternalHirTargetV1::Field(*declaration),
                Field::Struct { declaration, .. } => ExternalHirTargetV1::Field(declaration),
                Field::Field(DefaultFieldRefV1::Tuple { .. }) => return Ok(()),
            },
        };
        (self.visitor)(target)
    }
}

fn declaration(owner: DefaultCallableDeclarationV1) -> ExternalHirTargetV1 {
    match owner {
        DefaultCallableDeclarationV1::Function(id) => {
            ExternalHirTargetV1::Callable(CallableTemplateOrigin::Function(id))
        }
        DefaultCallableDeclarationV1::GenericFunction(id) => {
            ExternalHirTargetV1::Callable(CallableTemplateOrigin::GenericFunction(id))
        }
        DefaultCallableDeclarationV1::PropertyAccessor(id) => {
            ExternalHirTargetV1::Callable(CallableTemplateOrigin::Accessor(id))
        }
        DefaultCallableDeclarationV1::Generated(id) => ExternalHirTargetV1::GeneratedCallable(id),
    }
}
