//! Joins local representation inventory to independent checked source facts.
use scoop_identity::{ConeIdentity, PersistentTypeId, SourceDeclarationKey};
use scoop_wire::{BudgetMeter, WirePath};

use super::{
    CanonicalNominalRepresentationSupportV1, NominalRepresentationShapeV1,
    NominalRepresentationSupportV1,
};
use crate::{CanonicalPersistentIdsV1, DeclarationAccessSourceV1, NominalSourceShapeV1};

mod compare;
mod errors;
mod shared;
mod source;
#[cfg(test)]
mod tests;
mod types;
pub use errors::*;
pub(super) use source::charge_key;

impl NominalRepresentationSupportV1 {
    pub(in crate::cross_cone_type_semantics) fn charge_source_key_resources(
        key: &SourceDeclarationKey,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), scoop_wire::WireError> {
        source::charge_key(key, meter, path)
    }
}

impl NominalRepresentationSupportV1 {
    pub(in crate::cross_cone_type_semantics) fn signature_types_match_metered(
        left: &scoop_identity::SignatureTypeKey,
        right: &scoop_identity::SignatureTypeKey,
        depth: u64,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<bool, scoop_wire::WireError> {
        types::equal(left, right, depth, meter, path)
    }
}

impl NominalRepresentationSupportV1 {
    pub(in crate::cross_cone_type_semantics) fn public_source_shape_matches(
        &self,
        source: &NominalSourceShapeV1,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<bool, scoop_wire::WireError> {
        compare::public_value(self.shape(), source, meter, path)
    }
}

/// Effective public value lookup is projected from the real source surface,
/// including its lexical owners. It is not inferred from candidate visibility.
#[derive(Clone, Copy, Debug)]
pub enum NominalRepresentationPublicSourceShapeV1<'a> {
    PublicSourceShape(&'a NominalSourceShapeV1),
    NoPublicSourceShape,
}

#[derive(Clone, Copy, Debug)]
pub struct NominalRepresentationSourceV1<'a> {
    pub key: &'a SourceDeclarationKey,
    pub access: &'a DeclarationAccessSourceV1,
    pub shape: &'a NominalRepresentationShapeV1,
    pub public_source_shape: NominalRepresentationPublicSourceShapeV1<'a>,
}

pub trait NominalRepresentationSemanticAuthority<E> {
    fn current_provider(&self) -> ConeIdentity;
    /// The required local owners come from checked source/support inventory,
    /// never from the representation table being validated.
    fn required_representation_owners(
        &self,
    ) -> Result<&CanonicalPersistentIdsV1<PersistentTypeId>, E>;
    /// Every borrowed field is independently projected from the real source.
    /// Native witnesses and candidate representation records are not authority.
    fn representation_source(
        &self,
        owner: PersistentTypeId,
    ) -> Result<NominalRepresentationSourceV1<'_>, E>;
}

/// Source representation agreement only; facts, inheritance, access selection,
/// and the complete type section remain independent checks.
#[derive(Clone, Copy, Debug)]
pub struct CheckedNominalRepresentationSupportV1<'a> {
    table: &'a CanonicalNominalRepresentationSupportV1,
}
impl<'a> CheckedNominalRepresentationSupportV1<'a> {
    pub const fn table(self) -> &'a CanonicalNominalRepresentationSupportV1 {
        self.table
    }
    pub fn get(self, owner: PersistentTypeId) -> Option<&'a NominalRepresentationSupportV1> {
        self.table.get(owner)
    }
}

impl CanonicalNominalRepresentationSupportV1 {
    pub fn validate_source_semantics<A: NominalRepresentationSemanticAuthority<E>, E>(
        &self,
        authority: &A,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<
        CheckedNominalRepresentationSupportV1<'_>,
        NominalRepresentationSourceSemanticError<E>,
    > {
        use NominalRepresentationSourceSemanticError as Error;
        let required = authority
            .required_representation_owners()
            .map_err(Error::Inventory)?
            .values();
        source::sequence(required.len(), meter, path)?;
        source::sequence(self.records().len(), meter, path)?;
        let mut records = self.records().iter().enumerate();
        let mut next = records.next();
        for owner in required {
            meter.charge_work(32, path)?;
            let Some((index, record)) = next else {
                return Err(Error::Missing { owner: *owner });
            };
            match record.owner().cmp(owner) {
                std::cmp::Ordering::Less => {
                    return Err(Error::Extra {
                        index,
                        owner: record.owner(),
                    });
                }
                std::cmp::Ordering::Greater => return Err(Error::Missing { owner: *owner }),
                std::cmp::Ordering::Equal => {}
            }
            let at = path.clone().index(index as u64);
            meter.check_semantic_depth(1, &at)?;
            meter.charge_nodes(1, &at)?;
            meter.charge_work(1, &at)?;
            let expected =
                authority
                    .representation_source(*owner)
                    .map_err(|error| Error::Source {
                        index,
                        owner: *owner,
                        error,
                    })?;
            source::validate(record, expected, authority.current_provider(), meter, &at).map_err(
                |error| match error {
                    source::Failure::Resource(error) => Error::Resource(error),
                    source::Failure::Mismatch(error) => Error::Record {
                        index,
                        owner: *owner,
                        error,
                    },
                },
            )?;
            next = records.next();
        }
        if let Some((index, record)) = next {
            return Err(Error::Extra {
                index,
                owner: record.owner(),
            });
        }
        Ok(CheckedNominalRepresentationSupportV1 { table: self })
    }
}
