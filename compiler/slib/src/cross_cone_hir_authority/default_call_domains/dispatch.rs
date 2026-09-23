use super::*;

enum PhysicalSlot {
    Absent,
    Own(DispatchSlotKey),
    Inherited(PersistentDispatchSlotId),
}

impl Query<'_, '_> {
    pub(super) fn slot_domain(
        &mut self,
        source: Source<'_>,
        direct: &Rc<SourceAccessDomainV1>,
        inherited: &[Inherited<'_>],
        path: &WirePath,
    ) -> Result<Option<Rc<SourceAccessDomainV1>>, Error> {
        let nearest_class = inherited
            .iter()
            .filter(|candidate| candidate.kind == PublicNominalKindV1::Class)
            .min_by_key(|candidate| candidate.distance);
        self.authority
            .meter
            .charge_work(inherited.len() as u64 + 1, path)?;
        let physical = self.physical_slot(source, nearest_class, path)?;
        self.validate_physical_slot(source, &physical, path)?;
        if inherited.is_empty() {
            return Ok(match physical {
                PhysicalSlot::Absent => None,
                PhysicalSlot::Own(_) | PhysicalSlot::Inherited(_) => Some(Rc::clone(direct)),
            });
        }
        let domain = if let Some(nearest) = nearest_class
            && source.declaration.declared_visibility() == DeclaredVisibilityV1::Protected
            && nearest.source.declaration.declared_visibility() == DeclaredVisibilityV1::Protected
        {
            self.resolve(nearest.source.declaration.declaration(), path)?
                .slot
                .clone()
                .ok_or(Error::PhysicalSlot)?
        } else {
            let domain = self.authority.source_declared_access_domain(
                source.subject,
                source.key,
                source.declaration.declared_visibility(),
                path,
            )?;
            self.share_domain(domain, path)?
        };
        for candidate in inherited {
            let declaration = candidate.source.declaration.declaration();
            let required = self.resolve(declaration, path)?;
            let required = required
                .slot
                .as_ref()
                .ok_or(Error::FinalOverride(declaration))?;
            if !self
                .authority
                .source_domain_is_subset(required, &domain, path)?
            {
                return Err(Error::SlotCoverage(declaration));
            }
        }
        Ok(Some(domain))
    }

    fn physical_slot(
        &mut self,
        source: Source<'_>,
        nearest_class: Option<&Inherited<'_>>,
        path: &WirePath,
    ) -> Result<PhysicalSlot, Error> {
        let declaration = source.declaration;
        let (CallableTemplateOrigin::Function(function), PublicDeclarationOwnerV1::Nominal(owner)) =
            (declaration.declaration(), declaration.owner())
        else {
            return Ok(PhysicalSlot::Absent);
        };
        let (owner, _) = self.authority.visibility_nominal(owner)?;
        let slot = match owner.kind() {
            PublicNominalKindV1::Interface => {
                if declaration.declared_visibility() == DeclaredVisibilityV1::Private {
                    if declaration.modality() != CallableModalityV1::Final {
                        return Err(Error::PhysicalSlot);
                    }
                    PhysicalSlot::Absent
                } else {
                    if !matches!(
                        declaration.modality(),
                        CallableModalityV1::Abstract | CallableModalityV1::InterfaceDefault
                    ) {
                        return Err(Error::PhysicalSlot);
                    }
                    PhysicalSlot::Own(DispatchSlotKey::interface_method(function))
                }
            }
            PublicNominalKindV1::Class | PublicNominalKindV1::Object => {
                if let Some(nearest) = nearest_class {
                    let inherited = nearest.source.declaration;
                    if inherited.modality() == CallableModalityV1::Final {
                        return Err(Error::FinalOverride(inherited.declaration()));
                    }
                    let [slot] = inherited.slot_relations().values() else {
                        return Err(Error::PhysicalSlot);
                    };
                    PhysicalSlot::Inherited(*slot)
                } else {
                    match declaration.modality() {
                        CallableModalityV1::Final => PhysicalSlot::Absent,
                        CallableModalityV1::Open | CallableModalityV1::Abstract => {
                            PhysicalSlot::Own(DispatchSlotKey::virtual_method(function))
                        }
                        CallableModalityV1::InterfaceDefault => return Err(Error::PhysicalSlot),
                    }
                }
            }
            PublicNominalKindV1::Struct | PublicNominalKindV1::Enum => {
                if declaration.modality() != CallableModalityV1::Final {
                    return Err(Error::PhysicalSlot);
                }
                PhysicalSlot::Absent
            }
        };
        self.authority.meter.charge_work(1, path)?;
        Ok(slot)
    }

    fn validate_physical_slot(
        &mut self,
        source: Source<'_>,
        expected: &PhysicalSlot,
        path: &WirePath,
    ) -> Result<(), Error> {
        let slots = source.declaration.slot_relations().values();
        match (slots, expected) {
            ([], PhysicalSlot::Absent) => Ok(()),
            ([actual], PhysicalSlot::Own(expected)) => {
                if self.slot_key(*actual, path)? == *expected {
                    Ok(())
                } else {
                    Err(Error::PhysicalSlot)
                }
            }
            ([actual], PhysicalSlot::Inherited(expected)) if actual == expected => {
                if self.slot_key(*actual, path)?.role()
                    == scoop_identity::DispatchRole::VirtualMethod
                {
                    Ok(())
                } else {
                    Err(Error::PhysicalSlot)
                }
            }
            _ => Err(Error::PhysicalSlot),
        }
    }
}
