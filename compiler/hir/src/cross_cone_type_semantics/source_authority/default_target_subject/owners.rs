use super::*;

impl Query<'_, '_, '_> {
    pub(super) fn applied_owner(
        &mut self,
        owner: SourceNominalId,
        ty: &SignatureTypeKey,
    ) -> Result<(), Error> {
        self.meter.charge_edges(1, &self.path)?;
        self.meter.charge_nodes(1, &self.path)?;
        self.meter.charge_work(65, &self.path)?;
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
                self.meter.check_table_entries(count, &self.path)?;
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
