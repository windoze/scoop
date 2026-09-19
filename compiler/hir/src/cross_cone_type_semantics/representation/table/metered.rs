use super::*;
use crate::MeteredNominalRepresentationResolutionError;

impl DecodedCanonicalNominalRepresentationSupportV1 {
    pub fn resolve_metered<R: NominalRepresentationResolver<E>, E>(
        self,
        resolver: &mut R,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<CanonicalNominalRepresentationSupportV1, NominalRepresentationTableResolutionError<E>>
    {
        use NominalRepresentationTableResolutionError as Error;
        meter
            .check_table_entries(self.records.len() as u64, path)
            .map_err(Error::Resource)?;
        meter
            .charge_work(self.records.len() as u64, path)
            .map_err(Error::Resource)?;
        for (index, record) in self.records.iter().enumerate() {
            record
                .charge_resolution_at(meter, &path.clone().index(index as u64))
                .map_err(Error::Resource)?;
        }
        let mut records = Vec::new();
        meter
            .try_reserve_collection_slots(&mut records, self.records.len(), path)
            .map_err(Error::Resource)?;
        for (index, decoded) in self.records.into_iter().enumerate() {
            let at = path.clone().index(index as u64);
            let record = decoded
                .resolve_precharged(resolver, meter, &at)
                .map_err(|error| match error {
                    MeteredNominalRepresentationResolutionError::Resource(error) => {
                        Error::Resource(error)
                    }
                    MeteredNominalRepresentationResolutionError::Value(source) => {
                        Error::Record { index, source }
                    }
                })?;
            meter.charge_work(32, &at).map_err(Error::Resource)?;
            if records
                .last()
                .is_some_and(|previous: &NominalRepresentationSupportV1| {
                    previous.owner() >= record.owner()
                })
            {
                return Err(Error::Order(NominalRepresentationTableOrderError {
                    index,
                    owner: record.owner(),
                }));
            }
            records.push(record);
        }
        Ok(CanonicalNominalRepresentationSupportV1 { records })
    }
}
