//! Actual dispatch choices owned by the shared source nominal declaration.

use scoop_identity::PersistentDispatchSlotId;
use scoop_wire::{Encoder, WireEncode};

use crate::InheritanceSourceSlotSelectionV1;

mod decode;
mod role;
pub use decode::{
    DecodedCanonicalNominalDispatchSelectionsV1, NominalDispatchSelectionResolutionError,
};
pub use role::NominalDispatchSelectionRoleV1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NominalDispatchSelectionV1 {
    role: NominalDispatchSelectionRoleV1,
    receiver: scoop_identity::SignatureTypeKey,
    slot: PersistentDispatchSlotId,
    selection: InheritanceSourceSlotSelectionV1,
}

impl NominalDispatchSelectionV1 {
    pub const fn new(
        role: NominalDispatchSelectionRoleV1,
        receiver: scoop_identity::SignatureTypeKey,
        slot: PersistentDispatchSlotId,
        selection: InheritanceSourceSlotSelectionV1,
    ) -> Self {
        Self {
            role,
            receiver,
            slot,
            selection,
        }
    }

    pub const fn role(&self) -> &NominalDispatchSelectionRoleV1 {
        &self.role
    }

    /// The already selected receiver application, in the nominal's binder scope.
    pub const fn receiver(&self) -> &scoop_identity::SignatureTypeKey {
        &self.receiver
    }

    pub fn key(&self) -> (&NominalDispatchSelectionRoleV1, PersistentDispatchSlotId) {
        (&self.role, self.slot)
    }

    pub const fn slot(&self) -> PersistentDispatchSlotId {
        self.slot
    }

    pub const fn selection(&self) -> InheritanceSourceSlotSelectionV1 {
        self.selection
    }

    pub fn callable_target(&self) -> Option<scoop_identity::CallableTemplateOrigin> {
        self.selection.declaration().origin()
    }

    pub fn dependency_target(&self) -> crate::ExternalHirTargetV1 {
        use crate::{
            ExternalHirTargetV1 as Target, InheritanceCallableDeclarationV1 as Declaration,
        };
        match self.selection.declaration() {
            Declaration::DerivedEquality(owner) => Target::Nominal(match owner {
                crate::SourceNominalId::Concrete(id) => {
                    scoop_identity::NominalDeclarationOwner::Concrete(id)
                }
                crate::SourceNominalId::GenericTemplate(id) => {
                    scoop_identity::NominalDeclarationOwner::GenericTemplate(id)
                }
            }),
            declaration => {
                Target::Callable(declaration.origin().expect("ordinary dispatch target"))
            }
        }
    }
}

impl WireEncode for NominalDispatchSelectionV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.map(4)?;
        encoder.field(0)?;
        self.role.encode(encoder)?;
        encoder.field(1)?;
        self.slot.encode(encoder)?;
        encoder.field(2)?;
        self.selection.encode(encoder)?;
        encoder.field(3)?;
        self.receiver.encode(encoder)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanonicalNominalDispatchSelectionsV1 {
    records: Vec<NominalDispatchSelectionV1>,
}

impl CanonicalNominalDispatchSelectionsV1 {
    pub const fn empty() -> Self {
        Self {
            records: Vec::new(),
        }
    }

    pub fn try_new(
        mut records: Vec<NominalDispatchSelectionV1>,
    ) -> Result<Self, NominalDispatchSelectionError> {
        records.sort_unstable_by(|a, b| a.key().cmp(&b.key()));
        Self::from_ordered(records)
    }

    fn from_ordered(
        records: Vec<NominalDispatchSelectionV1>,
    ) -> Result<Self, NominalDispatchSelectionError> {
        for (index, pair) in records.windows(2).enumerate() {
            match pair[0].key().cmp(&pair[1].key()) {
                std::cmp::Ordering::Equal => {
                    return Err(NominalDispatchSelectionError::Duplicate(pair[1].slot));
                }
                std::cmp::Ordering::Greater => {
                    return Err(NominalDispatchSelectionError::NonCanonicalOrder {
                        index: index + 1,
                    });
                }
                std::cmp::Ordering::Less => {}
            }
        }
        Ok(Self { records })
    }

    pub fn records(&self) -> &[NominalDispatchSelectionV1] {
        &self.records
    }

    pub fn get(
        &self,
        role: &NominalDispatchSelectionRoleV1,
        slot: PersistentDispatchSlotId,
    ) -> Option<&NominalDispatchSelectionV1> {
        self.records
            .binary_search_by(|record| record.key().cmp(&(role, slot)))
            .ok()
            .map(|index| &self.records[index])
    }
}

impl WireEncode for CanonicalNominalDispatchSelectionsV1 {
    fn encode(&self, encoder: &mut Encoder) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.array(self.records.len() as u64)?;
        for record in &self.records {
            record.encode(encoder)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub enum NominalDispatchSelectionError {
    Duplicate(PersistentDispatchSlotId),
    NonCanonicalOrder { index: usize },
    Resource(scoop_wire::WireError),
}

impl From<scoop_wire::WireError> for NominalDispatchSelectionError {
    fn from(error: scoop_wire::WireError) -> Self {
        Self::Resource(error)
    }
}

impl std::fmt::Display for NominalDispatchSelectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Duplicate(slot) => write!(f, "duplicate nominal dispatch selection for {slot:?}"),
            Self::NonCanonicalOrder { index } => write!(
                f,
                "non-canonical nominal dispatch selection at index {index}"
            ),
            Self::Resource(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for NominalDispatchSelectionError {}
