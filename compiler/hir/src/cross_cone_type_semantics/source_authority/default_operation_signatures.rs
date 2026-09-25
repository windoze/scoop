//! Applied operation signatures from the declaration owner's bound source tables.
use super::nominal_binding::Applied;
use crate::*;
use scoop_identity::{
    CallableTemplateOrigin, DefinitionOriginSubject, OptionalSignatureType,
    PersistentConstructorId, SignatureTypeKey,
};
use scoop_wire::{WireError, WirePath};

mod constructors;
mod errors;
mod members;
pub use errors::DefaultSourceCallableOperationError;
type Error = DefaultSourceCallableOperationError;

fn owner_matches(applied: &Applied<'_, '_>, expected: SourceNominalId) -> Result<(), Error> {
    let actual = applied.source.owner();
    if actual != expected {
        return Err(Error::Owner { expected, actual });
    }
    Ok(())
}
