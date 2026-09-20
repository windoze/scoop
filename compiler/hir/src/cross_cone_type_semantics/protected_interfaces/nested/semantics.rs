use super::*;
use crate::{
    CanonicalNominalRepresentationSupportV1, CheckedNominalInheritanceGraphV1,
    NominalSourceShapeSemanticAuthority, NominalSupportPropertySemanticAuthority, SourceNominalId,
};
use scoop_wire::{BudgetMeter, WirePath};

mod concrete;
mod errors;
mod metadata;
mod records;
pub use errors::*;

/// Complete declaration-side inventory for the requested nominal, resolved
/// from source/foundation metadata independently of this transported section.
/// The inventory includes private members; presence grants no lookup access.
pub trait NestedNominalSemanticAuthority<E>:
    NominalSupportPropertySemanticAuthority<E> + NominalSourceShapeSemanticAuthority<E>
{
    fn nominal_source_binders(&self, owner: SourceNominalId) -> Result<&CanonicalBinderListV1, E>;
    fn nominal_source_supertypes(
        &self,
        owner: SourceNominalId,
    ) -> Result<&CanonicalSignatureTypesV1, E>;
    fn nominal_source_constructors(
        &self,
        owner: SourceNominalId,
    ) -> Result<&CanonicalPersistentIdsV1<PersistentConstructorId>, E>;
    fn nominal_source_members(
        &self,
        owner: SourceNominalId,
    ) -> Result<&CanonicalNestedMemberRefsV1, E>;
    fn nominal_source_children(
        &self,
        owner: SourceNominalId,
    ) -> Result<&CanonicalNestedNominalRefsV1, E>;
    fn nominal_source_shape(&self, owner: SourceNominalId) -> Result<&NominalSourceShapeV1, E>;
}

/// Source identity, complete inventory, accessor relationships and param-free
/// table joins have been checked. Defaults, slot selection and materialization
/// still require the complete section validator.
#[derive(Clone, Copy, Debug)]
pub struct CheckedNestedNominalSourceV1<'a> {
    record: &'a NominalSupportNestedInterfaceV1,
}
impl CheckedNestedNominalSourceV1<'_> {
    pub const fn record(&self) -> &NominalSupportNestedInterfaceV1 {
        self.record
    }
}
impl ProtectedNestedNominalInterfaceV1 {
    pub fn validate_source<'a, A: NestedNominalSemanticAuthority<E>, E>(
        &'a self,
        graph: &CheckedNominalInheritanceGraphV1<'_>,
        representations: &CanonicalNominalRepresentationSupportV1,
        authority: &mut A,
        meter: &mut BudgetMeter,
    ) -> Result<CheckedNestedNominalSourceV1<'a>, NestedSourceSemanticError<E>> {
        let owner = self
            .declaration_access()
            .lexical_owners()
            .last()
            .copied()
            .ok_or(NestedSourceSemanticError::Owner)?;
        if graph.source(owner).is_none_or(|source| {
            source.key.declaration_kind() != scoop_identity::SourceDeclarationKind::Class
        }) {
            return Err(NestedSourceSemanticError::Owner);
        }
        self.source
            .validate_source(graph, representations, authority, meter)
    }
}
impl NominalSupportNestedInterfaceV1 {
    pub(in crate::cross_cone_type_semantics) fn validate_concrete_support<E>(
        &self,
        graph: &CheckedNominalInheritanceGraphV1<'_>,
        representations: &CanonicalNominalRepresentationSupportV1,
        meter: &mut BudgetMeter,
    ) -> Result<(), NestedSourceSemanticError<E>> {
        concrete::validate(self, graph, representations, meter)
    }
    pub fn validate_source<'a, A: NestedNominalSemanticAuthority<E>, E>(
        &'a self,
        graph: &CheckedNominalInheritanceGraphV1<'_>,
        representations: &CanonicalNominalRepresentationSupportV1,
        authority: &mut A,
        meter: &mut BudgetMeter,
    ) -> Result<CheckedNestedNominalSourceV1<'a>, NestedSourceSemanticError<E>> {
        validate(self, graph, representations, authority, meter, 1)?;
        Ok(CheckedNestedNominalSourceV1 { record: self })
    }
}
fn validate<A: NestedNominalSemanticAuthority<E>, E>(
    record: &NominalSupportNestedInterfaceV1,
    graph: &CheckedNominalInheritanceGraphV1<'_>,
    representations: &CanonicalNominalRepresentationSupportV1,
    authority: &mut A,
    meter: &mut BudgetMeter,
    depth: u64,
) -> Result<(), NestedSourceSemanticError<E>> {
    use NestedSourceSemanticError as Error;
    meter
        .check_semantic_depth(depth, &WirePath::root())
        .map_err(Error::Resource)?;
    meter
        .charge_nodes(1, &WirePath::root())
        .map_err(Error::Resource)?;
    let source = graph.source(record.declaration()).ok_or(Error::Owner)?;
    let interface = record.payload().source_interface();
    if source.access != record.declaration_access()
        || source.key.declaration_kind() != interface.kind().source_kind()
        || source.key.duplicate_signature().type_parameter_count()
            != interface.type_parameters().len_u32()
    {
        return Err(Error::Identity);
    }
    if authority
        .source_nominal_modality(record.declaration())
        .map_err(Error::Foundation)?
        != interface.modality()
    {
        return Err(Error::Modality);
    }
    metadata::validate(record.declaration(), interface, authority, meter)?;
    concrete::validate(record, graph, representations, meter)?;
    records::validate(record, graph, representations, authority, meter, depth)?;
    Ok(())
}

fn compare<T: WireEncode + PartialEq, E>(
    actual: &T,
    expected: &T,
    meter: &mut BudgetMeter,
) -> Result<(), NestedSourceSemanticError<E>> {
    use NestedSourceSemanticError as Error;
    let count = scoop_wire::encoded_length(actual)
        .map_err(Error::Encoding)?
        .saturating_add(scoop_wire::encoded_length(expected).map_err(Error::Encoding)?);
    meter
        .charge_work(count, &WirePath::root())
        .map_err(Error::Resource)?;
    if actual != expected {
        return Err(Error::Inventory);
    }
    Ok(())
}
