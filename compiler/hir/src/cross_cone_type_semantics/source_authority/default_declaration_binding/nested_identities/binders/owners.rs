use super::*;
use scoop_identity::InitializationUnitKey;

impl Arity<'_, '_> {
    pub(super) fn owner(&mut self, owner: &DefinitionOwnerAtom, depth: u64) -> Result<u32, Error> {
        use DefinitionOwnerAtom as O;
        let callable = match *owner {
            O::Type(id) => return self.nominal(SourceNominalId::Concrete(id), depth),
            O::GenericType(id) => return self.nominal(SourceNominalId::GenericTemplate(id), depth),
            O::Property(id) => return self.property(PropertyOwner::Property(id), depth),
            O::ExtensionProperty(id) => {
                return self.property(PropertyOwner::ExtensionProperty(id), depth);
            }
            O::Function(id) => CallableTemplateOwner::Function(id),
            O::GenericFunction(id) => CallableTemplateOwner::GenericFunction(id),
            O::Constructor(id) => CallableTemplateOwner::Constructor(id),
            O::PropertyAccessor(id) => CallableTemplateOwner::Accessor(id),
            O::GeneratedCallable(id) => CallableTemplateOwner::Generated(id),
            O::EnumVariant(id) => CallableTemplateOwner::VariantConstructor(id),
        };
        self.callable(callable, depth)
    }
    pub(super) fn generated(
        &mut self,
        key: &GeneratedCallableKey,
        depth: u64,
    ) -> Result<u32, Error> {
        match key {
            GeneratedCallableKey::Lexical { parent, .. }
            | GeneratedCallableKey::CallableReferenceInvoke { parent, .. } => {
                self.callable(parent.template(), depth)
            }
            GeneratedCallableKey::ZeroArgumentConstructorAdapter { constructor } => {
                self.callable(CallableTemplateOwner::Constructor(*constructor), depth)
            }
            GeneratedCallableKey::Initialization { unit, .. } => {
                self.enter(depth)?;
                let key = self.lookup(
                    self.foundation
                        .foundation
                        .as_canonical()
                        .type_source_initialization_records(),
                    *unit,
                )?;
                match *key {
                    InitializationUnitKey::TopLevelProperty(id) => {
                        self.property(PropertyOwner::Property(id), depth + 1)
                    }
                    InitializationUnitKey::ExtensionProperty(id)
                    | InitializationUnitKey::GenericDelegatedExtensionApplication {
                        property: id,
                        ..
                    } => self.property(PropertyOwner::ExtensionProperty(id), depth + 1),
                    InitializationUnitKey::Object(id) | InitializationUnitKey::Companion(id) => {
                        self.nominal(SourceNominalId::Concrete(id), depth + 1)
                    }
                }
            }
            _ => Err(failure(self.identity, Failure::LexicalParent)),
        }
    }
}
