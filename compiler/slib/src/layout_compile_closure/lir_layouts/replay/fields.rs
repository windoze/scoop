use scoop_identity::{FieldIdentityKey, PersistentFieldId};

use super::*;

pub(super) struct Fields {
    identities: Vec<CborIdentityRecord<PersistentFieldId, FieldIdentityKey>>,
    values: Vec<Arc<lir::ExactValueLayoutV1>>,
}

impl Fields {
    pub(super) fn inputs(&self) -> Result<Vec<lir::NominalLayoutFieldInputV1<'_>>> {
        let mut inputs = Vec::new();
        scoop_wire::allocation::try_reserve(&mut inputs, self.identities.len(), &WirePath::root())?;
        inputs.extend(
            self.identities
                .iter()
                .zip(&self.values)
                .map(|(field, value)| lir::NominalLayoutFieldInputV1 { field, value }),
        );
        Ok(inputs)
    }
}

impl Replay<'_> {
    pub(super) fn fields(&mut self, fields: &[mir::MirRepresentationFieldV1]) -> Result<Fields> {
        let mut identities = self.reserve(fields.len())?;
        let mut values = self.reserve(fields.len())?;
        for field in fields {
            identities.push(self.identities.canonical_record(field.field)?);
            values.push(self.value_dependency(field.value)?);
        }
        Ok(Fields { identities, values })
    }
}
