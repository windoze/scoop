use super::*;

impl Lowerer {
    pub(in crate::semantic_queries) fn query_alias_identity(
        &mut self,
        id: hir::ExportTypeAliasId,
    ) -> Result<hir::HirTypeAliasIdentity, Diagnostic> {
        if let Some(identity) = self.query_alias_identities.get(&id) {
            return Ok(identity.clone());
        }
        let record = crate::persistent_aliases::record(self, id)
            .map_err(|error| Diagnostic::at_file(error.file(), error.span(), error.to_string()))?;
        self.query_alias_identities.insert(id, record.clone());
        Ok(record)
    }

    pub(in crate::semantic_queries) fn query_object_value_identity(
        &self,
        object: hir::ObjectId,
    ) -> scoop_identity::PersistentObjectValueId {
        let source = self
            .nominal_identity(Owner::Object(object))
            .source()
            .expect("a source object has its actual declaration key");
        scoop_identity::PersistentObjectValueId::from_source_object(source.declaration())
            .expect("a checked source object identifies its singleton value")
    }
}
