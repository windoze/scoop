use super::*;

impl Lowerer {
    pub(in crate::expr) fn imported_generic_companion_type(
        &mut self,
        qualifier: ImportedNominalQualifier,
        span: Span,
    ) -> Result<Option<TypeId>, ()> {
        let owner = self.imported_qualifier_owner(qualifier);
        let dependencies = self
            .dependencies
            .as_ref()
            .expect("dependency qualifier has a catalog");
        let declaration = dependencies
            .nominal_declaration(owner)
            .expect("dependency qualifier retains its declaration");
        let companion = if self.nominal_is_companion(owner) {
            Some(owner)
        } else {
            declaration
                .interface
                .declaration_details()
                .children()
                .values()
                .iter()
                .copied()
                .find(|child| self.nominal_is_companion(*child))
        };
        let Some(companion @ hir::SourceNominalId::GenericTemplate(_)) = companion else {
            return Ok(None);
        };
        let Some(host) = qualifier.applied() else {
            self.error(
                span,
                "generic companion access requires complete host type arguments".into(),
            );
            return Err(());
        };
        if companion == owner {
            return Ok(Some(host));
        }
        self.apply_companion_type(host, companion, span)
            .map(Some)
            .ok_or(())
    }

    pub(super) fn lower_qualified_imported_object(
        &mut self,
        value: scoop_identity::PersistentObjectValueId,
        qualifier: ImportedNominalQualifier,
        span: Span,
    ) -> Option<hir::Expr> {
        let owner = self.dependencies.as_ref()?.singleton_owner(value)?.owner();
        if matches!(owner, hir::SourceNominalId::GenericTemplate(_)) {
            let Some(host) = qualifier.applied() else {
                self.error(
                    span,
                    "generic companion access requires complete host type arguments".into(),
                );
                return None;
            };
            let ty = self.apply_companion_type(host, owner, span)?;
            self.lower_imported_singleton_application(value, ty, span)
        } else {
            self.lower_imported_singleton(value, span)
        }
    }
}
