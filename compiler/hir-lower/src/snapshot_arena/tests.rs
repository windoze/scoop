use super::*;
use std::cell::Cell;

struct Entry {
    value: usize,
    copies: Rc<Cell<usize>>,
}

impl Clone for Entry {
    fn clone(&self) -> Self {
        self.copies.set(self.copies.get() + 1);
        Self {
            value: self.value,
            copies: self.copies.clone(),
        }
    }
}

#[test]
fn snapshot_updates_do_not_copy_unrelated_entries() {
    let copies = Rc::new(Cell::new(0));
    let mut original = SnapshotArena::default();
    for value in 0..2048 {
        original.alloc(Entry {
            value,
            copies: copies.clone(),
        });
    }
    let mut candidate = original.clone();
    candidate[index(1024)].value = 7;
    let added = candidate.alloc(Entry {
        value: 2048,
        copies: copies.clone(),
    });
    assert_eq!(copies.get(), 1);
    assert_eq!(original[index(1024)].value, 1024);
    assert_eq!(original.len(), 2048);
    assert_eq!(candidate[added].value, 2048);
    assert_eq!(candidate[index(1024)].value, 7);
}

#[test]
fn projected_arenas_keep_each_snapshot_and_its_ids() {
    let mut original = SnapshotArena::default();
    let first = original.alloc(String::from("original"));
    let mut candidate = original.clone();
    assert_eq!(candidate.as_arena()[first], "original");
    candidate[first] = String::from("candidate");
    let second = candidate.alloc(String::from("new"));
    assert_eq!(original.as_arena()[first], "original");
    assert_eq!(candidate.as_arena()[first], "candidate");
    assert_eq!(original.as_arena().len(), 1);
    let output = candidate.into_arena();
    assert_eq!(output[first], "candidate");
    assert_eq!(output[second], "new");
    assert_eq!(original.into_arena()[first], "original");
}
