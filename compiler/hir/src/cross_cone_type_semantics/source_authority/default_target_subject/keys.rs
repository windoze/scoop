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
        self.meter.charge_work(
            (u64::from(records.len().max(1).ilog2()) + 1) * 65,
            &self.path,
        )?;
        let index = records
            .binary_search_by_key(&id, CborIdentityRecord::id)
            .map_err(|_| missing())?;
        let key = records[index].key();
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
