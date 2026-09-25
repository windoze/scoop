use super::*;

impl DefaultTargetIdentityQueriesV1<'_> {
    /// Resolves the logical property governing a global reference. Storage and
    /// const classification still require the complete property contract.
    pub fn default_global_access_subject(
        &self,
        id: PersistentPropertyId,
    ) -> Result<Subject, Error> {
        let mut query = Query { foundation: self };

        let subject = Subject::Property(id);
        let key = query.declaration(subject)?;
        if !key.owners().owners().is_empty()
            || matches!(key.scope(), DeclarationScope::LexicalScoped { .. })
        {
            return Err(Error::GlobalScope(id));
        }
        Ok(subject)
    }
}
