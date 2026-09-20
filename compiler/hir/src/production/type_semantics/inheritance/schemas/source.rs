use super::*;

pub(in crate::production::type_semantics) fn project(
    export: &ExportHir,
    nominals: &[ConcreteNominal<'_>],
    meter: &mut BudgetMeter,
) -> Result<CanonicalInterfaceSourceDispatchesV1, Error> {
    let mut seen = BTreeSet::new();
    let mut records = Vec::new();
    for nominal in nominals {
        let mut projection = Projection {
            export,
            owner: nominal.exact,
            meter,
        };
        for root in projection.roots(nominal.local)? {
            for application in projection.interface_postorder(root)? {
                projection.search(seen.len())?;
                if seen.insert(application) {
                    let record = projection.source(application)?;
                    projection.push(&mut records, record)?;
                }
            }
        }
    }
    CanonicalInterfaceSourceDispatchesV1::try_new(records, meter).map_err(Error::SourceInventory)
}

impl Projection<'_, '_> {
    fn roots(&mut self, nominal: NominalLocalId) -> Result<Vec<InterfaceApplicationId>, Error> {
        let mut types = Vec::new();
        let class = match nominal {
            NominalLocalId::Class(id) => Some(id),
            NominalLocalId::Object(id) => Some(self.export.objects[id].backing_class),
            NominalLocalId::Interface(id) => {
                let mut roots = Vec::new();
                self.push(&mut roots, self.export.interfaces[id].self_application)?;
                return Ok(roots);
            }
            NominalLocalId::Struct(id) => {
                self.extend(&mut types, &self.export.structs[id].interfaces)?;
                None
            }
            NominalLocalId::Enum(id) => {
                self.extend(&mut types, &self.export.enums[id].interfaces)?;
                None
            }
        };
        if let Some(class) = class {
            for id in self.class_chain(class)? {
                self.extend(&mut types, &self.export.classes[id].interfaces)?;
            }
        }
        let mut roots = Vec::new();
        for ty in types {
            let application = self.interface_application(ty)?;
            self.push(&mut roots, application)?;
        }
        Ok(roots)
    }

    fn source(
        &mut self,
        application: InterfaceApplicationId,
    ) -> Result<InterfaceSourceDispatchV1, Error> {
        let application = &self.export.interface_applications[application];
        let owner = exact(self.export, application.canonical_type)?;
        let declaration = &self.export.interfaces[application.template];
        let mut parents = Vec::new();
        for parent in &declaration.parents {
            let exact = exact(
                self.export,
                self.export.interface_applications[*parent].canonical_type,
            )?;
            self.push(&mut parents, exact)?;
        }
        let mut members = Vec::new();
        for member in &declaration.methods {
            let slot = self.export.dispatch_slot_identities[*member].id();
            let mut overrides = Vec::new();
            let mut seen = BTreeSet::new();
            for inherited in &self.export.interface_methods[*member].overrides {
                let inherited = self.export.dispatch_slot_identities[*inherited].id();
                self.search(seen.len())?;
                if seen.insert(inherited) {
                    self.push(&mut overrides, inherited)?;
                }
            }
            self.sort_work(overrides.len())?;
            let overrides = CanonicalPersistentIdsV1::try_new(overrides)
                .map_err(|error| self.invalid(error))?;
            self.push(&mut members, InterfaceSourceMemberV1::new(slot, overrides))?;
        }
        InterfaceSourceDispatchV1::try_new(owner, parents, members, self.meter)
            .map_err(Error::SourceInventory)
    }
}
