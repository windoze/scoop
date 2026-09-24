//! Complete protocol ownership derived from checked source surfaces.
use super::*;
use crate::{
    CheckedNominalInheritanceGraphV1, CheckedNominalInheritanceInterfacesV1,
    CheckedProtectedDeclarationSourcesV1, NominalSupportCallableSemanticAuthority,
};
use scoop_wire::{BudgetMeter, WirePath};

mod collect;
mod errors;
mod shared;
mod source;
pub use errors::*;

/// All source-use owners, parameter protocols and default keys have been joined.
/// Complete default bodies and selection authority remain separate obligations.
#[derive(Debug)]
pub struct CheckedProtectedSourceInterfacesV1<'a> {
    table: &'a CanonicalProtectedCallableSourceInterfacesV1,
    entries: Vec<CheckedProtectedSourceInterfaceV1<'a>>,
}
#[derive(Clone, Copy, Debug)]
pub struct CheckedProtectedSourceInterfaceV1<'a> {
    source: source::Source<'a>,
    protocol: CheckedProtectedSourceProtocolV1<'a>,
}
impl<'a> CheckedProtectedSourceInterfacesV1<'a> {
    pub const fn table(&self) -> &'a CanonicalProtectedCallableSourceInterfacesV1 {
        self.table
    }
    pub fn entries(&self) -> &[CheckedProtectedSourceInterfaceV1<'a>] {
        &self.entries
    }
    pub fn get(
        &self,
        owner: CallableTemplateOrigin,
    ) -> Option<CheckedProtectedSourceInterfaceV1<'a>> {
        self.entries
            .binary_search_by_key(&owner, CheckedProtectedSourceInterfaceV1::owner)
            .ok()
            .map(|index| self.entries[index])
    }
}
impl<'a> CheckedProtectedSourceInterfaceV1<'a> {
    pub fn owner(&self) -> CallableTemplateOrigin {
        self.source.owner()
    }
    pub const fn protocol(&self) -> CheckedProtectedSourceProtocolV1<'a> {
        self.protocol
    }
    pub fn payload(&self) -> &'a crate::NominalSourceCallablePayloadV1 {
        self.source.payload()
    }
    pub const fn declaration_access(&self) -> &'a crate::DeclarationAccessSourceV1 {
        self.source.access()
    }

    /// Replays this retained source leaf against the immutable foundation facts
    /// used by the enclosing semantic authority. Presence grants no lookup.
    pub fn validate_owner_source<'s, A: NominalSupportCallableSemanticAuthority<E>, E>(
        self,
        graph: &CheckedNominalInheritanceGraphV1<'_>,
        authority: &'s mut A,
        meter: &mut BudgetMeter,
    ) -> Result<crate::ProtectedDefaultOwnerSourceV1<'s>, ProtectedSourceClosureError<E>>
    where
        'a: 's,
    {
        self.source.validate(graph, authority, meter)
    }
}
impl CanonicalProtectedCallableSourceInterfacesV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn validate_protocols<'a, S, A, E>(
        &'a self,
        protected: CheckedProtectedDeclarationSourcesV1<'a>,
        inheritance: CheckedNominalInheritanceInterfacesV1<'a>,
        graph: &CheckedNominalInheritanceGraphV1<'_>,
        keys: &ProtectedDefaultKeyIndexV1,
        source_authority: &mut S,
        protocol_authority: &mut A,
        meter: &mut BudgetMeter,
    ) -> Result<CheckedProtectedSourceInterfacesV1<'a>, ProtectedSourceClosureError<E>>
    where
        S: NominalSupportCallableSemanticAuthority<E>,
        A: ProtectedSourceProtocolSemanticAuthority<E>,
    {
        use ProtectedSourceClosureError as Error;
        let sources = collect::sources(protected.table(), inheritance.table(), meter)?;
        meter
            .charge_work(
                (sources.len() as u64 + self.records().len() as u64).saturating_mul(64),
                &WirePath::root(),
            )
            .map_err(Error::Resource)?;
        if !sources.iter().map(source::Source::owner).eq(self
            .records()
            .iter()
            .map(ProtectedCallableSourceInterfaceV1::owner))
        {
            return Err(Error::OwnerInventory);
        }
        self.validate_default_closure(keys, meter)
            .map_err(Error::DefaultClosure)?;
        let mut entries = Vec::new();
        meter
            .check_table_entries(sources.len() as u64, &WirePath::root())
            .map_err(Error::Resource)?;
        meter
            .try_reserve_collection_slots(&mut entries, sources.len(), &WirePath::root())
            .map_err(Error::Resource)?;
        for (source, record) in sources.into_iter().zip(self.records()) {
            let owner = source.validate(graph, source_authority, meter)?;
            let protocol = match owner {
                crate::ProtectedDefaultOwnerSourceV1::Protected(owner) => {
                    record.validate_protected(owner, protocol_authority, meter)
                }
                crate::ProtectedDefaultOwnerSourceV1::NominalSupport(owner) => {
                    record.validate_nominal_support(owner, protocol_authority, meter)
                }
            }
            .map_err(|error| Error::Protocol {
                owner: record.owner(),
                error,
            })?;
            entries.push(CheckedProtectedSourceInterfaceV1 { source, protocol });
        }
        Ok(CheckedProtectedSourceInterfacesV1 {
            table: self,
            entries,
        })
    }
}

#[cfg(test)]
mod tests;
