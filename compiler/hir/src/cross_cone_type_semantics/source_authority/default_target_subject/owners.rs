use super::*;

impl Query<'_, '_> {
    pub(super) fn applied_owner(
        &mut self,
        owner: SourceNominalId,
        ty: &SignatureTypeKey,
    ) -> Result<(), Error> {
        match (owner, ty) {
            (SourceNominalId::Concrete(expected), SignatureTypeKey::Nominal(actual))
                if expected == *actual =>
            {
                Ok(())
            }
            (
                SourceNominalId::GenericTemplate(expected),
                SignatureTypeKey::NominalApplication { origin, arguments },
            ) if expected == *origin => {
                let count = arguments.as_slice().len() as u64;

                let key = self.declaration(subject(owner))?;
                let expected = key.duplicate_signature().type_parameter_count();
                if count != u64::from(expected) {
                    return Err(Error::AppliedOwnerArity {
                        owner,
                        expected,
                        actual: count,
                    });
                }
                Ok(())
            }
            _ => Err(Error::AppliedOwner(owner)),
        }
    }
}
