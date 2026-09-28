use std::fmt;

use scoop_identity::{PersistentExactTypeId, SourceDeclarationKey};
use scoop_wire::WireError;

use super::*;
use crate::{
    CheckedNominalInheritanceGraphV1, InheritanceGraphError, InheritanceQueryError,
    InheritanceSlotSchemaSemanticAuthority, InheritanceSlotSchemaSemanticError,
    NominalInheritanceSemanticAuthority,
};

mod declarations;
mod relations;
mod types;

/// Foundation and the already validated core Unit identity from one explicit
/// dependency closure. This does not replace the later source-interface join
/// for result, effects, modality, defaults, and declaration completeness.
pub trait InheritanceSlotContractSemanticAuthority<E>:
    InheritanceSlotSchemaSemanticAuthority<E> + NominalInheritanceSemanticAuthority<E>
{
    fn unit_exact_type(&self) -> Result<PersistentExactTypeId, E>;
}

/// Checks this record's foundation identity and source parameter shape,
/// receiver, schema membership, and target compatibility. Whole-table override
/// selection and source-interface completeness remain enclosing-section checks.
#[derive(Clone, Copy, Debug)]
pub struct CheckedInheritanceSlotContractV1<'a> {
    owner: PersistentExactTypeId,
    record: &'a InheritanceSlotContractV1,
}
impl CheckedInheritanceSlotContractV1<'_> {
    pub const fn owner(&self) -> PersistentExactTypeId {
        self.owner
    }
    pub const fn record(&self) -> &InheritanceSlotContractV1 {
        self.record
    }
}

impl CheckedNominalInheritanceGraphV1<'_> {
    pub fn validate_slot_contract<'s, A: InheritanceSlotContractSemanticAuthority<E>, E>(
        &self,
        owner: PersistentExactTypeId,
        record: &'s InheritanceSlotContractV1,
        authority: &A,
    ) -> Result<CheckedInheritanceSlotContractV1<'s>, InheritanceSlotContractSemanticError<E>> {
        relations::validate(self, owner, record, authority)?;
        Ok(CheckedInheritanceSlotContractV1 { owner, record })
    }
}

#[derive(Debug)]
pub enum InheritanceSlotContractSemanticError<E> {
    Resource(WireError),
    Foundation(E),
    Encoding(scoop_wire::cbor::EncodeError),
    Source(InheritanceGraphError<E>),
    Inheritance(InheritanceQueryError),
    Schema(InheritanceSlotSchemaSemanticError<E>),
    DeclarationIdentity(InheritanceCallableDeclarationV1),
    SlotIdentity,
    ReceiverOwner,
    Signature,
    PrivateDeclaration,
    TargetOwner,
    TargetName,
    AbstractObligation,
}
impl<E: fmt::Display> fmt::Display for InheritanceSlotContractSemanticError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::Foundation(error) => error.fmt(f),
            Self::Encoding(error) => error.fmt(f),
            Self::Source(error) => error.fmt(f),
            Self::Inheritance(error) => error.fmt(f),
            Self::Schema(error) => error.fmt(f),
            Self::DeclarationIdentity(id) => write!(
                f,
                "inheritance declaration identity or source owner disagrees for {id:?}"
            ),
            Self::SlotIdentity => f.write_str(
                "root slot key disagrees with its typed declaration or source owner kind",
            ),
            Self::ReceiverOwner => {
                f.write_str("inheritance signature receiver differs from its source owner")
            }
            Self::Signature => f.write_str(
                "inheritance signature disagrees with its source parameter or accessor shape",
            ),
            Self::PrivateDeclaration => {
                f.write_str("private declarations cannot participate in inheritance dispatch")
            }
            Self::TargetOwner => f.write_str(
                "inheritance slot target owner or modality is outside the owner's inheritance path",
            ),
            Self::TargetName => {
                f.write_str("inheritance slot target has a different source declaration name")
            }
            Self::AbstractObligation => {
                f.write_str("inheritance owner cannot expose or implement this abstract obligation")
            }
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for InheritanceSlotContractSemanticError<E> {}

struct Declaration<'s> {
    key: &'s SourceDeclarationKey,
    exact_owner: PersistentExactTypeId,
}
