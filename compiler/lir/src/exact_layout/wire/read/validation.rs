use super::*;

mod fields;
mod instance;
mod value;

impl DecodedExactLayoutExportV1 {
    /// The expected record has already replayed its canonical identities and
    /// checked dependencies. This checks every wire field, but does not confer
    /// source-join, export, or selected-dependency authority.
    pub fn validate_against(
        self,
        expected: &ExactLayoutExportV1,
        meter: &mut BudgetMeter,
    ) -> Result<ExactLayoutExportV1, ExactLayoutWireError> {
        meter.charge_work(7, &WirePath::root())?;
        let identity = expected.identity();
        verify(self.layout, identity.layout())?;
        verify(self.exact, identity.exact())?;
        verify(self.scan, expected.scan())?;
        let target = self
            .target
            .validate()
            .map_err(|_| ExactLayoutWireError::TargetMismatch)?;
        let target = scoop_identity::TargetProfileWireId::refine(target)
            .map_err(|_| ExactLayoutWireError::TargetMismatch)?;
        if target != identity.target().wire_id() {
            return Err(ExactLayoutWireError::TargetMismatch);
        }
        if self.role != identity.layout_key().representation() {
            return Err(ExactLayoutWireError::RoleMismatch);
        }
        if !self
            .definition
            .matches_layout_definition(identity.definition(), meter)?
        {
            return Err(ExactLayoutWireError::DefinitionMismatch);
        }
        match (self.body, expected.kind()) {
            (
                RawBody::Value {
                    storage,
                    representation,
                },
                ExactLayoutBodyKindV1::Value(value),
            ) => {
                storage.validate_against(value.value().storage(), meter)?;
                representation.validate_against(value.representation(), meter)?;
            }
            (
                RawBody::Instance {
                    shape,
                    representation,
                },
                ExactLayoutBodyKindV1::Instance(instance),
            ) => {
                shape.validate_against(instance.shape(), meter)?;
                representation.validate_against(instance.representation(), meter)?;
            }
            _ => return Err(ExactLayoutWireError::BodyMismatch),
        }
        Ok(expected.clone())
    }
}

fn verify<I: scoop_identity::PersistentId>(
    decoded: DecodedPersistentId<I>,
    expected: I,
) -> Result<(), ExactLayoutWireError> {
    decoded
        .verify(expected)
        .map(|_| ())
        .map_err(|_| ExactLayoutWireError::IdentityMismatch)
}

fn table_length(
    actual: usize,
    expected: usize,
    meter: &mut BudgetMeter,
) -> Result<(), ExactLayoutWireError> {
    if actual != expected {
        return Err(ExactLayoutWireError::TableLengthMismatch);
    }
    meter.charge_work(actual as u64, &WirePath::root())?;
    Ok(())
}

#[derive(Debug)]
pub enum ExactLayoutWireError {
    IdentityMismatch,
    TargetMismatch,
    RoleMismatch,
    DefinitionMismatch,
    BodyMismatch,
    RepresentationMismatch,
    TableLengthMismatch,
    FieldMismatch,
    RegionMismatch,
    Storage(crate::StorageWireReplayError),
    Field(crate::StorageReplayError),
    Tuple(crate::TupleStorageReplayError),
    Shape(crate::TypeInstanceShapeWireError),
    Resource(WireError),
}

macro_rules! from_error {
    ($source:ty, $variant:ident) => {
        impl From<$source> for ExactLayoutWireError {
            fn from(error: $source) -> Self {
                Self::$variant(error)
            }
        }
    };
}
from_error!(crate::StorageWireReplayError, Storage);
from_error!(crate::StorageReplayError, Field);
from_error!(crate::TupleStorageReplayError, Tuple);
from_error!(crate::TypeInstanceShapeWireError, Shape);
from_error!(WireError, Resource);
impl std::fmt::Display for ExactLayoutWireError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "layout wire differs from checked replay: {self:?}")
    }
}
impl std::error::Error for ExactLayoutWireError {}
