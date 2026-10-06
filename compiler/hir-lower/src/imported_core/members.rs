//! Member lookup through the actual dependency receiver hierarchy.

use super::*;
use hir::ImportedCallableSource;

mod selections;
use selections::SelectedInterfaceSources;

impl Lowerer {
    pub(crate) fn imported_member_owner_type(
        &mut self,
        receiver: hir::TypeId,
        owner: hir::SourceNominalId,
    ) -> Option<hir::TypeId> {
        let mut pending = vec![receiver];
        let mut seen = std::collections::BTreeSet::new();
        while let Some(ty) = pending.pop() {
            if !seen.insert(ty) {
                continue;
            }
            if self.imported_nominal_owner(ty) == Some(owner) {
                return Some(ty);
            }
            pending.extend(self.direct_nominal_supertypes(ty));
        }
        None
    }

    pub(crate) fn imported_member_candidates(
        &mut self,
        receiver: hir::TypeId,
        lookup: hir::ImportedMemberLookup<'_>,
    ) -> Result<Vec<hir::ImportedCallableDeclaration>, hir::ImportedDependencyCandidateError> {
        self.imported_member_candidates_for_receiver(receiver, Some(receiver), lookup)
    }

    pub(crate) fn imported_member_candidates_for_receiver(
        &mut self,
        receiver: hir::TypeId,
        access_receiver: Option<hir::TypeId>,
        lookup: hir::ImportedMemberLookup<'_>,
    ) -> Result<Vec<hir::ImportedCallableDeclaration>, hir::ImportedDependencyCandidateError> {
        if self.dependencies.is_none() {
            return Ok(Vec::new());
        }
        let effective = self
            .dependency_interface_definition(receiver)
            .map(|interface| {
                interface
                    .methods
                    .iter()
                    .map(|method| method.declaration.declaration())
                    .collect::<std::collections::BTreeSet<_>>()
            });
        let mut pending = vec![receiver];
        let mut seen = std::collections::BTreeSet::new();
        let mut suppressed_slots = std::collections::BTreeSet::new();
        let mut selected_sources = SelectedInterfaceSources::new();
        let mut result = Vec::<(hir::TypeId, hir::ImportedCallableDeclaration)>::new();
        while let Some(ty) = pending.pop() {
            if !seen.insert(ty) {
                continue;
            }
            self.record_selected_interface_sources(ty, &mut selected_sources);
            let kind = self.types[ty].clone();
            if self.element_encoding_parent(&kind).is_some()
                && let Some((encoding, _)) = self.element_encoding_for_type(&kind)
            {
                suppress_local_implementations(
                    std::slice::from_ref(&encoding.implementation),
                    &mut suppressed_slots,
                );
            }
            match &self.types[ty] {
                hir::Type::Class(application) => {
                    let template = self.class_applications[*application].template;
                    if let Some(class) = self.source_class_id(template) {
                        for function in &self.classes[class].methods {
                            if let Some(method) = self.functions[*function].method {
                                let family = match method.dispatch {
                                    hir::MethodDispatch::Virtual(family)
                                    | hir::MethodDispatch::FinalOverride(family) => family,
                                    _ => continue,
                                };
                                if let Some(
                                    crate::persistent_dispatch::VirtualMethodRoot::Imported(record),
                                ) = self.virtual_method_roots.get(&family)
                                {
                                    suppressed_slots.insert(record.id());
                                }
                            }
                        }
                    }
                    suppress_local_implementations(
                        &self.class_definition(template).interface_implementations,
                        &mut suppressed_slots,
                    );
                    pending.extend(self.direct_nominal_supertypes(ty).into_iter().rev());
                }
                hir::Type::Struct(application) => {
                    let declaration =
                        self.struct_definition(self.struct_applications[*application].template);
                    suppress_local_implementations(
                        &declaration.interface_implementations,
                        &mut suppressed_slots,
                    );
                    pending.extend(self.direct_nominal_supertypes(ty).into_iter().rev());
                }
                hir::Type::Enum(application) => {
                    let declaration =
                        self.enum_definition(self.enum_applications[*application].template);
                    suppress_local_implementations(
                        &declaration.interface_implementations,
                        &mut suppressed_slots,
                    );
                    pending.extend(self.direct_nominal_supertypes(ty).into_iter().rev());
                }
                hir::Type::Interface(application) => {
                    let template = self.interface_applications[*application].template;
                    if let Some(id) = self.source_interface_id(template) {
                        for reference in self.interfaces[id]
                            .methods
                            .iter()
                            .flat_map(|method| &self.interface_method_entities[*method].overrides)
                        {
                            if let hir::InterfaceMethodReference::Imported { slot, .. } = reference
                            {
                                suppressed_slots.insert(*slot);
                            }
                        }
                    }
                    pending.extend(self.direct_nominal_supertypes(ty).into_iter().rev());
                }
                hir::Type::Unit
                | hir::Type::Integer(_)
                | hir::Type::Boolean
                | hir::Type::String => {
                    let kind = match self.types[ty] {
                        hir::Type::Integer(kind) => hir::IntrinsicTypeKind::Integer(kind),
                        hir::Type::Unit => hir::IntrinsicTypeKind::Unit,
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
                                .map(|bound| bound.ty()),
                        );
                    }
                }
                _ => {}
            }
            let Some(owner) = self.imported_nominal_owner(ty) else {
                continue;
            };
            if let Some(interface) = self.dependency_interface_definition(ty) {
                suppressed_slots.extend(
                    interface
                        .methods
                        .iter()
                        .flat_map(|method| &method.overrides)
                        .copied(),
                );
            }
            for candidate in self
                .dependencies
                .as_ref()
                .expect("dependency candidates have a catalog")
                .member_callable_candidates(owner, lookup)?
            {
                let declaration = candidate.interface();
                if !self.imported_encoding_member_applies(ty, declaration.declaration()) {
                    continue;
                }
                if !self.imported_callable_is_accessible(declaration, access_receiver) {
                    continue;
                }
                if effective
                    .as_ref()
                    .is_some_and(|methods| !methods.contains(&declaration.declaration()))
                {
                    continue;
                }
                if result.iter().any(|(_, selected)| {
                    let selected_declaration = selected.interface();
                    selected_declaration.declaration() == declaration.declaration()
                        || selected_declaration
                            .slot_relations()
                            .values()
                            .iter()
                            .any(|slot| declaration.slot_relations().values().contains(slot))
                }) {
                    continue;
                }
                result.push((ty, candidate));
            }
        }
        let available = result
            .iter()
            .map(|(_, candidate)| candidate.interface().declaration())
            .collect::<std::collections::BTreeSet<_>>();
        Ok(result
            .into_iter()
            .filter(|(receiver, candidate)| {
                let declaration = candidate.interface().declaration();
                !candidate
                    .interface()
                    .slot_relations()
                    .values()
                    .iter()
                    .any(|slot| {
                        suppressed_slots.contains(slot)
                            || selected_sources
                                .get(&(*receiver, *slot))
                                .is_some_and(|target| {
                                    *target != declaration && available.contains(target)
                                })
                    })
            })
            .map(|(_, candidate)| candidate)
            .collect())
    }
}

fn suppress_local_implementations(
    implementations: &[hir::InterfaceImplementation],
    slots: &mut std::collections::BTreeSet<scoop_identity::PersistentDispatchSlotId>,
) {
    for method in implementations
        .iter()
        .flat_map(|implementation| &implementation.methods)
    {
        if matches!(
            method.target,
            hir::InterfaceImplementationTarget::Method(_)
                | hir::InterfaceImplementationTarget::Abstract(_)
        ) && let hir::InterfaceMethodReference::Imported { slot, .. } = method.member
        {
            slots.insert(slot);
        }
    }
}
