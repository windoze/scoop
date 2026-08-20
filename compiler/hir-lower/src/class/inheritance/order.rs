use std::collections::{HashMap, HashSet};

use super::*;

impl Lowerer {
    /// Source methods acquire their inherited virtual family only after the
    /// parent declaration has been checked. Imported parents already carry it.
    pub(super) fn source_inheritance_order(
        &self,
        owners: Vec<(Owner, usize)>,
    ) -> Vec<(Owner, usize)> {
        let files = owners.iter().copied().collect::<HashMap<_, _>>();
        let mut visited = HashSet::new();
        let mut ordered = Vec::with_capacity(owners.len());
        for (owner, _) in owners {
            let mut stack = vec![(owner, false)];
            while let Some((owner, expanded)) = stack.pop() {
                let Some(&file) = files.get(&owner) else {
                    continue;
                };
                if expanded {
                    ordered.push((owner, file));
                } else if visited.insert(owner) {
                    stack.push((owner, true));
                    stack.extend(
                        self.source_inheritance_parents(owner)
                            .into_iter()
                            .rev()
                            .map(|parent| (parent, false)),
                    );
                }
            }
        }
        ordered
    }

    fn source_inheritance_parents(&self, owner: Owner) -> Vec<Owner> {
        let parents = match owner {
            Owner::Class(id) => self.classes[id]
                .base_class
                .iter()
                .chain(&self.classes[id].interfaces)
                .copied()
                .collect(),
            Owner::Object(id) => {
                let class = &self.classes[self.objects[id].backing_class];
                class
                    .base_class
                    .iter()
                    .chain(&class.interfaces)
                    .copied()
                    .collect()
            }
            Owner::Interface(id) => self.interfaces[id].parents.clone(),
            Owner::Struct(id) => self.structs[id].interfaces.clone(),
            Owner::Enum(id) => self.enums[id].interfaces.clone(),
        };
        parents
            .into_iter()
            .filter_map(|parent| match self.types[parent] {
                Type::Class(application) => self
                    .source_class_id(self.class_applications[application].template)
                    .map(Owner::Class),
                Type::Interface(application) => self
                    .source_interface_id(self.interface_applications[application].template)
                    .map(Owner::Interface),
                _ => unreachable!("resolved nominal parents are class or interface types"),
            })
            .collect()
    }
}
