use super::*;

impl Writer<'_, '_> {
    pub(super) fn safepoint(&mut self, site: SafepointSiteRef) -> Result {
        self.id(&self.function.safepoints[site].site_id())
    }

    fn root_key(&self, source: CallerRootSource) -> (u64, u64) {
        match source {
            CallerRootSource::Param(index) => (1, u64::from(index)),
            CallerRootSource::Local(id) => (2, self.ids.locals[&id]),
            CallerRootSource::Temp(id) => (3, self.ids.temps[&id]),
        }
    }

    fn root_source(&mut self, source: CallerRootSource) -> Result {
        let (kind, rank) = self.root_key(source);
        record!(self, kind; self.u(rank))
    }

    pub(super) fn live(&mut self, live: &StatepointLiveSet) -> Result {
        if self.ordering {
            return self.e.array(0);
        }
        let mut values = live.as_slice().iter().collect::<Vec<_>>();
        values.sort_by_key(|value| self.root_key(value.source));
        self.e.array(values.len() as u64)?;
        for value in values {
            self.e.map(3)?;
            self.e.field(1)?;
            self.root_source(value.source)?;
            self.e.field(2)?;
            self.ty(&value.ty)?;
            self.e.field(3)?;
            self.e.array(value.leaves.as_slice().len() as u64)?;
            for leaf in value.leaves.as_slice() {
                self.u(leaf.byte_offset)?;
            }
        }
        Ok(())
    }

    pub(super) fn caller_roots(&mut self, roots: &[CallerRoot]) -> Result {
        if self.ordering {
            return self.e.array(0);
        }
        let mut roots = roots.iter().collect::<Vec<_>>();
        roots.sort_by_key(|root| self.root_key(root.source));
        self.e.array(roots.len() as u64)?;
        for root in roots {
            self.caller_root(root)?;
        }
        Ok(())
    }

    fn caller_root(&mut self, root: &CallerRoot) -> Result {
        record!(self, 1; self.root_source(root.source), self.scan(root.scan.as_ref_scan()))
    }

    pub(super) fn exceptional_roots(&mut self, roots: &ExceptionalRootSet) -> Result {
        if self.ordering {
            return self.e.array(0);
        }
        let mut roots = roots.as_slice().iter().collect::<Vec<_>>();
        roots.sort_by_key(|root| self.root_key(root.root.source));
        self.e.array(roots.len() as u64)?;
        for root in roots {
            record!(self, 1; self.caller_root(&root.root), self.boolean(root.normal_live), self.boolean(root.unwind_live))?;
        }
        Ok(())
    }
}
