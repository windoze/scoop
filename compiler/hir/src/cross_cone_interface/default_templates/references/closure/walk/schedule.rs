use super::super::{
    CallableTargetView, ConstructorTargetView, ExportDefaultReferenceOccurrenceSiteV1,
    FieldTargetView,
};
use super::{BodyNode, DefaultBodyReferenceVisitorV1, ReferenceWalker, ScheduledWork, WorkItem};
use crate::ExportDefinitionSourceV1;
use scoop_identity::{PersistentObjectValueId, PersistentPropertyId, SignatureTypeKey};

impl<'body, V: DefaultBodyReferenceVisitorV1<'body>> ReferenceWalker<'_, 'body, V> {
    pub(super) fn push_child(
        &mut self,
        pending: &mut Vec<ScheduledWork<'body>>,
        node: BodyNode<'body>,
    ) -> Result<(), V::Error> {
        self.reserve_edge(pending)?;
        pending.push(ScheduledWork {
            work: WorkItem::Body { node },
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

    pub(super) fn push_generic_delegate(
        &mut self,
        pending: &mut Vec<ScheduledWork<'body>>,
        target: &'body crate::DefaultGenericDelegateReferenceV1,
        origin: &'body ExportDefinitionSourceV1,
        site: ExportDefaultReferenceOccurrenceSiteV1,
    ) -> Result<(), V::Error> {
        self.push_leaf(
            pending,
            WorkItem::GenericDelegate {
                target,
                origin,
                site,
            },
        )?;
        for (index, argument) in target.arguments().iter().enumerate().rev() {
            self.push_type(
                pending,
                argument,
                origin,
                crate::DefaultBodyProviderTypeSiteV1::GenericDelegateTypeArgument { index },
            )?;
        }
        Ok(())
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
        self.reserve_edge(pending)?;
        pending.push(ScheduledWork {
            work,
            attachment: self.current,
        });
        Ok(())
    }

    fn reserve_edge<T>(&mut self, pending: &mut Vec<T>) -> Result<(), V::Error> {
        scoop_wire::allocation::try_reserve(pending, 1, self.path).map_err(V::Error::from)
    }
}
