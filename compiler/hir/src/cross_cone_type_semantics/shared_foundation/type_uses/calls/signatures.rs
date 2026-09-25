use super::*;

impl Graph<'_> {
    pub(super) fn call_signature(
        &mut self,
        source: &crate::CallableDeclarationRecordV1,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), Error> {
        meter.charge_work(1, path)?;
        match source.owner().nominal_owner() {
            Some(SourceNominalId::Concrete(owner)) => self.select(owner, Kind::Signature, meter)?,
            Some(SourceNominalId::GenericTemplate(_)) => return Err(Error::NonConcreteSignature),
            None => {
                if let Some(receiver) = source.receiver() {
                    self.signature(receiver, Kind::Signature, 1, meter)?;
                }
            }
        }
        for parameter in source.parameters().parameters() {
            self.signature(parameter.value_type(), Kind::Signature, 1, meter)?;
        }
        self.signature(source.result(), Kind::Signature, 1, meter)
    }
}
