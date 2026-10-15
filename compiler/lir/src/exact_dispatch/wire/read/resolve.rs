use super::*;
use crate::{
    CanonicalExactLayoutExportsV1, ConeLirFoundation, ExternalStrongShapeSubjectV1, LinkDataError,
    LirTargetProfile, StrongShapeDefinitionRefV1, link_data::link_error,
};
use scoop_identity::{
    PersistentIdResolver, RepresentationRole, StrongDefinitionEntity, StrongDefinitionRole,
    ValidatedIdentityGraph,
};

mod entries;

impl DecodedExactDispatchExportV1 {
    pub fn read(
        self,
        target: LirTargetProfile,
        foundation: &ConeLirFoundation,
        layouts: &[&CanonicalExactLayoutExportsV1],
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<ExactDispatchExportV1, LinkDataError> {
        let table = identities
            .resolve(self.semantic.table)
            .map_err(link_error)?;
        let owner_exact = identities
            .resolve(self.semantic.owner_exact)
            .map_err(link_error)?;
        let role = match self.semantic.role {
            DecodedExactDispatchRoleV1::Vtable => ExactDispatchRoleV1::Vtable,
            DecodedExactDispatchRoleV1::Itable(interface) => ExactDispatchRoleV1::Itable {
                interface_exact: identities.resolve(interface).map_err(link_error)?,
            },
        };
        let saved = foundation.dispatch_tables();
        let index = saved
            .binary_search_by_key(&table, |record| record.id())
            .map_err(|_| link_error("missing dispatch table identity"))?;
        let key = crate::exact_dispatch::replay::checked_role(saved[index].key(), table)
            .map_err(link_error)?;
        if key != (owner_exact, role) {
            return Err(link_error(
                "dispatch table owner or role does not match its identity",
            ));
        }
        let definition = self.definition.read(identities)?;
        let plan = foundation
            .definition_for(
                StrongDefinitionEntity::dispatch_table(table),
                StrongDefinitionRole::DispatchTable,
            )
            .ok_or_else(|| link_error("missing dispatch table definition"))?;
        if definition.semantic_id() != table
            || definition.definition_plan() != plan.id()
            || definition.symbol().key()
                != scoop_identity::PersistentSymbolKey::DispatchTable(table)
        {
            return Err(link_error(
                "dispatch definition does not refer to its table",
            ));
        }
        let physical = StrongShapeDefinitionRefV1::from_saved(
            ExternalStrongShapeSubjectV1::DispatchTable(table),
            definition.definition_plan(),
            definition.symbol(),
            foundation,
        )?;
        let mut entries = Vec::with_capacity(self.semantic.entries.len());
        for (position, entry) in self.semantic.entries.into_iter().enumerate() {
            if usize::try_from(entry.position).ok() != Some(position) {
                return Err(link_error("noncanonical dispatch entry position"));
            }
            entries.push(entry.read(layouts, identities)?);
        }
        Ok(ExactDispatchExportV1::from_parts(
            ExactDispatchBodyPartsV1 {
                table,
                owner_exact,
                target,
                role,
                entries,
                physical,
                definition,
            },
        ))
    }
}

impl DecodedCanonicalExactDispatchExportsV1 {
    pub fn read(
        self,
        target: LirTargetProfile,
        foundation: &ConeLirFoundation,
        layouts: &[&CanonicalExactLayoutExportsV1],
        identities: &mut ValidatedIdentityGraph,
    ) -> Result<CanonicalExactDispatchExportsV1, LinkDataError> {
        let records = self
            .records
            .into_iter()
            .map(|record| record.read(target, foundation, layouts, identities))
            .collect::<Result<Vec<_>, _>>()?;
        if records
            .windows(2)
            .any(|pair| pair[0].table() >= pair[1].table())
        {
            return Err(link_error("noncanonical dispatch table order"));
        }
        Ok(CanonicalExactDispatchExportsV1::from_saved(
            target,
            foundation.producer(),
            records,
        ))
    }
}
