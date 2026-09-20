//! Complete recursive source support joined to one artifact's bound declarations.
use crate::*;
use scoop_identity::{CallableTemplateOrigin, ConeIdentity};
use scoop_wire::{BudgetMeter, WireEncode, WireError, WirePath};
use std::collections::BTreeMap;

mod contracts;
mod errors;
mod replay;
pub use errors::*;
type Error = NominalNestedBindingError;

/// Artifact source joins and parameter protocols only. Defaults, dispatch
/// selection and machine-use eligibility remain separate obligations.
#[derive(Debug)]
pub struct BoundNestedNominalSourceV1<'c, 'p, 's, 'a, 'f> {
    members: &'p BoundNominalMemberSourcesV1<'s, 'a, 'f>,
    constructors: &'p BoundNominalConstructorSourcesV1<'s, 'a, 'f>,
    parameter_sources: &'p CanonicalNominalSourceParameterProtocolsV1,
    record: &'c NominalSupportNestedInterfaceV1,
    representations: &'c CanonicalNominalRepresentationSupportV1,
    protocols: BTreeMap<CallableTemplateOrigin, CheckedProtectedSourceProtocolV1<'c>>,
}
impl<'c, 'p, 's, 'a, 'f> BoundNestedNominalSourceV1<'c, 'p, 's, 'a, 'f> {
    pub fn provider(&self) -> ConeIdentity {
        self.members.provider()
    }
    pub const fn members(&self) -> &'p BoundNominalMemberSourcesV1<'s, 'a, 'f> {
        self.members
    }
    pub const fn constructors(&self) -> &'p BoundNominalConstructorSourcesV1<'s, 'a, 'f> {
        self.constructors
    }
    pub const fn parameter_sources(&self) -> &'p CanonicalNominalSourceParameterProtocolsV1 {
        self.parameter_sources
    }
    pub const fn record(&self) -> &'c NominalSupportNestedInterfaceV1 {
        self.record
    }
    pub const fn representations(&self) -> &'c CanonicalNominalRepresentationSupportV1 {
        self.representations
    }
    pub fn protocols(&self) -> impl Iterator<Item = CheckedProtectedSourceProtocolV1<'c>> + '_ {
        self.protocols.values().copied()
    }
    pub fn protocol(
        &self,
        owner: CallableTemplateOrigin,
    ) -> Option<CheckedProtectedSourceProtocolV1<'c>> {
        self.protocols.get(&owner).copied()
    }
}
impl<'p, 's, 'a, 'f> BoundNominalParameterProtocolsV1<'p, 's, 'a, 'f> {
    pub fn validate_nested_source<'c>(
        &mut self,
        candidate: &'c NominalSupportNestedInterfaceV1,
        protocols: &'c CanonicalProtectedCallableSourceInterfacesV1,
        representations: &'c CanonicalNominalRepresentationSupportV1,
        meter: &mut BudgetMeter,
    ) -> Result<BoundNestedNominalSourceV1<'c, 'p, 's, 'a, 'f>, Error> {
        let foundation = self.members().nominals.foundation;
        let entries = foundation.source().entries();
        let graph = CheckedNominalInheritanceGraphV1::validate_with_source_roots(
            entries.local_inheritance_edges.records().iter(),
            entries.source_roots.values().iter().copied(),
            foundation,
            meter,
        )
        .map_err(Error::from_graph)?;
        let mut checked = BoundNestedNominalSourceV1 {
            members: self.members(),
            constructors: self.constructors(),
            parameter_sources: self.table(),
            record: candidate,
            representations,
            protocols: BTreeMap::new(),
        };
        replay::validate(self, candidate, protocols, &graph, &mut checked, meter, 1)?;
        Ok(checked)
    }
}
pub(super) fn compare<T: WireEncode + PartialEq>(
    actual: &T,
    expected: &T,
    declaration: NestedSupportDeclarationV1,
    field: &'static str,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let path = WirePath::root();
    let bytes = scoop_wire::encoded_length(actual)
        .map_err(Error::Encoding)?
        .saturating_add(scoop_wire::encoded_length(expected).map_err(Error::Encoding)?);
    meter.charge_nodes(1, &path)?;
    meter.charge_work(bytes, &path)?;
    if actual != expected {
        return Err(Error::Contract { declaration, field });
    }
    Ok(())
}
pub(super) fn query(length: usize, meter: &mut BudgetMeter) -> Result<(), Error> {
    meter.charge_work(u64::from(length.max(1).ilog2()) + 1, &WirePath::root())?;
    Ok(())
}
