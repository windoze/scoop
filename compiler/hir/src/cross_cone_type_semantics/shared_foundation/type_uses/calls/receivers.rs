use super::*;

impl Graph<'_> {
    pub(super) fn source_receiver(
        &mut self,
        receiver: crate::SourceCallReceiver<PersistentExactTypeId>,
        meter: &mut BudgetMeter,
    ) -> Result<(), Error> {
        let crate::SourceCallReceiver::Receiver { static_type } = receiver else {
            return Ok(());
        };
        let nominals = crate::collect_type_site_nominals(
            static_type,
            |exact| {
                self.current
                    .identities
                    .canonical_key::<_, ExactTypeKey>(exact)
            },
            meter,
        )
        .map_err(|error| match error {
            crate::HirTypeSiteExactError::Resource(error) => Error::Resource(error),
            crate::HirTypeSiteExactError::Identity(error) => Error::Identity(error),
        })?;
        for owner in nominals {
            let SourceNominalId::Concrete(owner) = owner else {
                return Err(Error::NonConcreteSignature);
            };
            self.select(owner, Kind::Signature, meter)?;
        }
        Ok(())
    }
}
