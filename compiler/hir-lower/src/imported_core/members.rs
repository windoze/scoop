//! Member lookup through the actual dependency receiver hierarchy.

use super::*;
use hir::ImportedCallableSource;

impl Lowerer {
    pub(crate) fn imported_member_candidates(
        &self,
        receiver: hir::TypeId,
        lookup: hir::ImportedMemberLookup<'_>,
    ) -> Result<Vec<hir::ImportedCallableDeclaration>, hir::ImportedDependencyCandidateError> {
        let Some(dependencies) = &self.dependencies else {
            return Ok(Vec::new());
        };
        let effective = match &self.types[receiver] {
            hir::Type::ImportedInterface(interface) => Some(
                interface
                    .methods
                    .iter()
                    .map(|method| method.declaration.declaration())
                    .collect::<std::collections::BTreeSet<_>>(),
            ),
            _ => None,
        };
        let mut pending = vec![receiver];
        let mut seen = std::collections::BTreeSet::new();
        let mut suppressed_slots = std::collections::BTreeSet::new();
        let mut result = Vec::<(bool, hir::ImportedCallableDeclaration)>::new();
        while let Some(ty) = pending.pop() {
            if !seen.insert(ty) {
                continue;
            }
            let Some(owner) = self.imported_nominal_declaration(ty) else {
                continue;
            };
            let class_member = matches!(self.types[ty], hir::Type::ImportedClass(_));
            if let hir::Type::ImportedInterface(interface) = &self.types[ty] {
                suppressed_slots.extend(
                    interface
                        .methods
                        .iter()
                        .flat_map(|method| &method.overrides)
                        .copied(),
                );
            }
            for candidate in dependencies
                .member_callable_candidates(hir::SourceNominalId::Concrete(owner), lookup)?
            {
                let declaration = candidate.interface();
                if effective
                    .as_ref()
                    .is_some_and(|methods| !methods.contains(&declaration.declaration()))
                {
                    continue;
                }
                if result.iter().any(|(selected_class, selected)| {
                    let selected_declaration = selected.interface();
                    selected_declaration.declaration() == declaration.declaration()
                        || selected_declaration
                            .slot_relations()
                            .values()
                            .iter()
                            .any(|slot| declaration.slot_relations().values().contains(slot))
                        || (*selected_class
                            && !class_member
                            && same_member_signature(selected, &candidate))
                }) {
                    continue;
                }
                result.push((class_member, candidate));
            }
            match &self.types[ty] {
                hir::Type::ImportedClass(class) => {
                    pending.extend(class.interfaces.iter().rev().copied());
                    pending.extend(class.base_class);
                }
                hir::Type::ImportedInterface(interface) => {
                    pending.extend(interface.parents.iter().rev().copied())
                }
                _ => {}
            }
        }
        Ok(result
            .into_iter()
            .filter(|(_, candidate)| {
                !candidate
                    .interface()
                    .slot_relations()
                    .values()
                    .iter()
                    .any(|slot| suppressed_slots.contains(slot))
            })
            .map(|(_, candidate)| candidate)
            .collect())
    }
}

fn same_member_signature(
    selected: &hir::ImportedCallableDeclaration,
    candidate: &hir::ImportedCallableDeclaration,
) -> bool {
    let left = selected.interface();
    let right = candidate.interface();
    selected.name() == candidate.name()
        && left.type_parameters() == right.type_parameters()
        && left.result() == right.result()
        && left.effects().execution() == right.effects().execution()
        && left.effects().operator_role() == right.effects().operator_role()
        && left.effects().infix() == right.effects().infix()
        && left
            .parameters()
            .parameters()
            .iter()
            .map(|parameter| parameter.value_type())
            .eq(right
                .parameters()
                .parameters()
                .iter()
                .map(|parameter| parameter.value_type()))
}
