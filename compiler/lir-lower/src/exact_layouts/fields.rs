use scoop_identity::{FieldIdentityKey, PersistentFieldId};

use super::*;

type FieldRecord = CborIdentityRecord<PersistentFieldId, FieldIdentityKey>;

pub(super) struct Fields {
    identities: Vec<FieldRecord>,
    values: Vec<Arc<lir::ExactValueLayoutV1>>,
}
impl Fields {
    pub(super) fn inputs<'a>(&'a self) -> Result<Vec<lir::NominalLayoutFieldInputV1<'a>>> {
        let mut inputs = Vec::new();
        scoop_wire::allocation::try_reserve(&mut inputs, self.identities.len(), &WirePath::root())?;
        for (field, value) in self.identities.iter().zip(&self.values) {
            inputs.push(lir::NominalLayoutFieldInputV1 { field, value });
        }
        Ok(inputs)
    }
}
impl Projection<'_> {
    pub(super) fn fields(&mut self, source: &[mir::MirRepresentationFieldV1]) -> Result<Fields> {
        let mut identities = self.reserve(source.len())?;
        let mut values = self.reserve(source.len())?;
        for field in source {
            identities.push(self.identities.canonical_record(field.field)?);
            values.push(self.value_dependency(field.value)?);
        }
        Ok(Fields { identities, values })
    }
}
