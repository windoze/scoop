use scoop_identity::{FieldIdentityKey, PersistentFieldId};

use super::*;

pub(super) struct Fields {
    identities: Vec<CborIdentityRecord<PersistentFieldId, FieldIdentityKey>>,
    values: Vec<Arc<lir::ExactValueLayoutV1>>,
}

impl Fields {
    pub(super) fn inputs(
        &self,
        meter: &mut BudgetMeter,
    ) -> Result<Vec<lir::NominalLayoutFieldInputV1<'_>>> {
        let mut inputs = Vec::new();
        meter.try_reserve_collection_slots(
            &mut inputs,
            self.identities.len(),
            &WirePath::root(),
        )?;
        inputs.extend(
            self.identities
                .iter()
                .zip(&self.values)
                .map(|(field, value)| lir::NominalLayoutFieldInputV1 { field, value }),
        );
        Ok(inputs)
    }
}

impl Replay<'_, '_> {
    pub(super) fn fields(
        &mut self,
        fields: &[mir::MirRepresentationFieldV1],
        depth: u64,
    ) -> Result<Fields> {
        let mut identities = self.reserve(fields.len())?;
        let mut values = self.reserve(fields.len())?;
        for field in fields {
            self.meter.charge_work(1, &WirePath::root())?;
            identities.push(self.identities.canonical_record(field.field)?);
            values.push(self.value_dependency(field.value, depth)?);
        }
        Ok(Fields { identities, values })
    }
}
