use super::*;
use scoop_identity::PropertyOwner;

impl<'f> DefaultTargetIdentityQueriesV1<'f> {
    /// The slot key must occur in this provider's original foundation and agree
    /// with the same identity graph. Inheritance and dispatch remain separate.
    pub fn source_dispatch_slot_key(
        &self,
        slot: PersistentDispatchSlotId,
    ) -> Result<&'f DispatchSlotKey, Error> {
        Query { foundation: self }.key(
            self.foundation
                .as_canonical()
                .type_source_dispatch_records(),
            slot,
            || Error::MissingSlot(slot),
        )
    }

    /// Borrows the provider's already validated source key. An accessor uses its logical
    /// property's lexical key; visibility still belongs to the accessor itself.
    pub fn source_declaration_key(
        &self,
        subject: Subject,
    ) -> Result<&'f SourceDeclarationKey, Error> {
        let mut query = Query { foundation: self };
        let lexical_subject = match subject {
            Subject::PropertyAccessor(id) => query.accessor_property(id)?,
            _ => subject,
        };
        query.declaration(lexical_subject)
    }
}

impl<'f> Query<'_, 'f> {
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
        // Foundation records may be dependency-first rather than ID-sorted.

        let record = records
            .iter()
            .find(|record| record.id() == id)
            .ok_or_else(missing)?;
        let key = record.key();
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
