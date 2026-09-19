use scoop_wire::{BudgetMeter, WireError, WireErrorKind, WirePath};

use crate::CanonicalTemplateLocalTableV1;

mod entries;
mod leaves;
mod local;

#[cfg(test)]
mod tests;

pub(crate) trait ResolutionNode {
    fn children<'a>(&'a self, children: &mut ResourceChildren<'a, '_>) -> Result<(), WireError>;
}

struct Pending<'a> {
    node: &'a dyn ResolutionNode,
    depth: u64,
}

pub(crate) struct ResourceChildren<'a, 'm> {
    pending: &'m mut Vec<Pending<'a>>,
    pub(crate) meter: &'m mut BudgetMeter,
    pub(crate) depth: u64,
    pub(crate) copies: u64,
    locals: Option<&'a CanonicalTemplateLocalTableV1>,
    selector_bytes: u64,
}
impl<'a> ResourceChildren<'a, '_> {
    pub(crate) fn push(&mut self, node: &'a dyn ResolutionNode) -> Result<(), WireError> {
        let path = WirePath::root();
        let depth = self.child_depth()?;
        self.meter.charge_work(1, &path)?;
        self.meter.charge_edges(self.copies, &path)?;
        self.meter
            .try_reserve_collection_slots(self.pending, 1, &path)?;
        self.pending.push(Pending { node, depth });
        Ok(())
    }

    fn push_slice<T: ResolutionNode>(&mut self, values: &'a [T]) -> Result<(), WireError> {
        if values.is_empty() {
            return Ok(());
        }
        let path = WirePath::root();
        let depth = self.child_depth()?;
        let count = values.len() as u64;
        self.meter.charge_work(count, &path)?;
        let edges = count
            .checked_mul(self.copies)
            .ok_or_else(|| WireError::new(WireErrorKind::IntegerOutOfRange, path.clone(), None))?;
        self.meter.charge_edges(edges, &path)?;
        self.meter
            .try_reserve_collection_slots(self.pending, values.len(), &path)?;
        self.pending
            .extend(values.iter().rev().map(|node| Pending { node, depth }));
        Ok(())
    }

    fn child_depth(&self) -> Result<u64, WireError> {
        let path = WirePath::root();
        let depth = self
            .depth
            .checked_add(1)
            .ok_or_else(|| WireError::new(WireErrorKind::IntegerOutOfRange, path.clone(), None))?;
        self.meter.check_semantic_depth(depth, &path)?;
        Ok(depth)
    }

    pub(crate) fn leaf_bytes(&mut self, bytes: u64) -> Result<(), WireError> {
        let path = WirePath::root();
        let bytes = bytes
            .checked_mul(self.copies)
            .ok_or_else(|| WireError::new(WireErrorKind::IntegerOutOfRange, path.clone(), None))?;
        self.meter.charge_owned_bytes(bytes, &path)?;
        self.meter.charge_work(bytes, &path)
    }
}

pub(crate) fn charge(
    root: &dyn ResolutionNode,
    locals: Option<&CanonicalTemplateLocalTableV1>,
    copies: u64,
    meter: &mut BudgetMeter,
) -> Result<(), WireError> {
    let selector_bytes = local::selector_bound(locals, meter)?;
    let path = WirePath::root();
    let mut pending = Vec::new();
    meter.try_reserve_collection_slots(&mut pending, 1, &path)?;
    pending.push(Pending {
        node: root,
        depth: 1,
    });
    while let Some(Pending { node, depth }) = pending.pop() {
        meter.check_semantic_depth(depth, &path)?;
        meter.charge_nodes(copies, &path)?;
        meter.charge_work(copies, &path)?;
        node.children(&mut ResourceChildren {
            pending: &mut pending,
            meter,
            depth,
            copies,
            locals,
            selector_bytes,
        })?;
    }
    Ok(())
}

impl<T: ResolutionNode> ResolutionNode for Vec<T> {
    fn children<'a>(&'a self, children: &mut ResourceChildren<'a, '_>) -> Result<(), WireError> {
        for _ in 0..children.copies {
            children
                .meter
                .charge_collection_slots(self.len() as u64, &WirePath::root())?;
        }
        children.push_slice(self)
    }
}
impl<T: ResolutionNode> ResolutionNode for Box<T> {
    fn children<'a>(&'a self, children: &mut ResourceChildren<'a, '_>) -> Result<(), WireError> {
        children.push(self.as_ref())
    }
}

macro_rules! resource_node {
    ($target:ty, $node:ident, $children:ident, $body:block) => {
        impl $crate::cross_cone_interface::default_templates::resolution_resources::ResolutionNode for $target {
            fn children<'a>(
                &'a self,
                $children: &mut $crate::cross_cone_interface::default_templates::resolution_resources::ResourceChildren<'a, '_>,
            ) -> Result<(), scoop_wire::WireError> {
                let $node = self;
                $body
            }
        }
    };
}
pub(crate) use resource_node;
