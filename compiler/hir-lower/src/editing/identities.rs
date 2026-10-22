use crate::Lowerer;
use scoop_hir::{self as hir, EditingCallableIdentity as Callable};
use std::collections::HashMap;

mod imports;

#[derive(Default)]
pub(super) struct DeclarationIdentities {
    callables: HashMap<hir::FunctionId, Option<Callable>>,
    properties: HashMap<hir::PropertyId, Option<hir::PropertyDeclarationId>>,
}

impl DeclarationIdentities {
    pub(super) fn callee(
        &mut self,
        lowerer: &mut Lowerer,
        callee: hir::EditingCallee,
    ) -> Option<Callable> {
        use hir::{EditingCallee as C, SemanticConstructor as Constructor};
        Some(match callee {
            C::Callable(callable) => return self.callable(lowerer, callable),
            C::Constructor(Constructor::CurrentClass(constructor)) => Callable::Constructor(
                lowerer
                    .query_class_constructor_identity(constructor)
                    .ok()?
                    .id(),
            ),
            C::Constructor(Constructor::CurrentStruct(constructor)) => Callable::Constructor(
                lowerer
                    .query_struct_constructor_identity(constructor)
                    .ok()?
                    .id(),
            ),
            C::Constructor(Constructor::Imported(constructor)) => {
                Callable::Constructor(constructor)
            }
            C::CurrentVariant(variant) => {
                Callable::Variant(lowerer.enum_member_identities.as_ref()?[variant].id())
            }
            C::ImportedVariant(variant) => Callable::Variant(variant),
        })
    }

    pub(super) fn callable(
        &mut self,
        lowerer: &mut Lowerer,
        callable: hir::SemanticCallable,
    ) -> Option<Callable> {
        match callable {
            hir::SemanticCallable::Imported(declaration) => {
                Some(Callable::Declaration(declaration))
            }
            hir::SemanticCallable::Current(function) => {
                *self.callables.entry(function).or_insert_with(|| {
                    if lowerer.signatures.get(&function)?.parameter_pending
                        || lowerer
                            .declaration_surface
                            .unavailable_signature(function)
                            .is_some()
                    {
                        return None;
                    }
                    if let Some(local) = lowerer.local_function_by_function.get(&function) {
                        return Some(Callable::Local(*local));
                    }
                    if !lowerer.source_function_declarations.contains_key(&function) {
                        return None;
                    }
                    let declaration = match lowerer.query_function_identity(function).ok()? {
                        hir::HirSourceFunctionIdentity::Plain(record) => {
                            hir::DefaultCallableDeclarationV1::Function(record.id())
                        }
                        hir::HirSourceFunctionIdentity::Generic(record) => {
                            hir::DefaultCallableDeclarationV1::GenericFunction(record.id())
                        }
                    };
                    Some(Callable::Declaration(declaration))
                })
            }
            hir::SemanticCallable::CurrentGetter(_) | hir::SemanticCallable::CurrentSetter(_) => {
                None
            }
        }
    }

    pub(super) fn property(
        &mut self,
        lowerer: &mut Lowerer,
        property: hir::SemanticProperty,
    ) -> Option<hir::PropertyDeclarationId> {
        match property {
            hir::SemanticProperty::Imported(declaration) => Some(declaration),
            hir::SemanticProperty::Current(property) => {
                *self.properties.entry(property).or_insert_with(|| {
                    if lowerer.property_type_pending(property) {
                        return None;
                    }
                    lowerer
                        .complete_property_identity(property)
                        .ok()
                        .map(|identity| identity.property_owner())
                })
            }
        }
    }
}
