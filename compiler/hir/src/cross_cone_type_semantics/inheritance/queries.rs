use std::fmt;

use scoop_identity::{PersistentExactTypeId, SourceDeclarationKind};
use scoop_wire::WireError;

use super::{CheckedNominalInheritanceGraphV1, DirectClassBaseV1};
use crate::SourceNominalId;

impl CheckedNominalInheritanceGraphV1<'_> {
    /// Includes the declaring class itself. Object types may be receivers but
    /// never class-base targets or protected declaration owners.
    pub fn is_subclass(
        &self,
        derived: PersistentExactTypeId,
        base: PersistentExactTypeId,
    ) -> Result<bool, InheritanceQueryError> {
        self.require_class(base)?;
        let mut current = derived;
        loop {
            let node = self
                .nodes
                .get(&current)
                .ok_or(InheritanceQueryError::UnknownExact(current))?;
            if !matches!(
                self.sources[&node.source].key.declaration_kind(),
                SourceDeclarationKind::Class | SourceDeclarationKind::Object
            ) {
                return Ok(false);
            }
            if current == base {
                return Ok(true);
            }
            match node.edges.direct_base() {
                DirectClassBaseV1::NoClassBase => return Ok(false),
                DirectClassBaseV1::ClassBase { exact } => current = exact,
            }
        }
    }

    pub fn require_class(&self, exact: PersistentExactTypeId) -> Result<(), InheritanceQueryError> {
        let node = self
            .nodes
            .get(&exact)
            .ok_or(InheritanceQueryError::UnknownExact(exact))?;
        if self.sources[&node.source].key.declaration_kind() == SourceDeclarationKind::Class {
            Ok(())
        } else {
            Err(InheritanceQueryError::NotClass(exact))
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum InheritanceQueryError {
    Resource(WireError),
    UnknownExact(PersistentExactTypeId),
    NotClass(PersistentExactTypeId),
    UnknownSource(SourceNominalId),
}
impl fmt::Display for InheritanceQueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(f),
            Self::UnknownExact(exact) => write!(
                f,
                "exact type {exact} is outside the checked inheritance closure"
            ),
            Self::NotClass(exact) => write!(f, "inheritance type {exact} is not a source class"),
            Self::UnknownSource(owner) => write!(
                f,
                "source nominal {owner:?} is outside the checked lexical closure"
            ),
        }
    }
}
impl std::error::Error for InheritanceQueryError {}
