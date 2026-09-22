//! Declaration-side roots, independent of transported nominal candidates.

use super::*;
use std::collections::{BTreeMap, BTreeSet};

mod index;
mod storage;
pub(super) use index::Index;

impl CanonicalSourceNominalIdsV1 {
    /// Discovers public/inheritance roots, their storage dependencies, and
    /// protected nested support without granting public lookup authority.
    pub fn from_export_hir(
        output: &ExportHirOutput,
        meter: &mut BudgetMeter,
    ) -> Result<Self, Error> {
        let export = output.module();
        meter
            .check_semantic_depth(1, &WirePath::root())
            .map_err(resource)?;
        let index = Index::new(export, meter)?;
        let mut roots = Roots {
            required: BTreeMap::new(),
            pending: Vec::new(),
            field_types: BTreeSet::new(),
            meter,
        };
        for local in index::public(export) {
            work(roots.meter, index.nodes.len())?;
            let owner = index::source(export, local)?;
            if !index.nodes.contains_key(&owner) {
                return Err(invalid("public source root is not owned by this Cone"));
            }
            roots.require(owner, false)?;
        }
        while let Some((owner, complete)) = roots.pending.pop() {
            work(roots.meter, index.nodes.len())?;
            let node = index
                .nodes
                .get(&owner)
                .ok_or_else(|| invalid("required nominal source root is absent"))?;
            if let Some(parent) = node.parent {
                roots.require(parent, false)?;
            }
            index::visit_bases(export, node.local, |ty| {
                work(roots.meter, index.nodes.len())?;
                if matches!(export.types[ty], Type::Class(_)) {
                    roots
                        .meter
                        .charge_work(export.objects.len() as u64, &WirePath::root())
                        .map_err(resource)?;
                }
                let base = owner_resolution::from_type(export, ty)
                    .ok_or_else(|| invalid("source inheritance has no nominal identity"))?;
                // Foreign source owners are supplied by their provider closure.
                if index.nodes.contains_key(&base) {
                    roots.require(base, false)?;
                }
                Ok(())
            })?;
            storage::visit_fields(export, node.local, |ty| {
                roots.require_field_type(export, &index, ty, 1)
            })?;
            work(roots.meter, index.children.len())?;
            if let Some(children) = index.children.get(&owner) {
                for child in children {
                    work(roots.meter, index.nodes.len())?;
                    if complete || index.nodes[child].visibility == DeclaredVisibility::Protected {
                        roots.require(*child, true)?;
                    }
                }
            }
        }
        let mut values = Vec::new();
        roots
            .meter
            .try_reserve_collection_slots(&mut values, roots.required.len(), &WirePath::root())
            .map_err(resource)?;
        values.extend(roots.required.into_keys());
        Self::try_new(values, roots.meter).map_err(Error::SourceInventory)
    }
}

struct Roots<'m> {
    // A complete root recursively owns all lexical children, including private
    // support. A normal inheritance root only introduces protected children.
    required: BTreeMap<SourceNominalId, bool>,
    pending: Vec<(SourceNominalId, bool)>,
    field_types: BTreeSet<TypeId>,
    meter: &'m mut BudgetMeter,
}
impl Roots<'_> {
    fn require(&mut self, owner: SourceNominalId, complete: bool) -> Result<(), Error> {
        work(self.meter, self.required.len())?;
        let previous = self.required.get(&owner).copied();
        if previous.is_some_and(|already_complete| already_complete || !complete) {
            return Ok(());
        }
        if previous.is_none() {
            self.meter
                .check_table_entries(self.required.len() as u64 + 1, &WirePath::root())
                .map_err(resource)?;
            self.meter
                .charge_collection_slots(1, &WirePath::root())
                .map_err(resource)?;
        }
        push(&mut self.pending, (owner, complete), self.meter)?;
        self.required.insert(owner, complete);
        Ok(())
    }
}
