use super::*;
use crate::defaults::DefaultOverrideSource;

impl MemberIndex {
    pub(super) fn source_function(
        &mut self,
        lowerer: &mut Lowerer,
        function: hir::FunctionId,
        check: &impl Fn() -> bool,
    ) -> Result<(), EditingCancelled> {
        if !self.functions.insert(function) {
            return Ok(());
        }
        let Some(method) = lowerer.functions[function].method else {
            return Ok(());
        };
        let origin = lowerer.editing_callable_origin(hir::SemanticCallable::Current(function));
        let Some(member) = self.current_member(lowerer, function) else {
            self.result.unavailable.extend(origin);
            return Ok(());
        };
        if lowerer.invalid_override_methods.contains(&function)
            || lowerer.generation.methods.contains_key(&function)
        {
            self.result.unavailable.extend(origin);
            return Ok(());
        }
        if method.modifier != hir::MethodModifier::Abstract
            && !matches!(
                lowerer.functions[function].kind,
                hir::FunctionKind::Intrinsic(_) | hir::FunctionKind::Extern(_)
            )
        {
            self.implementation(member, origin);
        }
        let sources = lowerer
            .override_default_sources
            .get(&function)
            .cloned()
            .unwrap_or_default();
        for source in sources {
            if check() {
                return Err(EditingCancelled);
            }
            let requirement = match source {
                DefaultOverrideSource::Local { function, .. } => {
                    self.current_member(lowerer, function)
                }
                DefaultOverrideSource::Imported { declaration, .. } => {
                    self.include_imported(lowerer, declaration, check)?
                }
            };
            if let Some(requirement) = requirement {
                self.relation(requirement, member);
            }
        }
        if let hir::MethodDispatch::Virtual(family) | hir::MethodDispatch::FinalOverride(family) =
            method.dispatch
        {
            let requirement = match lowerer.virtual_method_roots[&family].clone() {
                crate::persistent_dispatch::VirtualMethodRoot::Local(root) => {
                    self.current_member(lowerer, root)
                }
                crate::persistent_dispatch::VirtualMethodRoot::Imported(slot) => {
                    self.include_imported(lowerer, slot_origin(slot.key()), check)?
                }
            };
            if let Some(requirement) = requirement {
                self.relation(requirement, member);
            }
        }
        Ok(())
    }

    pub(super) fn source_property(
        &mut self,
        lowerer: &mut Lowerer,
        property: hir::PropertyId,
        check: &impl Fn() -> bool,
    ) -> Result<(), EditingCancelled> {
        if !self.properties.insert(property) {
            return Ok(());
        }
        if matches!(
            lowerer.properties[property].owner,
            hir::PropertyOwner::TopLevel | hir::PropertyOwner::Extension(_)
        ) {
            return Ok(());
        }
        let origin = lowerer.editing_property_origin(hir::SemanticProperty::Current(property));
        let Some(member) = self.current_property(lowerer, property) else {
            self.result.unavailable.extend(origin);
            return Ok(());
        };
        let declaration = lowerer.properties[property].clone();
        let concrete_getter = !matches!(
            lowerer.property_getters[declaration.capability.getter()].implementation,
            hir::PropertyAccessorImplementation::AbstractSlot(_)
        );
        let concrete_setter = declaration.capability.setter().is_some_and(|setter| {
            !matches!(
                lowerer.property_setters[setter].implementation,
                hir::PropertyAccessorImplementation::AbstractSlot(_)
            )
        });
        if concrete_getter || concrete_setter {
            self.implementation(member, origin);
        }
        for inherited in declaration.overrides {
            if check() {
                return Err(EditingCancelled);
            }
            let requirement = match inherited {
                hir::PropertyReference::Local(id) => self.current_property(lowerer, id),
                hir::PropertyReference::Imported { declaration, .. } => {
                    let declaration = hir::PropertyDeclarationId::Property(declaration);
                    self.include_imported_property(lowerer, declaration, check)?;
                    Some(Member::Property(declaration))
                }
            };
            if let Some(requirement) = requirement {
                self.relation(requirement, member);
            }
        }
        Ok(())
    }
}

pub(super) fn slot_origin(slot: &scoop_identity::DispatchSlotKey) -> Imported {
    match slot.owner() {
        scoop_identity::DispatchDeclarationOwner::Function(id) => Imported::Function(id),
        scoop_identity::DispatchDeclarationOwner::Accessor(id) => Imported::Accessor(id),
    }
}
