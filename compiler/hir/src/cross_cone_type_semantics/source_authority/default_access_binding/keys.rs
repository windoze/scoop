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
    pub(super) fn new(
        foundation: &BoundTypeFoundationSourcesV1<'f>,
        meter: &mut BudgetMeter,
    ) -> Result<Self, Error> {
        let canonical = foundation.foundation.as_canonical();
        let path = WirePath::root();
        Ok(Self {
            types: binding_keys::index(canonical.type_source_nominal_records(), meter, &path)?,
            generic_types: binding_keys::index(
                canonical.type_source_generic_records(),
                meter,
                &path,
            )?,
            functions: binding_keys::index(canonical.type_source_function_records(), meter, &path)?,
            generic_functions: binding_keys::index(
                canonical.type_source_generic_function_records(),
                meter,
                &path,
            )?,
            constructors: binding_keys::index(
                canonical.type_source_constructor_records(),
                meter,
                &path,
            )?,
            properties: binding_keys::index(
                canonical.type_source_property_records(),
                meter,
                &path,
            )?,
            extensions: binding_keys::index(
                canonical.type_source_extension_property_records(),
                meter,
                &path,
            )?,
            accessors: binding_keys::index(canonical.type_source_accessor_records(), meter, &path)?,
            identities: foundation.identities,
        })
    }
    pub(super) fn get(
        &self,
        subject: Subject,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<&'f SourceDeclarationKey, Error> {
        match subject {
            Subject::Type(id) => self.source(id, subject, &self.types, meter, path),
            Subject::GenericType(id) => self.source(id, subject, &self.generic_types, meter, path),
            Subject::Function(id) => self.source(id, subject, &self.functions, meter, path),
            Subject::GenericFunction(id) => {
                self.source(id, subject, &self.generic_functions, meter, path)
            }
            Subject::Constructor(id) => self.source(id, subject, &self.constructors, meter, path),
            Subject::Property(id) => self.source(id, subject, &self.properties, meter, path),
            Subject::ExtensionProperty(id) => {
                self.source(id, subject, &self.extensions, meter, path)
            }
            Subject::PropertyAccessor(id) => {
                query(self.accessors.len(), meter, path)?;
                let key = self
                    .accessors
                    .get(&id)
                    .copied()
                    .ok_or(Error::MissingKey(subject))?;
                binding_keys::verify(id, key, self.identities, meter, path)?;
                match key.owner() {
                    PropertyOwner::Property(id) => {
                        self.source(id, Subject::Property(id), &self.properties, meter, path)
                    }
                    PropertyOwner::ExtensionProperty(id) => self.source(
                        id,
                        Subject::ExtensionProperty(id),
                        &self.extensions,
                        meter,
                        path,
                    ),
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
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<&'f SourceDeclarationKey, Error>
    where
        I: PersistentId + 'static,
        SourceDeclarationKey: CborIdentityKey<I>,
    {
        query(index.len(), meter, path)?;
        let key = index.get(&id).copied().ok_or(Error::MissingKey(subject))?;
        NominalRepresentationSupportV1::charge_source_key_resources(key, meter, path)?;
        binding_keys::verify(id, key, self.identities, meter, path)?;
        Ok(key)
    }
}
