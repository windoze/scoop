//! Declaration-side roots, independent of transported nominal candidates.

use super::*;
use std::collections::{BTreeMap, BTreeSet};

mod callables;
mod index;
mod properties;
mod storage;
pub(super) use index::Index;

impl CanonicalSourceNominalIdsV1 {
    /// Discovers public/inheritance roots, their storage dependencies, and
    /// protected nested support without granting public lookup authority.
    pub fn from_export_hir(
        output: &ExportHirOutput,
        meter: &mut BudgetMeter,
    ) -> Result<Self, Error> {
        Self::from_module(output.module(), meter)
    }

    pub(in crate::production) fn from_module(
        export: &ExportHir,
        meter: &mut BudgetMeter,
    ) -> Result<Self, Error> {
        Self::collect_module(export, false, meter)
    }

    pub(in crate::production::nominal_interfaces) fn from_complete_module(
        export: &ExportHir,
        meter: &mut BudgetMeter,
    ) -> Result<Self, Error> {
        Self::collect_module(export, true, meter)
    }

    fn collect_module(
        export: &ExportHir,
        complete_children: bool,
        meter: &mut BudgetMeter,
    ) -> Result<Self, Error> {
        Self::collect_roots(
            export,
            complete_children,
            index::public(export).map(|local| index::source(export, local)),
            meter,
        )
    }

    pub(in crate::production::nominal_interfaces) fn from_complete_roots(
        export: &ExportHir,
        roots: &[SourceNominalId],
        meter: &mut BudgetMeter,
    ) -> Result<Self, Error> {
        Self::collect_roots(export, true, roots.iter().copied().map(Ok), meter)
    }

    fn collect_roots(
        export: &ExportHir,
        complete_children: bool,
        seeds: impl Iterator<Item = Result<SourceNominalId, Error>>,
        meter: &mut BudgetMeter,
    ) -> Result<Self, Error> {
        meter
            .check_semantic_depth(1, &WirePath::root())
            .map_err(resource)?;
        let index = Index::new(export, meter)?;
        let mut roots = Roots {
            complete_children,
            required: BTreeMap::new(),
            pending: Vec::new(),
            field_types: BTreeSet::new(),
            meter,
        };
        for owner in seeds {
            work(roots.meter, index.nodes.len())?;
            let owner = owner?;
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
                if roots.complete_children {
                    roots.require_field_type(export, &index, ty, 1)?;
                }
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
            if roots.complete_children {
                callables::visit_types(export, node.local, |ty| {
                    roots.require_field_type(export, &index, ty, 1)
                })?;
                properties::visit_types(export, node.local, |ty| {
                    roots.require_field_type(export, &index, ty, 1)
                })?;
            }
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
    complete_children: bool,
    // A complete root recursively owns all lexical children, including private
    // support. A normal inheritance root only introduces protected children.
    required: BTreeMap<SourceNominalId, bool>,
    pending: Vec<(SourceNominalId, bool)>,
    field_types: BTreeSet<TypeId>,
    meter: &'m mut BudgetMeter,
}
impl Roots<'_> {
    fn require(&mut self, owner: SourceNominalId, complete: bool) -> Result<(), Error> {
        let complete = complete || self.complete_children;
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
