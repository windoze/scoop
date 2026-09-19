use scoop_identity::{ConeIdentity, SourceDeclarationKey};
use scoop_wire::{BudgetMeter, WirePath};

use super::{
    CheckedInheritanceSourceV1, CheckedNominalInheritanceGraphV1, InheritanceGraphError,
    NominalInheritanceSemanticAuthority,
};
use crate::{
    DeclarationAccessSourceSemanticAuthority, ExportDefinitionSourceSemanticAuthority,
    ExportDefinitionSourceV1, SourceNominalId,
};

impl<'a> CheckedNominalInheritanceGraphV1<'a> {
    pub(super) fn validate_source<A, E>(
        &mut self,
        owner: SourceNominalId,
        authority: &'a A,
        meter: &mut BudgetMeter,
        depth: u64,
    ) -> Result<(), InheritanceGraphError<E>>
    where
        A: NominalInheritanceSemanticAuthority<E>,
    {
        let path = WirePath::root();
        meter
            .check_semantic_depth(depth, &path)
            .map_err(InheritanceGraphError::Resource)?;
        meter
            .charge_work(1, &path)
            .map_err(InheritanceGraphError::Resource)?;
        if self.sources.contains_key(&owner) {
            return Ok(());
        }
        let key = authority
            .nominal_declaration_key(owner)
            .map_err(InheritanceGraphError::Foundation)?;
        let owner_count = key.owners().owners().len() as u64;
        meter
            .check_semantic_depth(owner_count.saturating_add(1), &path)
            .map_err(InheritanceGraphError::Resource)?;
        meter
            .charge_work(owner_count.saturating_add(1).saturating_pow(2), &path)
            .map_err(InheritanceGraphError::Resource)?;
        if SourceNominalId::from_source_declaration(key).ok() != Some(owner) {
            return Err(InheritanceGraphError::SourceIdentity(owner));
        }
        let access = authority
            .nominal_access_source(owner)
            .map_err(InheritanceGraphError::Foundation)?;
        if authority
            .nominal_definition_source(owner)
            .map_err(InheritanceGraphError::Foundation)?
            != access.definition_origin()
        {
            return Err(InheritanceGraphError::SourceOrigin(owner));
        }
        let mut scoped = SourceAuthority {
            authority,
            cone: key.origin(),
        };
        meter
            .charge_edges(access.lexical_owners().len() as u64, &path)
            .map_err(InheritanceGraphError::Resource)?;
        access
            .validate_for_declaration(key, &mut scoped)
            .map_err(|error| InheritanceGraphError::Source { owner, error })?;
        for ancestor in access.lexical_owners() {
            self.validate_source(*ancestor, authority, meter, depth + 1)?;
        }
        meter
            .charge_nodes(1, &path)
            .map_err(InheritanceGraphError::Resource)?;
        meter
            .charge_collection_slots(1, &path)
            .map_err(InheritanceGraphError::Resource)?;
        self.sources
            .insert(owner, CheckedInheritanceSourceV1 { key, access });
        Ok(())
    }
}

struct SourceAuthority<'a, A> {
    authority: &'a A,
    cone: ConeIdentity,
}
impl<A: NominalInheritanceSemanticAuthority<E>, E> ExportDefinitionSourceSemanticAuthority<E>
    for SourceAuthority<'_, A>
{
    fn current_cone(&self) -> ConeIdentity {
        self.cone
    }
    fn validate_export_definition_source(
        &mut self,
        source: &ExportDefinitionSourceV1,
    ) -> Result<(), E> {
        self.authority.validate_definition_source(source)
    }
}
impl<A: NominalInheritanceSemanticAuthority<E>, E> DeclarationAccessSourceSemanticAuthority<E>
    for SourceAuthority<'_, A>
{
    fn nominal_declaration_key(&self, owner: SourceNominalId) -> Result<&SourceDeclarationKey, E> {
        self.authority.nominal_declaration_key(owner)
    }
    fn nominal_definition_source(
        &self,
        owner: SourceNominalId,
    ) -> Result<&ExportDefinitionSourceV1, E> {
        self.authority.nominal_definition_source(owner)
    }
}
