//! Counts every lexical binder, including those absent from the descriptor ABI.
use super::*;
use scoop_identity::{CallableTemplateOwner, DefinitionOwnerAtom, PropertyOwner};

mod owners;

pub(super) fn validate(
    foundation: &BoundTypeFoundationSourcesV1<'_>,
    parent: CallableTemplateOwner,
    descriptor: DefaultSourceNestedCallableDescriptorV1<'_>,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), Error> {
    let identity = descriptor.identity();
    let expected = Arity {
        foundation,
        identity,
        meter,
        path,
    }
    .callable(parent, 1)?;
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
    foundation: &'a BoundTypeFoundationSourcesV1<'f>,
    identity: Identity,
    meter: &'a mut BudgetMeter,
    path: &'a WirePath,
}
impl<'f> Arity<'_, 'f> {
    fn enter(&mut self, depth: u64) -> Result<(), Error> {
        self.meter.check_semantic_depth(depth, self.path)?;
        self.meter.charge_nodes(1, self.path)?;
        self.meter.charge_edges(1, self.path)?;
        Ok(())
    }
    fn lookup<I, K>(
        &mut self,
        records: &'f [CborIdentityRecord<I, K>],
        id: I,
    ) -> Result<&'f K, Error>
    where
        I: PersistentId + 'static,
        K: CborIdentityKey<I> + Eq + Clone + Send + Sync + 'static,
    {
        key(
            self.foundation,
            records,
            id,
            self.identity,
            self.meter,
            self.path,
        )
    }
    fn callable(&mut self, owner: CallableTemplateOwner, depth: u64) -> Result<u32, Error> {
        self.enter(depth)?;
        let canonical = self.foundation.foundation.as_canonical();
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
                return self.property(owner, depth + 1);
            }
            CallableTemplateOwner::Generated(id) => {
                let key = self.lookup(canonical.type_source_generated_callable_records(), id)?;
                return self.generated(key, depth + 1);
            }
            CallableTemplateOwner::VariantConstructor(id) => {
                let owner = self
                    .lookup(canonical.type_source_enum_variant_records(), id)?
                    .source_owner()
                    .ok_or_else(|| failure(self.identity, Failure::LexicalParent))?;
                return self.nominal(owner, depth + 1);
            }
        };
        self.declaration(source, depth)
    }
    fn declaration(&mut self, key: &SourceDeclarationKey, depth: u64) -> Result<u32, Error> {
        self.meter.charge_work(1, self.path)?;
        if key.origin() != self.foundation.source().entries().provider {
            return Err(failure(self.identity, Failure::LexicalParent));
        }
        let own = key.duplicate_signature().type_parameter_count();
        let inherited = match key.owners().owners().last() {
            Some(owner) => self.owner(owner, depth + 1)?,
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
    fn property(&mut self, owner: PropertyOwner, depth: u64) -> Result<u32, Error> {
        self.enter(depth)?;
        let canonical = self.foundation.foundation.as_canonical();
        let source = match owner {
            PropertyOwner::Property(id) => {
                self.lookup(canonical.type_source_property_records(), id)?
            }
            PropertyOwner::ExtensionProperty(id) => {
                self.lookup(canonical.type_source_extension_property_records(), id)?
            }
        };
        self.declaration(source, depth)
    }
    fn nominal(&mut self, owner: SourceNominalId, depth: u64) -> Result<u32, Error> {
        self.enter(depth)?;
        let canonical = self.foundation.foundation.as_canonical();
        let key = match owner {
            SourceNominalId::Concrete(id) => {
                self.lookup(canonical.type_source_nominal_records(), id)?
            }
            SourceNominalId::GenericTemplate(id) => {
                self.lookup(canonical.type_source_generic_records(), id)?
            }
        };
        if key.origin() != self.foundation.source().entries().provider {
            return Err(failure(self.identity, Failure::LexicalParent));
        }
        // A static nested nominal starts a fresh type-parameter scope.
        Ok(key.duplicate_signature().type_parameter_count())
    }
}
