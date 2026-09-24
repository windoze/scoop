use super::*;

impl Replay<'_, '_> {
    pub(super) fn reserve<T>(&mut self, count: usize) -> Result<Vec<T>> {
        let path = WirePath::root();
        self.meter.check_table_entries(count as u64, &path)?;
        let mut values = Vec::new();
        self.meter
            .try_reserve_collection_slots(&mut values, count, &path)?;
        Ok(values)
    }

    pub(super) fn lookup(&mut self, count: usize) -> Result<()> {
        self.meter
            .charge_work(u64::from(count.max(1).ilog2()) + 1, &WirePath::root())?;
        Ok(())
    }

    pub(super) fn index_entry<T>(&mut self) -> Result<()> {
        let path = WirePath::root();
        self.meter.charge_collection_slots(1, &path)?;
        self.meter
            .charge_owned_bytes(std::mem::size_of::<T>() as u64, &path)?;
        Ok(())
    }
}
