//! Complete parent-table prefixes, independent of the implementing type.

use std::collections::{HashMap, HashSet};

use super::*;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct InterfaceSlot {
    pub interface: mir::InterfaceId,
    pub method: u32,
}

pub(crate) struct InterfaceLayout {
    pub slots: Vec<InterfaceSlot>,
    pub methods: Vec<u32>,
    /// First complete view of each exact ancestor, in pointer-sized slots.
    pub parents: HashMap<mir::InterfaceId, u32>,
}

#[derive(Default)]
pub(crate) struct InterfaceLayouts(HashMap<mir::InterfaceId, InterfaceLayout>);

impl InterfaceLayouts {
    pub fn new(module: &mir::Module) -> StorageResult<Self> {
        let mut layouts = Self::default();
        let mut visiting = HashSet::new();
        for (id, _) in module.interfaces.iter() {
            layouts.build(module, id, &mut visiting)?;
        }
        Ok(layouts)
    }

    pub fn get(&self, id: mir::InterfaceId) -> &InterfaceLayout {
        &self.0[&id]
    }

    pub fn projection(&self, child: mir::InterfaceId, parent: mir::InterfaceId) -> Option<u32> {
        if child == parent {
            Some(0)
        } else {
            self.get(child).parents.get(&parent).copied()
        }
    }

    fn build(
        &mut self,
        module: &mir::Module,
        interface: mir::InterfaceId,
        visiting: &mut HashSet<mir::InterfaceId>,
    ) -> StorageResult<()> {
        if self.0.contains_key(&interface) {
            return Ok(());
        }
        if !visiting.insert(interface) {
            return Err(invalid("interface inheritance cycle"));
        }
        let definition = &module.interfaces[interface];
        let mut slots = Vec::new();
        let mut parents = HashMap::new();
        for &parent in &definition.parents {
            self.build(module, parent, visiting)?;
            let offset = position(slots.len())?;
            let layout = self.get(parent);
            parents.entry(parent).or_insert(offset);
            for (&ancestor, &relative) in &layout.parents {
                let absolute = offset
                    .checked_add(relative)
                    .ok_or_else(|| invalid("interface table offset overflow"))?;
                parents.entry(ancestor).or_insert(absolute);
            }
            position(
                slots
                    .len()
                    .checked_add(layout.slots.len())
                    .ok_or_else(|| invalid("interface table size overflow"))?,
            )?;
            slots.extend_from_slice(&layout.slots);
        }
        let inherited = slots.len();
        let mut methods = Vec::with_capacity(definition.methods.len());
        for (index, method) in definition.methods.iter().enumerate() {
            // Distinct exact generic origins remain distinct even when their
            // argument and result representations happen to coincide.
            let mut matched = None;
            let mut ambiguous = false;
            for (offset, source) in slots[..inherited].iter().enumerate() {
                let candidate =
                    &module.interfaces[source.interface].methods[source.method as usize];
                if same_method(method, candidate) {
                    match matched {
                        None => matched = Some((offset, *source)),
                        Some((_, previous)) if previous != *source => ambiguous = true,
                        _ => {}
                    }
                }
            }
            let offset = match matched.filter(|_| !ambiguous) {
                Some((offset, _)) => position(offset)?,
                None => {
                    let offset = position(slots.len())?;
                    slots.push(InterfaceSlot {
                        interface,
                        method: position(index)?,
                    });
                    offset
                }
            };
            methods.push(offset);
        }
        position(slots.len())?;
        self.0.insert(
            interface,
            InterfaceLayout {
                slots,
                methods,
                parents,
            },
        );
        visiting.remove(&interface);
        Ok(())
    }
}

fn same_method(left: &mir::InterfaceMethod, right: &mir::InterfaceMethod) -> bool {
    left.identity == right.identity
        && left.gc_effect == right.gc_effect
        && left
            .parameters
            .iter()
            .skip(1)
            .eq(right.parameters.iter().skip(1))
        && left.return_type == right.return_type
}

fn position(value: usize) -> StorageResult<u32> {
    u32::try_from(value).map_err(|_| invalid("interface table size overflow"))
}

fn invalid(message: &'static str) -> StorageLoweringError {
    StorageLoweringError::InvalidRepresentation(message)
}
