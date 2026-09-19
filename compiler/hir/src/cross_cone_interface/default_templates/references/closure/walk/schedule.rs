use super::super::{
    CallableTargetView, ConstructorTargetView, ExportDefaultReferenceOccurrenceSiteV1,
    FieldTargetView,
};
use super::{BodyNode, DefaultBodyReferenceVisitorV1, ReferenceWalker, ScheduledWork, WorkItem};
use crate::ExportDefinitionSourceV1;
use scoop_identity::{PersistentObjectValueId, PersistentPropertyId, SignatureTypeKey};
use scoop_wire::{WireError, WireErrorKind};

impl<'body, V: DefaultBodyReferenceVisitorV1<'body>> ReferenceWalker<'_, 'body, V> {
    pub(super) fn push_child(
        &mut self,
        pending: &mut Vec<ScheduledWork<'body>>,
        parent_depth: u64,
        node: BodyNode<'body>,
    ) -> Result<(), V::Error> {
        let depth = parent_depth
            .checked_add(1)
            .ok_or_else(|| V::Error::from(integer_out_of_range(self.path)))?;
        self.meter
            .check_semantic_depth(depth, self.path)
            .map_err(V::Error::from)?;
        self.reserve_edge(pending)?;
        pending.push(ScheduledWork {
            work: WorkItem::Body { node, depth },
            attachment: self.current,
        });
        Ok(())
    }

    pub(super) fn push_type(
        &mut self,
        pending: &mut Vec<ScheduledWork<'body>>,
        target: &'body SignatureTypeKey,
        origin: &'body ExportDefinitionSourceV1,
        site: crate::DefaultBodyProviderTypeSiteV1,
    ) -> Result<(), V::Error> {
        self.push_leaf(
            pending,
            WorkItem::Type {
                target,
                origin,
                site: ExportDefaultReferenceOccurrenceSiteV1::BodyType(site),
            },
        )
    }

    pub(super) fn push_callable(
        &mut self,
        pending: &mut Vec<ScheduledWork<'body>>,
        target: CallableTargetView<'body>,
        origin: &'body ExportDefinitionSourceV1,
        site: ExportDefaultReferenceOccurrenceSiteV1,
    ) -> Result<(), V::Error> {
        self.push_leaf(
            pending,
            WorkItem::Callable {
                target,
                origin,
                site,
            },
        )
    }

    pub(super) fn push_constructor(
        &mut self,
        pending: &mut Vec<ScheduledWork<'body>>,
        target: ConstructorTargetView<'body>,
        origin: &'body ExportDefinitionSourceV1,
        site: ExportDefaultReferenceOccurrenceSiteV1,
    ) -> Result<(), V::Error> {
        self.push_leaf(
            pending,
            WorkItem::Constructor {
                target,
                origin,
                site,
            },
        )
    }

    pub(super) fn push_global(
        &mut self,
        pending: &mut Vec<ScheduledWork<'body>>,
        target: PersistentPropertyId,
        origin: &'body ExportDefinitionSourceV1,
        site: ExportDefaultReferenceOccurrenceSiteV1,
    ) -> Result<(), V::Error> {
        self.push_leaf(
            pending,
            WorkItem::Global {
                target,
                origin,
                site,
            },
        )
    }

    pub(super) fn push_singleton(
        &mut self,
        pending: &mut Vec<ScheduledWork<'body>>,
        target: PersistentObjectValueId,
        origin: &'body ExportDefinitionSourceV1,
        site: ExportDefaultReferenceOccurrenceSiteV1,
    ) -> Result<(), V::Error> {
        self.push_leaf(
            pending,
            WorkItem::Singleton {
                target,
                origin,
                site,
            },
        )
    }

    pub(super) fn push_field(
        &mut self,
        pending: &mut Vec<ScheduledWork<'body>>,
        target: FieldTargetView<'body>,
        origin: &'body ExportDefinitionSourceV1,
        site: ExportDefaultReferenceOccurrenceSiteV1,
    ) -> Result<(), V::Error> {
        self.push_leaf(
            pending,
            WorkItem::Field {
                target,
                origin,
                site,
            },
        )
    }

    fn push_leaf(
        &mut self,
        pending: &mut Vec<ScheduledWork<'body>>,
        work: WorkItem<'body>,
    ) -> Result<(), V::Error> {
        self.meter
            .check_semantic_depth(1, self.path)
            .map_err(V::Error::from)?;
        self.reserve_edge(pending)?;
        pending.push(ScheduledWork {
            work,
            attachment: self.current,
        });
        Ok(())
    }

    fn reserve_edge<T>(&mut self, pending: &mut Vec<T>) -> Result<(), V::Error> {
        self.meter
            .charge_edges(1, self.path)
            .map_err(V::Error::from)?;
        self.meter
            .charge_work(1, self.path)
            .map_err(V::Error::from)?;
        self.meter
            .try_reserve_collection_slots(pending, 1, self.path)
            .map_err(V::Error::from)
    }
}

fn integer_out_of_range(path: &scoop_wire::WirePath) -> WireError {
    WireError::new(WireErrorKind::IntegerOutOfRange, path.clone(), None)
}
