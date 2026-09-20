//! A borrowed, monotonic budget for portable default projection.

use std::cell::{Cell, RefCell};

use scoop_identity::{LocalValueSelector, StructuralDefinitionPath};
use scoop_wire::{BudgetMeter, WireError, WirePath};

pub(super) struct ProjectionResources<'meter> {
    meter: RefCell<&'meter mut BudgetMeter>,
    depth: Cell<u64>,
}

impl<'meter> ProjectionResources<'meter> {
    pub(super) fn new(meter: &'meter mut BudgetMeter) -> Self {
        Self {
            meter: RefCell::new(meter),
            depth: Cell::new(0),
        }
    }

    pub(super) fn with_meter<T>(
        &self,
        run: impl FnOnce(&mut BudgetMeter, u64) -> Result<T, WireError>,
    ) -> Result<T, WireError> {
        self.with_fallible_meter(run)
    }

    pub(super) fn with_fallible_meter<T, E>(
        &self,
        run: impl FnOnce(&mut BudgetMeter, u64) -> Result<T, E>,
    ) -> Result<T, E> {
        run(&mut self.meter.borrow_mut(), self.depth.get())
    }

    pub(super) fn enter<T>(&self) -> Result<ProjectionDepth<'_>, WireError> {
        let depth = self.depth.get().saturating_add(1);
        self.with_meter(|meter, _| {
            let path = WirePath::root();
            meter.check_semantic_depth(depth, &path)?;
            meter.charge_nodes(1, &path)?;
            meter.charge_owned_bytes(std::mem::size_of::<T>() as u64, &path)?;
            meter.charge_work(depth, &path)
        })?;
        self.depth.set(depth);
        Ok(ProjectionDepth(&self.depth))
    }

    pub(super) fn collection<T>(&self, count: usize) -> Result<(), WireError> {
        self.with_meter(|meter, _| {
            let path = WirePath::root();
            meter.charge_collection_slots(count as u64, &path)?;
            meter.charge_owned_bytes(
                (count as u64).saturating_mul(std::mem::size_of::<T>() as u64),
                &path,
            )?;
            meter.charge_work(count as u64, &path)
        })
    }

    pub(super) fn sort(&self, count: usize) -> Result<(), WireError> {
        self.with_meter(|meter, _| {
            meter.charge_work(
                (count as u64).saturating_mul(u64::from(count.max(1).ilog2()) + 1),
                &WirePath::root(),
            )
        })
    }

    pub(super) fn leaf(&self, bytes: usize) -> Result<(), WireError> {
        self.with_meter(|meter, _| {
            let path = WirePath::root();
            meter.check_semantic_leaf(bytes as u64, &path)?;
            meter.charge_owned_bytes(bytes as u64, &path)?;
            meter.charge_work(bytes as u64, &path)
        })
    }

    pub(super) fn path(&self, path: &StructuralDefinitionPath) -> Result<(), WireError> {
        self.collection::<scoop_identity::StructuralPathSegment>(path.segments().len())
    }

    pub(super) fn selector(&self, selector: &LocalValueSelector) -> Result<(), WireError> {
        match selector {
            LocalValueSelector::This | LocalValueSelector::Parameter { .. } => Ok(()),
            LocalValueSelector::LocalDeclaration { path }
            | LocalValueSelector::BoundReceiver { path }
            | LocalValueSelector::Synthetic { path, .. } => self.path(path),
            LocalValueSelector::SuspensionResult { site } => self.path(site),
        }
    }
}

pub(super) struct ProjectionDepth<'a>(&'a Cell<u64>);
impl Drop for ProjectionDepth<'_> {
    fn drop(&mut self) {
        self.0.set(self.0.get() - 1);
    }
}
