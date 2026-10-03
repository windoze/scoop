use super::*;

impl Graph<'_> {
    pub(super) fn source_construction(
        &mut self,
        source: &crate::CallableDeclarationRecordV1,
    ) -> Result<(), Error> {
        use scoop_identity::CallableTemplateOrigin;
        let kind = match source.declaration() {
            CallableTemplateOrigin::Constructor(id) => Kind::Construct(id),
            CallableTemplateOrigin::VariantConstructor(id) => Kind::VariantConstruct(id),
            _ => return Ok(()),
        };
        let Some(SourceNominalId::Concrete(owner)) = source.owner().nominal_owner() else {
            return Err(Error::NonConcreteSignature);
        };
        self.select(owner, kind)
    }

    pub(super) fn call_signature(
        &mut self,
        call: &crate::HirDependencyCallSiteV1,
    ) -> Result<(), Error> {
        // The declaration join already checked these complete exact types.
        for &ty in call
            .arguments()
            .iter()
            .chain(std::iter::once(&call.result()))
        {
            self.exact_signature_type(ty)?;
        }
        Ok(())
    }
}
