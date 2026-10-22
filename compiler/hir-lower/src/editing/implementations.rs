//! Reverse nominal relationships reuse the checked declaration graph.

use std::collections::{HashMap, HashSet};

use super::*;
use crate::Owner;
use hir::{EditingTypeIdentity as Identity, EditingUnavailable as U, EditingValue as V};

mod members;
mod targets;

impl DeclarationAnalysis {
    pub fn type_implementations(
        &mut self,
        declaration: hir::SourceNominalId,
        is_cancelled: impl Fn() -> bool,
    ) -> Result<V<&[hir::EditingTypeImplementation]>, EditingCancelled> {
        if is_cancelled() {
            return Err(EditingCancelled);
        }
        if !self.declarations_ready {
            return Ok(V::Unavailable(U::DeclarationFailure));
        }
        if self.type_implementations.is_none() {
            let mut index = HashMap::<_, Vec<_>>::new();
            for reference in &self.type_references {
                if is_cancelled() {
                    return Err(EditingCancelled);
                }
                let Identity::Nominal(declaration) = reference.target else {
                    continue;
                };
                if !reference.is_declaration {
                    continue;
                }
                let Some(&owner) = self.lowerer.nominal_owners.get(&declaration) else {
                    continue;
                };
                let mut ancestors = HashSet::new();
                for interface in self.lowerer.owner_interfaces(owner) {
                    if is_cancelled() {
                        return Err(EditingCancelled);
                    }
                    if let Some(ancestor) = self.lowerer.semantic_nominal_identity(interface) {
                        ancestors.insert(ancestor);
                    }
                }
                let class = match owner {
                    Owner::Class(id) => Some(id),
                    Owner::Object(id) => Some(self.lowerer.objects[id].backing_class),
                    _ => None,
                };
                let mut base = class.and_then(|id| self.lowerer.classes[id].base_class);
                while let Some(ty) = base {
                    if is_cancelled() {
                        return Err(EditingCancelled);
                    }
                    let hir::Type::Class(application) = self.lowerer.types[ty] else {
                        unreachable!("a checked class base retains a class application")
                    };
                    let ancestor = self.lowerer.class_applications[application].template;
                    if !ancestors.insert(ancestor) {
                        break;
                    }
                    base = self.lowerer.class_definition(ancestor).base_class;
                }
                ancestors.remove(&declaration);
                for ancestor in ancestors {
                    index
                        .entry(ancestor)
                        .or_default()
                        .push(hir::EditingTypeImplementation {
                            declaration,
                            origin: reference.origin.clone(),
                        });
                }
            }
            if is_cancelled() {
                return Err(EditingCancelled);
            }
            self.type_implementations = Some(index);
        }
        Ok(V::Known(
            self.type_implementations
                .as_ref()
                .expect("the declaration input has a complete nominal index")
                .get(&declaration)
                .map_or(&[], Vec::as_slice),
        ))
    }
}
