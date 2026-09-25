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
        source: &crate::CallableDeclarationRecordV1,
    ) -> Result<(), Error> {
        match source.owner().nominal_owner() {
            Some(SourceNominalId::Concrete(owner)) => self.select(owner, Kind::Signature)?,
            Some(SourceNominalId::GenericTemplate(_)) => return Err(Error::NonConcreteSignature),
            None => {
                if let Some(receiver) = source.receiver() {
                    self.signature(receiver, Kind::Signature)?;
                }
            }
        }
        for parameter in source.parameters().parameters() {
            self.signature(parameter.value_type(), Kind::Signature)?;
        }
        self.signature(source.result(), Kind::Signature)
    }
}
