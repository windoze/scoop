//! Declaration-side roots, independent of transported nominal candidates.

use super::*;
use std::collections::{BTreeMap, BTreeSet};

mod callables;
mod index;
mod properties;
mod shared;
pub(in crate::production) use shared::SharedSourceRoots;
mod storage;
pub(super) use index::Index;

impl CanonicalSourceNominalIdsV1 {
    /// Discovers public/inheritance roots, their storage dependencies, and
    /// protected nested support without granting public lookup authority.
    pub fn from_export_hir(output: &ExportHirOutput) -> Result<Self, Error> {
        Self::from_module(output.module())
    }

    pub(in crate::production) fn from_module(export: &ExportHir) -> Result<Self, Error> {
        Self::collect_module(export, false)
    }

    fn collect_module(export: &ExportHir, complete_children: bool) -> Result<Self, Error> {
        Self::collect_roots(
            export,
            complete_children,
            index::public(export).map(|local| index::source(export, local)),
        )
    }

    pub(in crate::production::nominal_interfaces) fn from_complete_roots(
        export: &ExportHir,
        roots: &[SourceNominalId],
    ) -> Result<Self, Error> {
        Self::collect_roots(export, true, roots.iter().copied().map(Ok))
    }

    fn collect_roots(
        export: &ExportHir,
        complete_children: bool,
        seeds: impl Iterator<Item = Result<SourceNominalId, Error>>,
    ) -> Result<Self, Error> {
        let index = Index::new(export)?;
        let mut roots = Roots {
            complete_children,
            required: BTreeMap::new(),
            pending: Vec::new(),
            field_types: BTreeSet::new(),
        };
        for owner in seeds {
            let owner = owner?;
            if !index.nodes.contains_key(&owner) {
                return Err(invalid("public source root is not owned by this Cone"));
            }
            roots.require(owner, false)?;
        }
        loop {
            if roots.expand_next(export, &index)?.is_none() {
                break;
            }
        }
        let mut values = Vec::new();
        scoop_wire::allocation::try_reserve(&mut values, roots.required.len(), &WirePath::root())
            .map_err(resource)?;
        values.extend(roots.required.into_keys());
        Self::try_new(values).map_err(Error::SourceInventory)
    }
}

struct Roots {
    complete_children: bool,
    // A complete root recursively owns all lexical children, including private
    // support. A normal inheritance root only introduces protected children.
    required: BTreeMap<SourceNominalId, bool>,
    pending: Vec<(SourceNominalId, bool)>,
    field_types: BTreeSet<TypeId>,
}
impl Roots {
    fn expand_next(
        &mut self,
        export: &ExportHir,
        index: &Index,
    ) -> Result<Option<SourceNominalId>, Error> {
        let Some((owner, complete)) = self.pending.pop() else {
            return Ok(None);
        };

        let node = index
            .nodes
            .get(&owner)
            .ok_or_else(|| invalid("required nominal source root is absent"))?;
        if let Some(parent) = node.parent {
            self.require(parent, false)?;
        }
        index::visit_bases(export, node.local, |ty| {
            if self.complete_children {
                self.require_field_type(export, index, ty)?;
            }

            let base = owner_resolution::from_type(export, ty)
                .ok_or_else(|| invalid("source inheritance has no nominal identity"))?;
            // Foreign source owners are supplied by their provider closure.
            if index.nodes.contains_key(&base) {
                self.require(base, false)?;
            }
            Ok(())
        })?;
        storage::visit_fields(export, node.local, |ty| {
            self.require_field_type(export, index, ty)
        })?;
        if self.complete_children {
            callables::visit_types(export, node.local, |ty| {
                self.require_field_type(export, index, ty)
            })?;
            properties::visit_types(export, node.local, |ty| {
                self.require_field_type(export, index, ty)
            })?;
        }

        if let Some(children) = index.children.get(&owner) {
            for child in children {
                if complete || index.nodes[child].visibility == DeclaredVisibility::Protected {
                    self.require(*child, true)?;
                }
            }
        }
        Ok(Some(owner))
    }

    fn require(&mut self, owner: SourceNominalId, complete: bool) -> Result<(), Error> {
        let complete = complete || self.complete_children;

        let previous = self.required.get(&owner).copied();
        if previous.is_some_and(|already_complete| already_complete || !complete) {
            return Ok(());
        }

        push(&mut self.pending, (owner, complete))?;
        self.required.insert(owner, complete);
        Ok(())
    }
}
