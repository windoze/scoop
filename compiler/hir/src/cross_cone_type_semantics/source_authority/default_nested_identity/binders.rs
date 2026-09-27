//! Counts every lexical binder, including those absent from the descriptor ABI.
use super::*;
use scoop_identity::{CallableTemplateOwner, DefinitionOwnerAtom, PropertyOwner};

mod owners;

pub(super) fn validate(
    foundation: &NestedIdentityInput<'_>,
    parent: CallableTemplateOwner,
    descriptor: DefaultSourceNestedCallableDescriptorV1<'_>,

    path: &WirePath,
) -> Result<(), Error> {
    let identity = descriptor.identity();
    let expected = Arity {
        foundation,
        identity,

        path,
    }
    .callable(parent)?;
    let actual = descriptor.owner_type_parameter_count();
    if actual != expected {
        return Err(failure(
            identity,
            Failure::OwnerBinderArity { expected, actual },
        ));
    }
    if let DefaultNestedCallableBodyArgumentsV1::Explicit(arguments) = descriptor.body_arguments() {
        let actual = arguments.len() as u32;
        if actual != expected {
            return Err(failure(
                identity,
                Failure::BodyBinderArity { expected, actual },
            ));
        }
    }
    Ok(())
}

struct Arity<'a, 'f> {
    foundation: &'a NestedIdentityInput<'f>,
    identity: Identity,

    path: &'a WirePath,
}
impl<'f> Arity<'_, 'f> {
    fn lookup<I, K>(
        &mut self,
        records: &'f [CborIdentityRecord<I, K>],
        id: I,
    ) -> Result<&'f K, Error>
    where
        I: PersistentId + 'static,
        K: CborIdentityKey<I> + Eq + Clone + Send + Sync + 'static,
    {
        key(records, id, self.identity)
    }
    fn callable(&mut self, owner: CallableTemplateOwner) -> Result<u32, Error> {
        let canonical = self.foundation.foundation;
        let source = match owner {
            CallableTemplateOwner::Function(id) => {
                self.lookup(canonical.type_source_function_records(), id)?
            }
            CallableTemplateOwner::GenericFunction(id) => {
                self.lookup(canonical.type_source_generic_function_records(), id)?
            }
            CallableTemplateOwner::Constructor(id) => {
                self.lookup(canonical.type_source_constructor_records(), id)?
            }
            CallableTemplateOwner::Accessor(id) => {
                let owner = self
                    .lookup(canonical.type_source_accessor_records(), id)?
                    .owner();
                return self.property(owner);
            }
            CallableTemplateOwner::Generated(id) => {
                let key = self.lookup(canonical.type_source_generated_callable_records(), id)?;
                return self.generated(key);
            }
            CallableTemplateOwner::VariantConstructor(id) => {
                let owner = self
                    .lookup(canonical.type_source_enum_variant_records(), id)?
                    .source_owner()
                    .ok_or_else(|| failure(self.identity, Failure::LexicalParent))?;
                return self.nominal(owner);
            }
        };
        self.declaration(source)
    }
    fn declaration(&mut self, key: &SourceDeclarationKey) -> Result<u32, Error> {
        if key.origin() != self.foundation.provider {
            return Err(failure(self.identity, Failure::LexicalParent));
        }
        let own = key.duplicate_signature().type_parameter_count();
        let inherited = match key.owners().owners().last() {
            Some(owner) => self.owner(owner)?,
            None => 0,
        };
        own.checked_add(inherited).ok_or_else(|| {
            Error::Resource(WireError::new(
                scoop_wire::WireErrorKind::IntegerOutOfRange,
                self.path.clone(),
                None,
            ))
        })
    }
    fn property(&mut self, owner: PropertyOwner) -> Result<u32, Error> {
        let canonical = self.foundation.foundation;
        let source = match owner {
            PropertyOwner::Property(id) => {
                self.lookup(canonical.type_source_property_records(), id)?
            }
            PropertyOwner::ExtensionProperty(id) => {
                self.lookup(canonical.type_source_extension_property_records(), id)?
            }
        };
        self.declaration(source)
    }
    fn nominal(&mut self, owner: SourceNominalId) -> Result<u32, Error> {
        let canonical = self.foundation.foundation;
        let key = match owner {
            SourceNominalId::Concrete(id) => {
                self.lookup(canonical.type_source_nominal_records(), id)?
            }
            SourceNominalId::GenericTemplate(id) => {
                self.lookup(canonical.type_source_generic_records(), id)?
            }
        };
        if key.origin() != self.foundation.provider {
            return Err(failure(self.identity, Failure::LexicalParent));
        }
        // A static nested nominal starts a fresh type-parameter scope.
        Ok(key.duplicate_signature().type_parameter_count())
    }
}
