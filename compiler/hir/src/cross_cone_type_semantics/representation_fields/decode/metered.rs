use super::*;
use scoop_wire::{BudgetMeter, WirePath};

mod keys;

#[derive(Debug)]
pub enum MeteredRepresentationFieldResolutionError<E, I: PersistentId> {
    Resource(WireError),
    Value(RepresentationFieldResolutionError<E, I>),
}
impl<E: fmt::Display, I: PersistentId> fmt::Display
    for MeteredRepresentationFieldResolutionError<E, I>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Value(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static, I: PersistentId> std::error::Error
    for MeteredRepresentationFieldResolutionError<E, I>
{
}

macro_rules! metered_field {
    ($decoded:ident, $resolved:ident, $id:ty, $key:ty) => {
        impl $decoded {
            pub fn charge_resolution_at(
                &self,
                meter: &mut BudgetMeter,
                path: &WirePath,
                depth: u64,
            ) -> Result<(), WireError> {
                meter.check_semantic_depth(depth, path)?;
                meter.charge_nodes(1, path)?;
                meter.charge_edges(2, path)?;
                meter.charge_work(33, path)?;
                let depth = depth.checked_add(1).ok_or_else(|| keys::overflow(path))?;
                meter.check_semantic_depth(depth, &path.clone().field(2))?;
                self.0
                    .value_type
                    .charge_resolution_at(meter, &path.clone().field(2), depth)
            }
            pub fn resolve_metered<R, E>(
                self,
                resolver: &mut R,
                meter: &mut BudgetMeter,
                path: &WirePath,
            ) -> Result<$resolved, MeteredRepresentationFieldResolutionError<E, $id>>
            where
                R: SignatureTypeReferenceResolver<E> + PersistentKeyResolver<$id, $key, Error = E>,
            {
                self.charge_resolution_at(meter, path, 1)
                    .map_err(MeteredRepresentationFieldResolutionError::Resource)?;
                self.resolve_precharged(resolver, meter, path)
            }
            /// Used only after the enclosing record's complete typed preflight.
            pub(crate) fn resolve_precharged<R, E>(
                self,
                resolver: &mut R,
                meter: &mut BudgetMeter,
                path: &WirePath,
            ) -> Result<$resolved, MeteredRepresentationFieldResolutionError<E, $id>>
            where
                R: SignatureTypeReferenceResolver<E> + PersistentKeyResolver<$id, $key, Error = E>,
            {
                use MeteredRepresentationFieldResolutionError as Error;
                let key = resolver
                    .resolve_key(self.0.field)
                    .map_err(|e| Error::Value(RepresentationFieldResolutionError::Key(e)))?;
                keys::KeyBudget::charge(key.as_ref(), meter, &path.clone().field(1))
                    .map_err(Error::Resource)?;
                let value_type = self
                    .0
                    .value_type
                    .resolve(resolver)
                    .map_err(|e| Error::Value(RepresentationFieldResolutionError::Type(e)))?;
                let record = $resolved::try_new(&key, value_type)
                    .map_err(|e| Error::Value(RepresentationFieldResolutionError::Field(e)))?;
                self.0
                    .field
                    .verify(record.field())
                    .map_err(|e| Error::Value(RepresentationFieldResolutionError::Identity(e)))?;
                Ok(record)
            }
        }
    };
}
metered_field!(
    DecodedStructRepresentationFieldV1,
    StructRepresentationFieldV1,
    PersistentFieldId,
    FieldIdentityKey
);
metered_field!(
    DecodedClassRepresentationFieldV1,
    ClassRepresentationFieldV1,
    PersistentFieldId,
    FieldIdentityKey
);
metered_field!(
    DecodedEnumRepresentationFieldV1,
    EnumRepresentationFieldV1,
    PersistentEnumVariantFieldId,
    EnumVariantFieldKey
);
