use super::*;

impl Lowerer {
    pub(super) fn query_struct_constructor_identity(
        &mut self,
        constructor: hir::StructConstructorId,
    ) -> Result<hir::HirSourceConstructorIdentity, Diagnostic> {
        if let Some(record) = self.struct_constructor_identity_records.get(&constructor) {
            return Ok(record.clone());
        }
        let value = &self.struct_constructors[constructor];
        let nominals = self
            .nominal_identities
            .as_ref()
            .expect("constructor identities follow nominal declarations");
        let owner = nominals[value.owner]
            .source()
            .expect("source constructors have source nominal owners");
        let record = self.query_constructor_record(
            owner,
            &self.structs[value.owner].type_params,
            &value.parameters,
            value.origin,
        )?;
        self.struct_constructor_identity_records
            .insert(constructor, record.clone());
        Ok(record)
    }

    pub(super) fn query_class_constructor_identity(
        &mut self,
        constructor: hir::ClassConstructorId,
    ) -> Result<hir::HirSourceConstructorIdentity, Diagnostic> {
        if let Some(record) = self.class_constructor_identity_records.get(&constructor) {
            return Ok(record.clone());
        }
        let value = &self.class_constructors[constructor];
        let nominals = self
            .nominal_identities
            .as_ref()
            .expect("constructor identities follow nominal declarations");
        let owner = nominals[value.owner]
            .source()
            .expect("source constructors have source nominal owners");
        let record = self.query_constructor_record(
            owner,
            &self.classes[value.owner].type_params,
            &value.parameters,
            value.origin,
        )?;
        self.class_constructor_identity_records
            .insert(constructor, record.clone());
        Ok(record)
    }

    fn query_constructor_record(
        &self,
        owner: &hir::HirSourceNominalIdentity,
        type_parameters: &[hir::TypeParamDecl],
        parameters: &[hir::ConstructorParameter],
        origin: hir::DefinitionOrigin,
    ) -> Result<hir::HirSourceConstructorIdentity, Diagnostic> {
        let nominals = self
            .nominal_identities
            .as_ref()
            .expect("constructor identities follow nominal declarations");
        hir::derive_source_constructor_identity(
            crate::persistent_types::identity_inputs(self, nominals, self.query_core_types()),
            owner,
            type_parameters,
            parameters,
        )
        .map_err(|error| Diagnostic::at_file(origin.file as usize, origin.span, error.to_string()))
    }
}
