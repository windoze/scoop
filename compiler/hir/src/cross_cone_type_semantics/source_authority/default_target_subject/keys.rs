use super::*;

impl<'f> Query<'_, 'f, '_> {
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
            other => return Err(Error::DeclarationRole(other)),
        };
        NominalRepresentationSupportV1::charge_source_key_resources(key, self.meter, &self.path)?;
        if key.origin() != self.foundation.source().entries().provider {
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
