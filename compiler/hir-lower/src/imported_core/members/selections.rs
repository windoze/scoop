use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use scoop_identity::{CallableTemplateOrigin, PersistentDispatchSlotId};

use super::*;

pub(super) type SelectedInterfaceSources =
    BTreeMap<(hir::TypeId, PersistentDispatchSlotId), CallableTemplateOrigin>;

impl Lowerer {
    pub(super) fn record_selected_interface_sources(
        &mut self,
        ty: hir::TypeId,
        selected: &mut SelectedInterfaceSources,
    ) {
        let (declaration, mut pending) = match &self.types[ty] {
            hir::Type::Class(application) => {
                let Some(definition) = self
                    .loaded_class_definitions
                    .get(&self.class_applications[*application].template)
                else {
                    return;
                };
                let declaration = Arc::clone(&definition.declaration);
                (declaration, self.direct_nominal_supertypes(ty))
            }
            hir::Type::Struct(application) => {
                let Some(definition) = self
                    .loaded_struct_definitions
                    .get(&self.struct_applications[*application].template)
                else {
                    return;
                };
                let declaration = Arc::clone(&definition.declaration);
                (declaration, self.direct_nominal_supertypes(ty))
            }
            hir::Type::Enum(application) => {
                let Some(definition) = self
                    .loaded_enum_definitions
                    .get(&self.enum_applications[*application].template)
                else {
                    return;
                };
                let declaration = Arc::clone(&definition.declaration);
                (declaration, self.direct_nominal_supertypes(ty))
            }
            hir::Type::Integer(_) | hir::Type::Boolean | hir::Type::String => {
                let kind = match self.types[ty] {
                    hir::Type::Integer(kind) => hir::IntrinsicTypeKind::Integer(kind),
                    hir::Type::Boolean => hir::IntrinsicTypeKind::Boolean,
                    hir::Type::String => hir::IntrinsicTypeKind::String,
                    _ => unreachable!("primitive receiver kind"),
                };
                let Some(value) = self.imported_intrinsic_types.get(&kind) else {
                    return;
                };
                (Arc::clone(&value.declaration), value.interfaces.clone())
            }
            _ => return,
        };
        let selections = declaration
            .interface
            .declaration_details()
            .dispatch_selections();
        let mut seen = BTreeSet::new();
        while let Some(owner) = pending.pop() {
            if !seen.insert(owner) {
                continue;
            }
            if matches!(self.types[owner], hir::Type::Class(_)) {
                pending.extend(self.direct_nominal_supertypes(owner));
                continue;
            }
            pending.extend(self.direct_nominal_supertypes(owner));
            let interface = self
                .dependency_interface_definition(owner)
                .expect("dependency parents retain their complete nominal types");
            for method in &interface.methods {
                let slot = method.slot.id();
                let target = selections
                    .records()
                    .iter()
                    .find(|selection| selection.slot() == slot)
                    .expect("the declaration retains its selected interface source")
                    .callable_target();
                selected.entry((owner, slot)).or_insert(target);
            }
        }
    }
}
