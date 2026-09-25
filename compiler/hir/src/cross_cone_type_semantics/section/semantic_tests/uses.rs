use super::*;
use std::cell::Cell;

#[derive(Clone, Copy)]
pub(super) struct Root {
    pub request: SelectedExternalTypeUseV1,
    pub origin: TypeSectionCommittedRootOriginV1,
    pub permitted: bool,
}
#[derive(Default)]
pub(super) struct Uses {
    pub roots: Vec<Root>,
    pub edges: Vec<(SelectedExternalTypeUseV1, Vec<Root>)>,
    pub root_calls: Cell<usize>,
    pub edge_calls: Cell<usize>,
}
impl Uses {
    pub fn new(requests: &[SelectedExternalTypeUseV1]) -> Self {
        Self {
            roots: requests
                .iter()
                .map(|request| Root {
                    request: *request,
                    origin: TypeSectionCommittedRootOriginV1::Source,
                    permitted: true,
                })
                .collect(),
            ..Self::default()
        }
    }
}
impl CommittedTypeUseSemanticAuthorityV1<&'static str> for Uses {
    type Root = Root;
    type Edge = Root;
    fn committed_roots(&self) -> Result<&[Root], &'static str> {
        Ok(&self.roots)
    }
    fn root_request(&self, root: &Root) -> Result<SelectedExternalTypeUseV1, &'static str> {
        Ok(root.request)
    }
    fn root_origin(&self, root: &Root) -> Result<TypeSectionCommittedRootOriginV1, &'static str> {
        Ok(root.origin)
    }
    fn validate_root(
        &self,
        root: &Root,
        target: CheckedTypeSelectionTargetV1<'_>,
        _context: TypeSectionUseContextV1<'_>,

        _path: &WirePath,
    ) -> Result<(), &'static str> {
        self.root_calls.set(self.root_calls.get() + 1);
        if root.request == target.request() && root.permitted {
            Ok(())
        } else {
            Err("source access denied")
        }
    }
    fn semantic_edges(
        &self,
        parent: CheckedTypeSelectionTargetV1<'_>,
    ) -> Result<&[Root], &'static str> {
        Ok(self
            .edges
            .iter()
            .find(|(request, _)| *request == parent.request())
            .map_or(&[], |(_, edges)| edges))
    }
    fn edge_request(&self, edge: &Root) -> Result<SelectedExternalTypeUseV1, &'static str> {
        Ok(edge.request)
    }
    fn edge_origin(&self, edge: &Root) -> Result<TypeSectionCommittedRootOriginV1, &'static str> {
        Ok(edge.origin)
    }
    fn validate_edge(
        &self,
        _parent: CheckedTypeSelectionTargetV1<'_>,
        edge: &Root,
        target: CheckedTypeSelectionTargetV1<'_>,
        _context: TypeSectionUseContextV1<'_>,

        _path: &WirePath,
    ) -> Result<(), &'static str> {
        self.edge_calls.set(self.edge_calls.get() + 1);
        if edge.request == target.request() && edge.permitted {
            Ok(())
        } else {
            Err("semantic edge denied")
        }
    }
}
