//! Typed, appendable arenas whose candidate snapshots share unchanged entries.

use std::cell::OnceCell;
use std::ops::{Index, IndexMut};
use std::rc::Rc;

use imbl::Vector;
use la_arena::{Arena, Idx, RawIdx};

#[derive(Clone)]
pub(crate) struct SnapshotArena<T: Clone> {
    entries: Vector<Rc<T>>,
    // Identity projections use the public IR arena API. Materialize one view
    // per unchanged snapshot, outside ordinary expression lowering.
    view: Rc<OnceCell<Arena<T>>>,
}

impl<T: Clone> Default for SnapshotArena<T> {
    fn default() -> Self {
        Self {
            entries: Vector::new(),
            view: Rc::default(),
        }
    }
}

impl<T: Clone> SnapshotArena<T> {
    pub(crate) fn alloc(&mut self, value: T) -> Idx<T> {
        let id = index(self.entries.len());
        self.view = Rc::default();
        self.entries.push_back(Rc::new(value));
        id
    }

    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(crate) fn iter(&self) -> impl DoubleEndedIterator<Item = (Idx<T>, &T)> + ExactSizeIterator {
        self.entries
            .iter()
            .enumerate()
            .map(|(i, value)| (index(i), value.as_ref()))
    }

    pub(crate) fn as_arena(&self) -> &Arena<T> {
        self.view.get_or_init(|| {
            self.entries
                .iter()
                .map(|value| value.as_ref().clone())
                .collect()
        })
    }

    pub(crate) fn into_arena(self) -> Arena<T> {
        drop(self.view);
        self.entries.into_iter().map(Rc::unwrap_or_clone).collect()
    }
}

impl<T: Clone> From<Arena<T>> for SnapshotArena<T> {
    fn from(arena: Arena<T>) -> Self {
        Self {
            entries: arena.into_iter().map(|(_, value)| Rc::new(value)).collect(),
            view: Rc::default(),
        }
    }
}

impl<T: Clone> Index<Idx<T>> for SnapshotArena<T> {
    type Output = T;

    fn index(&self, id: Idx<T>) -> &T {
        &self.entries[u32::from(id.into_raw()) as usize]
    }
}

impl<T: Clone> IndexMut<Idx<T>> for SnapshotArena<T> {
    fn index_mut(&mut self, id: Idx<T>) -> &mut T {
        self.view = Rc::default();
        Rc::make_mut(&mut self.entries[u32::from(id.into_raw()) as usize])
    }
}

fn index<T>(value: usize) -> Idx<T> {
    Idx::from_raw(RawIdx::from(
        u32::try_from(value).expect("HIR arena identity space exhausted"),
    ))
}

#[cfg(test)]
mod tests;
