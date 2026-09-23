use super::*;
use scoop_identity::PropertyOwner;

impl<'f> DefaultTargetIdentityQueriesV1<'f> {
    /// Borrows the provider's actual source key and checks it against the same
    /// identity graph used for the target route. An accessor borrows its logical
    /// property's lexical key; visibility still belongs to the accessor itself.
    pub fn source_declaration_key(
        &self,
        subject: Subject,
        meter: &mut BudgetMeter,
    ) -> Result<&'f SourceDeclarationKey, Error> {
        let mut query = Query {
            foundation: self,
            meter,
            path: WirePath::root(),
        };
        let lexical_subject = match subject {
            Subject::PropertyAccessor(id) => query.accessor_property(id)?,
            _ => subject,
        };
        query.declaration(lexical_subject)
    }
}

impl<'f> Query<'_, 'f, '_> {
    pub(super) fn accessor_property(
        &mut self,
        id: PersistentPropertyAccessorId,
    ) -> Result<Subject, Error> {
        let subject = Subject::PropertyAccessor(id);
        let key = self.key(
            self.foundation
                .foundation
                .as_canonical()
                .type_source_accessor_records(),
            id,
            || Error::MissingDeclaration(subject),
        )?;
        self.meter.charge_edges(1, &self.path)?;
        Ok(match key.owner() {
            PropertyOwner::Property(id) => Subject::Property(id),
            PropertyOwner::ExtensionProperty(id) => Subject::ExtensionProperty(id),
        })
    }

    pub(super) fn key<I, K>(
        &mut self,
        records: &'f [CborIdentityRecord<I, K>],
        id: I,
        missing: impl FnOnce() -> Error,
    ) -> Result<&'f K, Error>
    where
        I: PersistentId + 'static,
        K: CborIdentityKey<I> + Eq + Clone + Send + Sync + 'static,
    {
        self.meter
            .check_table_entries(records.len() as u64, &self.path)?;
        // Foundation records may be dependency-first rather than ID-sorted.
        // Charge the complete scan without copying or reordering the artifact.
        self.meter
            .charge_work((records.len() as u64).saturating_mul(65), &self.path)?;
        let record = records
            .iter()
            .find(|record| record.id() == id)
            .ok_or_else(missing)?;
        let key = record.key();
        binding_keys::verify(id, key, self.foundation.identities, self.meter, &self.path)?;
        Ok(key)
    }

    pub(super) fn declaration(&mut self, id: Subject) -> Result<&'f SourceDeclarationKey, Error> {
        let canonical = self.foundation.foundation.as_canonical();
        let key = match id {
            Subject::Type(id) => self.key(canonical.type_source_nominal_records(), id, || {
                Error::MissingDeclaration(Subject::Type(id))
            })?,
            Subject::GenericType(id) => {
                self.key(canonical.type_source_generic_records(), id, || {
                    Error::MissingDeclaration(Subject::GenericType(id))
                })?
            }
            Subject::Property(id) => {
                self.key(canonical.type_source_property_records(), id, || {
                    Error::MissingDeclaration(Subject::Property(id))
                })?
            }
            Subject::Constructor(id) => {
                self.key(canonical.type_source_constructor_records(), id, || {
                    Error::MissingDeclaration(Subject::Constructor(id))
                })?
            }
            Subject::Function(id) => {
                self.key(canonical.type_source_function_records(), id, || {
                    Error::MissingDeclaration(Subject::Function(id))
                })?
            }
            Subject::GenericFunction(id) => {
                self.key(canonical.type_source_generic_function_records(), id, || {
                    Error::MissingDeclaration(Subject::GenericFunction(id))
                })?
            }
            Subject::ExtensionProperty(id) => self.key(
                canonical.type_source_extension_property_records(),
                id,
                || Error::MissingDeclaration(Subject::ExtensionProperty(id)),
            )?,
            other => return Err(Error::DeclarationRole(other)),
        };
        NominalRepresentationSupportV1::charge_source_key_resources(key, self.meter, &self.path)?;
        if key.origin() != self.foundation.provider {
            return Err(Error::ForeignDeclaration(id));
        }
        Ok(key)
    }

    pub(super) fn nominal(
        &mut self,
        owner: SourceNominalId,
        kind: SourceDeclarationKind,
    ) -> Result<Subject, Error> {
        let id = subject(owner);
        let key = self.declaration(id)?;
        if key.declaration_kind() != kind {
            return Err(Error::NominalKind {
                owner,
                expected: kind,
            });
        }
        Ok(id)
    }
}
