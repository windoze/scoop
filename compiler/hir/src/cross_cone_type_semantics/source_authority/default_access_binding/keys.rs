use super::*;
use scoop_identity::PropertyOwner;
use scoop_identity::*;

pub(super) struct Available<'f> {
    types: BTreeMap<PersistentTypeId, &'f SourceDeclarationKey>,
    generic_types: BTreeMap<PersistentGenericTypeId, &'f SourceDeclarationKey>,
    functions: BTreeMap<PersistentFunctionId, &'f SourceDeclarationKey>,
    generic_functions: BTreeMap<PersistentGenericFunctionId, &'f SourceDeclarationKey>,
    constructors: BTreeMap<PersistentConstructorId, &'f SourceDeclarationKey>,
    properties: BTreeMap<PersistentPropertyId, &'f SourceDeclarationKey>,
    extensions: BTreeMap<PersistentExtensionPropertyId, &'f SourceDeclarationKey>,
    accessors: BTreeMap<PersistentPropertyAccessorId, &'f PropertyAccessorKey>,
    identities: &'f ValidatedIdentityGraph,
}
impl<'f> Available<'f> {
    pub(super) fn new(foundation: &BoundTypeFoundationSourcesV1<'f>) -> Result<Self, Error> {
        let canonical = foundation.foundation.as_canonical();

        Ok(Self {
            types: binding_keys::index(canonical.type_source_nominal_records())?,
            generic_types: binding_keys::index(canonical.type_source_generic_records())?,
            functions: binding_keys::index(canonical.type_source_function_records())?,
            generic_functions: binding_keys::index(
                canonical.type_source_generic_function_records(),
            )?,
            constructors: binding_keys::index(canonical.type_source_constructor_records())?,
            properties: binding_keys::index(canonical.type_source_property_records())?,
            extensions: binding_keys::index(canonical.type_source_extension_property_records())?,
            accessors: binding_keys::index(canonical.type_source_accessor_records())?,
            identities: foundation.identities,
        })
    }
    pub(super) fn get(&self, subject: Subject) -> Result<&'f SourceDeclarationKey, Error> {
        match subject {
            Subject::Type(id) => self.source(id, subject, &self.types),
            Subject::GenericType(id) => self.source(id, subject, &self.generic_types),
            Subject::Function(id) => self.source(id, subject, &self.functions),
            Subject::GenericFunction(id) => self.source(id, subject, &self.generic_functions),
            Subject::Constructor(id) => self.source(id, subject, &self.constructors),
            Subject::Property(id) => self.source(id, subject, &self.properties),
            Subject::ExtensionProperty(id) => self.source(id, subject, &self.extensions),
            Subject::PropertyAccessor(id) => {
                let key = self
                    .accessors
                    .get(&id)
                    .copied()
                    .ok_or(Error::MissingKey(subject))?;
                binding_keys::verify(id, key, self.identities)?;
                match key.owner() {
                    PropertyOwner::Property(id) => {
                        self.source(id, Subject::Property(id), &self.properties)
                    }
                    PropertyOwner::ExtensionProperty(id) => {
                        self.source(id, Subject::ExtensionProperty(id), &self.extensions)
                    }
                }
            }
            other => Err(Error::InvalidSubject(other)),
        }
    }
    fn source<I>(
        &self,
        id: I,
        subject: Subject,
        index: &BTreeMap<I, &'f SourceDeclarationKey>,
    ) -> Result<&'f SourceDeclarationKey, Error>
    where
        I: PersistentId + 'static,
        SourceDeclarationKey: CborIdentityKey<I>,
    {
        let key = index.get(&id).copied().ok_or(Error::MissingKey(subject))?;

        binding_keys::verify(id, key, self.identities)?;
        Ok(key)
    }
}
