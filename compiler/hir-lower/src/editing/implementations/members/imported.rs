use super::*;

impl MemberIndex {
    pub(super) fn include_imported(
        &mut self,
        lowerer: &Lowerer,
        origin: Imported,
        check: &impl Fn() -> bool,
    ) -> Result<Option<Member>, EditingCancelled> {
        if check() {
            return Err(EditingCancelled);
        }
        let Some(member) = self.imported_member(lowerer, origin) else {
            return Ok(None);
        };
        if !self.imported.insert(origin) {
            return Ok(Some(member));
        }
        if let Member::Property(property) = member {
            self.include_imported_property(lowerer, property, check)?;
            return Ok(Some(member));
        }
        let dependencies = lowerer
            .dependencies
            .as_ref()
            .expect("imported declaration catalog");
        let declaration = dependencies
            .callable_declaration(origin)
            .expect("checked imported method");
        let callable = declaration.interface();
        if callable.modality() != hir::CallableModalityV1::Abstract
            && callable.effects().implementation() == hir::CallableImplementationV1::Scoop
            && let Member::Callable(callable) = member
        {
            self.implementation(
                member,
                lowerer.editing_callable_origin(hir::SemanticCallable::Imported(callable)),
            );
        }
        let hir::PublicDeclarationOwnerV1::Nominal(owner) = callable.owner() else {
            unreachable!("an imported implementation member has a nominal owner")
        };
        let nominal = dependencies
            .nominal_declaration(owner)
            .expect("imported method owner");
        let slots = callable.slot_relations().values();
        let mut ancestors = Vec::new();
        match nominal.interface.declaration_details().dispatch_order() {
            hir::NominalDispatchOrderV1::Class { .. } => {
                let mut base = lowerer.class_definition(owner).base_class;
                let mut seen = HashSet::new();
                while let Some(ty) = base {
                    if check() {
                        return Err(EditingCancelled);
                    }
                    let hir::Type::Class(application) = lowerer.types[ty] else {
                        unreachable!("checked dependency class ancestry")
                    };
                    let parent = lowerer.class_applications[application].template;
                    if !seen.insert(parent) {
                        break;
                    }
                    for &slot in slots {
                        if let Some(declaration) = dependencies
                            .callable_for_slot(parent, slot)
                            .expect("checked parent slot")
                        {
                            ancestors.push(declaration.interface().declaration());
                        }
                    }
                    base = lowerer.class_definition(parent).base_class;
                }
            }
            hir::NominalDispatchOrderV1::Interface { members, .. } => {
                self.prepare_imported_slots(lowerer, check)?;
                let origins = self.imported_slots.as_ref().expect("prepared slot origins");
                for inherited in members
                    .iter()
                    .filter(|member| slots.contains(&member.slot()))
                {
                    for slot in inherited.overrides().values() {
                        ancestors
                            .push(*origins.get(slot).expect("checked inherited interface slot"));
                    }
                }
            }
            hir::NominalDispatchOrderV1::NonVirtual => {}
        }
        for ancestor in ancestors {
            if let Some(requirement) = self.include_imported(lowerer, ancestor, check)? {
                self.relation(requirement, member);
            }
        }
        Ok(Some(member))
    }

    fn prepare_imported_slots(
        &mut self,
        lowerer: &Lowerer,
        check: &impl Fn() -> bool,
    ) -> Result<(), EditingCancelled> {
        if self.imported_slots.is_some() {
            return Ok(());
        }
        let mut slots = HashMap::new();
        for nominal in lowerer
            .dependencies
            .as_ref()
            .expect("imported catalog")
            .nominal_declarations()
        {
            if check() {
                return Err(EditingCancelled);
            }
            for slot in &nominal.dispatch_slots {
                slots.insert(slot.id(), super::source::slot_origin(slot.key()));
            }
        }
        self.imported_slots = Some(slots);
        Ok(())
    }

    pub(super) fn include_imported_property(
        &mut self,
        lowerer: &Lowerer,
        property: hir::PropertyDeclarationId,
        check: &impl Fn() -> bool,
    ) -> Result<(), EditingCancelled> {
        if check() {
            return Err(EditingCancelled);
        }
        if !self.imported_properties.insert(property) {
            return Ok(());
        }
        let dependencies = lowerer
            .dependencies
            .as_ref()
            .expect("imported property catalog");
        let declaration = dependencies
            .property_declaration(property)
            .expect("checked imported property");
        if !matches!(
            declaration.owner(),
            hir::PublicDeclarationOwnerV1::Nominal(_)
        ) {
            return Ok(());
        }
        let member = Member::Property(property);
        if declaration.accessors().getter_source().implementation()
            != hir::PropertyAccessorImplementationV1::AbstractSlot
            || declaration
                .accessors()
                .setter_source()
                .is_some_and(|setter| {
                    setter.implementation() != hir::PropertyAccessorImplementationV1::AbstractSlot
                })
        {
            self.implementation(
                member,
                lowerer.editing_property_origin(hir::SemanticProperty::Imported(property)),
            );
        }
        for &parent in declaration.overrides() {
            let parent = hir::PropertyDeclarationId::Property(parent);
            self.include_imported_property(lowerer, parent, check)?;
            self.relation(Member::Property(parent), member);
        }
        Ok(())
    }
}
