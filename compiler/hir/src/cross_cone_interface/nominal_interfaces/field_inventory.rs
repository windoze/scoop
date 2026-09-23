use super::*;
use crate::CanonicalHirFoundation;
use scoop_identity::{FieldIdentityView, GeneratedNominalKey, PersistentFieldId, PersistentTypeId};
use scoop_wire::{BudgetMeter, WirePath};

impl CanonicalNominalInterfacesV1 {
    /// Joins complete declaration fields to the artifact's canonical field keys.
    /// This does not add private fields or their owners to public lookup.
    pub fn validate_declared_field_inventory(
        &self,
        foundation: &CanonicalHirFoundation,
        meter: &mut BudgetMeter,
    ) -> Result<(), NominalSourceFieldInventoryError> {
        let path = WirePath::root().field(2);
        let mut backing = Vec::new();
        reserve(&mut backing, self.records().len(), meter, &path)?;
        for record in self.records() {
            if record.kind() == PublicNominalKindV1::Object {
                let SourceNominalId::Concrete(object) = record.declaration() else {
                    return Err(NominalSourceFieldInventoryError::ObjectOwner(
                        record.declaration(),
                    ));
                };
                let key = GeneratedNominalKey::ObjectBackingClass { object };
                let bytes = scoop_wire::encoded_length(&key).map_err(|_| {
                    NominalSourceFieldInventoryError::ObjectOwner(record.declaration())
                })?;
                meter.charge_sha256(bytes, &path)?;
                let id = PersistentTypeId::from_generated_key(&key).map_err(|_| {
                    NominalSourceFieldInventoryError::ObjectOwner(record.declaration())
                })?;
                backing.push((id, record.declaration()));
            }
        }
        backing.sort_unstable();
        let keys = foundation.type_source_field_records();
        let mut expected = Vec::new();
        reserve(&mut expected, keys.len(), meter, &path)?;
        for field in keys {
            let owner = match field.key().view() {
                FieldIdentityView::SourceDeclared { owner, .. }
                | FieldIdentityView::SourcePropertyBacking { owner, .. }
                | FieldIdentityView::SourcePropertyDelegate { owner, .. } => Some(owner),
                FieldIdentityView::Generated { owner, .. } => {
                    meter.charge_work(u64::from(backing.len().max(1).ilog2()) + 1, &path)?;
                    backing
                        .binary_search_by_key(&owner, |(id, _)| *id)
                        .ok()
                        .map(|index| backing[index].1)
                }
            };
            meter.charge_work(u64::from(self.records().len().max(1).ilog2()) + 1, &path)?;
            if let Some(owner) = owner.filter(|owner| self.get(*owner).is_some()) {
                expected.push((owner, field.id()));
            }
        }
        expected.sort_unstable();
        let count = self
            .records()
            .iter()
            .try_fold(0usize, |count, record| {
                count.checked_add(record.source_shape().declared_fields().len())
            })
            .ok_or_else(|| {
                scoop_wire::WireError::new(
                    scoop_wire::WireErrorKind::IntegerOutOfRange,
                    path.clone(),
                    None,
                )
            })?;
        let mut actual = Vec::new();
        reserve(&mut actual, count, meter, &path)?;
        for record in self.records() {
            actual.extend(
                record
                    .source_shape()
                    .declared_fields()
                    .iter()
                    .map(|field| (record.declaration(), field.field())),
            );
        }
        actual.sort_unstable();
        let mut expected = expected.iter().peekable();
        let mut actual = actual.iter().peekable();
        loop {
            meter.charge_work(1, &path)?;
            match (expected.peek(), actual.peek()) {
                (Some(left), Some(right)) if left == right => {
                    expected.next();
                    actual.next();
                }
                (Some(left), right) if right.is_none_or(|right| left < right) => {
                    return Err(NominalSourceFieldInventoryError::Missing {
                        owner: left.0,
                        field: left.1,
                    });
                }
                (_, Some(right)) => {
                    return Err(NominalSourceFieldInventoryError::Extra {
                        owner: right.0,
                        field: right.1,
                    });
                }
                (None, None) => return Ok(()),
                (Some(left), None) => {
                    return Err(NominalSourceFieldInventoryError::Missing {
                        owner: left.0,
                        field: left.1,
                    });
                }
            }
        }
    }
}

fn reserve<T>(
    values: &mut Vec<T>,
    count: usize,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), WireError> {
    meter.check_table_entries(count as u64, path)?;
    meter.charge_work(
        (count as u64).saturating_mul(u64::from(count.max(1).ilog2()) + 1),
        path,
    )?;
    meter.try_reserve_collection_slots(values, count, path)
}

#[derive(Debug)]
pub enum NominalSourceFieldInventoryError {
    Resource(WireError),
    ObjectOwner(SourceNominalId),
    Missing {
        owner: SourceNominalId,
        field: PersistentFieldId,
    },
    Extra {
        owner: SourceNominalId,
        field: PersistentFieldId,
    },
}

impl From<WireError> for NominalSourceFieldInventoryError {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl std::fmt::Display for NominalSourceFieldInventoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::ObjectOwner(owner) => write!(f, "invalid source object owner {owner:?}"),
            Self::Missing { owner, field } => {
                write!(f, "nominal {owner:?} omits declared field {field}")
            }
            Self::Extra { owner, field } => {
                write!(f, "nominal {owner:?} contains undeclared field {field}")
            }
        }
    }
}
impl std::error::Error for NominalSourceFieldInventoryError {}
