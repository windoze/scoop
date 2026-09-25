use super::*;

impl Replay<'_> {
    pub(super) fn reserve<T>(&mut self, count: usize) -> Result<Vec<T>> {
        let path = WirePath::root();

        let mut values = Vec::new();
        scoop_wire::allocation::try_reserve(&mut values, count, &path)?;
        Ok(values)
    }
}
