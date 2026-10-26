use super::*;
use crate::metadata::interface_layouts::{InterfaceLayouts, InterfaceSlot};

fn finish(mut builder: Builder) -> mir::Module {
    let main = builder.main(Arena::new(), Vec::new());
    builder.finish(main)
}

#[test]
fn complete_diamond_parents_survive_logical_method_deduplication() {
    let mut b = Builder::new();
    let root = b.interface("Root", &["root"]);
    let left = b.interface("Left", &["left"]);
    let right = b.interface("Right", &["right"]);
    let child = b.interface("Child", &["own"]);
    let r = b.interfaces[root].methods[0].clone();
    let l = b.interfaces[left].methods[0].clone();
    let q = b.interfaces[right].methods[0].clone();
    b.interfaces[left].parents = vec![root];
    b.interfaces[left].methods.insert(0, r.clone());
    b.interfaces[right].parents = vec![root];
    b.interfaces[right].methods.insert(0, r.clone());
    b.interfaces[child].parents = vec![left, right];
    b.interfaces[child].methods.splice(0..0, [r, l, q]);
    let module = finish(b);
    let layouts = InterfaceLayouts::new(&module).unwrap();
    let child_layout = layouts.get(child);
    assert_eq!(child_layout.methods, [0, 1, 3, 4]);
    assert_eq!(
        child_layout.slots,
        [
            InterfaceSlot {
                interface: root,
                method: 0
            },
            InterfaceSlot {
                interface: left,
                method: 1
            },
            InterfaceSlot {
                interface: root,
                method: 0
            },
            InterfaceSlot {
                interface: right,
                method: 1
            },
            InterfaceSlot {
                interface: child,
                method: 3
            },
        ]
    );
    assert_eq!(layouts.projection(child, left), Some(0));
    assert_eq!(layouts.projection(child, right), Some(2));
    assert_eq!(layouts.projection(child, root), Some(0));
    assert_eq!(&child_layout.slots[2..4], layouts.get(right).slots);
}

#[test]
fn override_identity_does_not_replace_parent_slots() {
    let mut b = Builder::new();
    let parent = b.interface("Parent", &["read"]);
    let child = b.interface("Child", &["read"]);
    b.interfaces[parent].methods[0].return_type = mir::Type::String;
    b.interfaces[child].methods[0].return_type = mir::Type::String;
    b.interfaces[child].parents = vec![parent];
    let module = finish(b);
    let layouts = InterfaceLayouts::new(&module).unwrap();
    assert_eq!(layouts.get(child).methods, [1]);
    assert_eq!(
        layouts.get(child).slots[0],
        InterfaceSlot {
            interface: parent,
            method: 0
        }
    );
    assert_eq!(layouts.projection(child, parent), Some(0));
}

#[test]
fn exact_generic_parents_with_equal_signatures_are_not_merged() {
    let mut b = Builder::new();
    let first = b.interface("IntReader", &["label"]);
    let second = b.interface("StringReader", &["label"]);
    let child = b.interface("Both", &[]);
    b.interfaces[first].type_arguments = vec![INT];
    b.interfaces[second].type_arguments = vec![mir::Type::String];
    b.interfaces[second].methods[0].identity = b.interfaces[first].methods[0].identity;
    b.interfaces[child].parents = vec![first, second];
    b.interfaces[child].methods = b.interfaces[first].methods.clone();
    let module = finish(b);
    let layouts = InterfaceLayouts::new(&module).unwrap();
    assert_eq!(layouts.projection(child, first), Some(0));
    assert_eq!(layouts.projection(child, second), Some(1));
    assert_eq!(layouts.get(child).methods, [2]);
    assert_eq!(layouts.get(child).slots.len(), 3);
}

#[test]
fn empty_interfaces_need_no_address_arithmetic_or_method_storage() {
    let mut b = Builder::new();
    let parent = b.interface("Empty", &[]);
    let child = b.interface("EmptyChild", &[]);
    b.interfaces[child].parents = vec![parent];
    let module = finish(b);
    let layouts = InterfaceLayouts::new(&module).unwrap();
    assert!(layouts.get(child).slots.is_empty());
    assert_eq!(layouts.projection(child, parent), Some(0));
}
