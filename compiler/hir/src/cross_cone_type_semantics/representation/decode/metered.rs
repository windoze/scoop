use super::*;
use crate::MeteredRepresentationFieldResolutionError;
use scoop_wire::{BudgetMeter, WirePath};

mod preflight;
mod resolve;
#[cfg(test)]
mod tests;

#[derive(Debug)]
pub enum MeteredNominalRepresentationResolutionError<E> {
    Resource(WireError),
    Value(NominalRepresentationResolutionError<E>),
}
impl<E> From<WireError> for MeteredNominalRepresentationResolutionError<E> {
    fn from(error: WireError) -> Self {
        Self::Resource(error)
    }
}
impl<E: std::fmt::Display> std::fmt::Display for MeteredNominalRepresentationResolutionError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Value(error) => error.fmt(f),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error
    for MeteredNominalRepresentationResolutionError<E>
{
}

impl DecodedNominalRepresentationSupportV1 {
    pub fn resolve_metered<R: NominalRepresentationResolver<E>, E>(
        self,
        resolver: &mut R,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<NominalRepresentationSupportV1, MeteredNominalRepresentationResolutionError<E>>
    {
        self.charge_resolution_at(meter, path)?;
        self.resolve_precharged(resolver, meter, path)
    }
    /// The entire inline tree must have passed `charge_resolution_at` first.
    pub(crate) fn resolve_precharged<R: NominalRepresentationResolver<E>, E>(
        self,
        resolver: &mut R,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<NominalRepresentationSupportV1, MeteredNominalRepresentationResolutionError<E>>
    {
        use MeteredNominalRepresentationResolutionError as Error;
        let key = resolver
            .resolve_key(self.owner)
            .map_err(|e| Error::Value(NominalRepresentationResolutionError::Reference(e)))?;
        if key.duplicate_signature().type_parameter_count() != 0 {
            return Err(Error::Value(NominalRepresentationResolutionError::Record(
                NominalRepresentationBuildError::GenericTemplate,
            )));
        }
        if !key.declaration_kind().is_nominal() {
            return Err(Error::Value(NominalRepresentationResolutionError::Record(
                NominalRepresentationBuildError::SourceKind,
            )));
        }
        super::super::semantics::charge_key(&key, meter, &path.clone().field(1))?;
        let access = self
            .declaration_access
            .resolve(resolver)
            .map_err(|e| Error::Value(NominalRepresentationResolutionError::Access(e)))?;
        let shape = resolve::shape(self.shape, resolver, meter, &path.clone().field(3))?;
        let record = NominalRepresentationSupportV1::try_new(&key, access, shape)
            .map_err(|e| Error::Value(NominalRepresentationResolutionError::Record(e)))?;
        self.owner
            .verify(record.owner())
            .map_err(|e| Error::Value(NominalRepresentationResolutionError::OwnerIdentity(e)))?;
        Ok(record)
    }
}
