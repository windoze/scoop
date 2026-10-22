use super::*;

impl ReferenceBuilder {
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
