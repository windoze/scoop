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
            match &self.types[ty] {
                hir::Type::Class(application) => {
                    let declaration = &self.classes[self.class_applications[*application].template];
                    pending.extend(declaration.interfaces.iter().rev().copied());
                    pending.extend(declaration.base_class);
                    suppress_local_implementations(
                        &declaration.interface_implementations,
                        &mut suppressed_slots,
                    );
                }
                hir::Type::Struct(application) => {
                    let declaration =
                        &self.structs[self.struct_applications[*application].template];
                    pending.extend(declaration.interfaces.iter().rev().copied());
                    suppress_local_implementations(
                        &declaration.interface_implementations,
                        &mut suppressed_slots,
                    );
                }
                hir::Type::Enum(application) => {
                    let declaration = &self.enums[self.enum_applications[*application].template];
                    pending.extend(declaration.interfaces.iter().rev().copied());
                    suppress_local_implementations(
                        &declaration.interface_implementations,
                        &mut suppressed_slots,
                    );
                }
                hir::Type::Interface(application) => {
                    let declaration =
                        &self.interfaces[self.interface_applications[*application].template];
                    pending.extend(declaration.parents.iter().rev().copied());
                    for reference in declaration
                        .methods
                        .iter()
                        .flat_map(|method| &self.interface_method_entities[*method].overrides)
                    {
                        if let hir::InterfaceMethodReference::Imported { slot, .. } = reference {
                            suppressed_slots.insert(*slot);
                        }
                    }
                }
                hir::Type::ImportedClass(class) => {
                    pending.extend(class.interfaces.iter().rev().copied());
                    pending.extend(class.base_class);
                }
                hir::Type::ImportedInterface(interface) => {
                    pending.extend(interface.parents.iter().rev().copied())
                }
                hir::Type::ImportedStruct(structure) => {
                    pending.extend(structure.interfaces.iter().rev().copied())
                }
                hir::Type::ImportedEnum(enumeration) => {
                    pending.extend(enumeration.interfaces.iter().rev().copied())
                }
                hir::Type::Integer(_) | hir::Type::Boolean | hir::Type::String => {
                    let kind = match self.types[ty] {
                        hir::Type::Integer(kind) => hir::IntrinsicTypeKind::Integer(kind),
                        hir::Type::Boolean => hir::IntrinsicTypeKind::Boolean,
                        hir::Type::String => hir::IntrinsicTypeKind::String,
                        _ => unreachable!("intrinsic member receiver kind"),
                    };
                    if let Some(source) = self.imported_intrinsic_types.get(&kind) {
                        pending.extend(source.interfaces.iter().rev().copied());
                    }
                }
                hir::Type::Param(parameter) => {
                    if let Some(declaration) = self
                        .type_params_in_scope
                        .iter()
                        .find(|declaration| declaration.id == *parameter)
                    {
                        pending.extend(
                            declaration
                                .nominal_bounds_in_source_order()
                                .into_iter()
                                .map(|bound| match bound {
                                    hir::NominalBoundRef::Class(bound) => {
                                        self.class_applications[bound.application].canonical_type
                                    }
                                    hir::NominalBoundRef::Interface(bound) => {
                                        self.interface_applications[bound.application]
                                            .canonical_type
                                    }
                                }),
                        );
                    }
                }
                _ => {}
            }
            let Some(owner) = self.imported_nominal_declaration(ty) else {
                continue;
            };
            let non_interface = !matches!(self.types[ty], hir::Type::ImportedInterface(_));
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
                if result.iter().any(|(selected_nominal, selected)| {
                    let selected_declaration = selected.interface();
                    selected_declaration.declaration() == declaration.declaration()
                        || selected_declaration
                            .slot_relations()
                            .values()
                            .iter()
                            .any(|slot| declaration.slot_relations().values().contains(slot))
                        || (*selected_nominal
                            && !non_interface
                            && same_member_signature(selected, &candidate))
                }) {
                    continue;
                }
                result.push((non_interface, candidate));
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

fn suppress_local_implementations(
    implementations: &[hir::InterfaceImplementation],
    slots: &mut std::collections::BTreeSet<scoop_identity::PersistentDispatchSlotId>,
) {
    for method in implementations
        .iter()
        .flat_map(|implementation| &implementation.methods)
    {
        if matches!(method.target, hir::InterfaceImplementationTarget::Method(_))
            && let hir::InterfaceMethodReference::Imported { slot, .. } = method.member
        {
            slots.insert(slot);
        }
    }
}
