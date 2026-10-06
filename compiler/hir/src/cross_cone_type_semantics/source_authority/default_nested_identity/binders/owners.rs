use super::*;
use scoop_identity::InitializationUnitKey;

impl Arity<'_, '_> {
    pub(super) fn owner(&mut self, owner: &DefinitionOwnerAtom) -> Result<u32, Error> {
        use DefinitionOwnerAtom as O;
        let callable = match *owner {
            O::Type(id) => return self.nominal(SourceNominalId::Concrete(id)),
            O::GenericType(id) => return self.nominal(SourceNominalId::GenericTemplate(id)),
            O::Property(id) => return self.property(PropertyOwner::Property(id)),
            O::ExtensionProperty(id) => {
                return self.property(PropertyOwner::ExtensionProperty(id));
            }
            O::Function(id) => CallableTemplateOwner::Function(id),
            O::GenericFunction(id) => CallableTemplateOwner::GenericFunction(id),
            O::Constructor(id) => CallableTemplateOwner::Constructor(id),
            O::PropertyAccessor(id) => CallableTemplateOwner::Accessor(id),
            O::GeneratedCallable(id) => CallableTemplateOwner::Generated(id),
            O::EnumVariant(id) => CallableTemplateOwner::VariantConstructor(id),
        };
        self.callable(callable)
    }
    pub(super) fn generated(&mut self, key: &GeneratedCallableKey) -> Result<u32, Error> {
        match key {
            GeneratedCallableKey::Lexical { parent, .. }
            | GeneratedCallableKey::CallableReferenceInvoke { parent, .. } => {
                self.callable(parent.template())
            }
            GeneratedCallableKey::ZeroArgumentConstructorAdapter { constructor } => {
                self.callable(CallableTemplateOwner::Constructor(*constructor))
            }
            GeneratedCallableKey::Initialization { unit, .. } => {
                let key = self.lookup(
                    self.foundation
                        .foundation
                        .type_source_initialization_records(),
                    *unit,
                )?;
                match *key {
                    InitializationUnitKey::TopLevelProperty(id) => {
                        self.property(PropertyOwner::Property(id))
                    }
                    InitializationUnitKey::ExtensionProperty(id)
                    | InitializationUnitKey::GenericDelegatedExtensionApplication {
                        property: id,
                        ..
                    } => self.property(PropertyOwner::ExtensionProperty(id)),
                    InitializationUnitKey::GenericCompanionTemplate(id)
                    | InitializationUnitKey::GenericCompanionApplication {
                        companion: id, ..
                    } => self.nominal(SourceNominalId::GenericTemplate(id)),
                    InitializationUnitKey::Object(id) | InitializationUnitKey::Companion(id) => {
                        self.nominal(SourceNominalId::Concrete(id))
                    }
                }
            }
            _ => Err(failure(self.identity, Failure::LexicalParent)),
        }
    }
}
