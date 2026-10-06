use super::*;
use crate::CanonicalHirFoundation;
use scoop_identity::{FieldIdentityView, GeneratedNominalKey, PersistentFieldId, PersistentTypeId};
use scoop_wire::WirePath;

impl CanonicalNominalInterfacesV1 {
    /// Joins complete declaration fields to the artifact's canonical field keys.
    /// This does not add private fields or their owners to public lookup.
    pub fn validate_declared_field_inventory(
        &self,
        foundation: &CanonicalHirFoundation,
    ) -> Result<(), NominalSourceFieldInventoryError> {
        let path = WirePath::root().field(2);
        let mut backing = Vec::new();
        reserve(&mut backing, self.declaration_count(), &path)?;
        for record in self.all_records() {
            if record.kind() == PublicNominalKindV1::Object {
                let key = match record.declaration() {
                    SourceNominalId::Concrete(object) => {
                        GeneratedNominalKey::ObjectBackingClass { object }
                    }
                    SourceNominalId::GenericTemplate(object) => {
                        GeneratedNominalKey::GenericObjectBackingClass { object }
                    }
                };

                let id = PersistentTypeId::from_generated_key(&key).map_err(|_| {
                    NominalSourceFieldInventoryError::ObjectOwner(record.declaration())
                })?;
                backing.push((id, record.declaration()));
            }
        }
        backing.sort_unstable();
        let keys = foundation.type_source_field_records();
        let mut expected = Vec::new();
        reserve(&mut expected, keys.len(), &path)?;
        for field in keys {
            let owner = match field.key().view() {
                FieldIdentityView::SourceDeclared { owner, .. }
                | FieldIdentityView::SourcePropertyBacking { owner, .. }
                | FieldIdentityView::SourcePropertyDelegate { owner, .. } => Some(owner),
                FieldIdentityView::Generated { owner, .. } => backing
                    .binary_search_by_key(&owner, |(id, _)| *id)
                    .ok()
                    .map(|index| backing[index].1),
            };

            if let Some(owner) = owner.filter(|owner| self.declaration(*owner).is_some()) {
                expected.push((owner, field.id()));
            }
        }
        expected.sort_unstable();
        let count = self
            .all_records()
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
        reserve(&mut actual, count, &path)?;
        for record in self.all_records() {
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

fn reserve<T>(values: &mut Vec<T>, count: usize, path: &WirePath) -> Result<(), WireError> {
    scoop_wire::allocation::try_reserve(values, count, path)
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
