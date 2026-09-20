use super::*;
use std::collections::BTreeMap;

mod merge;
mod targets;

type Selection = InheritanceSourceSlotSelectionV1;
type Selections = BTreeMap<PersistentDispatchSlotId, Selection>;

pub(in crate::production::type_semantics) fn project(
    export: &ExportHir,
    nominals: &[ConcreteNominal<'_>],
    meter: &mut BudgetMeter,
) -> Result<CanonicalInheritanceSourceSlotSelectionsV1, Error> {
    let mut records = Vec::new();
    for nominal in nominals {
        let mut projection = Projection {
            export,
            owner: nominal.exact,
            meter,
        };
        for (slot, selection) in projection.selections(nominal.local)? {
            projection.push(
                &mut records,
                InheritanceSourceSlotSelectionRecordV1::new(nominal.exact, slot, selection),
            )?;
        }
    }
    CanonicalInheritanceSourceSlotSelectionsV1::try_new(records, meter)
        .map_err(Error::SourceInventory)
}

impl Projection<'_, '_> {
    fn selections(&mut self, nominal: NominalLocalId) -> Result<Selections, Error> {
        let mut selections = Selections::new();
        let (implementations, allow_abstract) = match nominal {
            NominalLocalId::Class(id) => {
                self.virtual_selections(id, &mut selections)?;
                let class = &self.export.classes[id];
                (
                    &class.interface_implementations,
                    class.modifier == ClassModifier::Abstract,
                )
            }
            NominalLocalId::Object(id) => {
                let class = self.export.objects[id].backing_class;
                self.virtual_selections(class, &mut selections)?;
                (&self.export.classes[class].interface_implementations, false)
            }
            NominalLocalId::Struct(id) => {
                (&self.export.structs[id].interface_implementations, false)
            }
            NominalLocalId::Enum(id) => (&self.export.enums[id].interface_implementations, false),
            NominalLocalId::Interface(id) => {
                for member in self.interface_members(self.export.interfaces[id].self_application)? {
                    let slot = self.interface_slot(member)?;
                    let selection = self.interface_selection(member)?;
                    self.merge_selection(&mut selections, slot, selection)?;
                }
                return Ok(selections);
            }
        };
        for implementation in implementations {
            self.work(1)?;
            exact(
                self.export,
                self.export.interface_applications[implementation.interface].canonical_type,
            )?;
            for method in &implementation.methods {
                let slot = self.interface_slot(method.member)?;
                let selection = match method.target {
                    InterfaceImplementationTarget::Method(application) => {
                        self.target(application)?
                    }
                    InterfaceImplementationTarget::Subclass if allow_abstract => {
                        Selection::Abstract
                    }
                    InterfaceImplementationTarget::Subclass => {
                        return Err(
                            self.invalid("non-abstract owner leaves an interface slot abstract")
                        );
                    }
                };
                self.merge_selection(&mut selections, slot, selection)?;
            }
        }
        Ok(selections)
    }

    fn virtual_selections(
        &mut self,
        class: ClassId,
        selections: &mut Selections,
    ) -> Result<(), Error> {
        // The first encounter in derived-to-base order is the selected override.
        for class in self.class_chain(class)? {
            for function in &self.export.classes[class].methods {
                self.work(1)?;
                let method = self.method(*function)?;
                let family = match method.dispatch {
                    MethodDispatch::Virtual(family) | MethodDispatch::FinalOverride(family) => {
                        family
                    }
                    MethodDispatch::Direct => continue,
                    MethodDispatch::Interface(_) => {
                        return Err(
                            self.invalid("class method carries interface declaration dispatch")
                        );
                    }
                };
                let slot = self
                    .export
                    .dispatch_slot_identities
                    .get_virtual(family)
                    .ok_or_else(|| self.invalid("virtual family has no sealed dispatch identity"))?
                    .id();
                self.search(selections.len())?;
                if !selections.contains_key(&slot) {
                    let selection = if method.modifier == MethodModifier::Abstract {
                        Selection::Abstract
                    } else {
                        Selection::Concrete(self.callable(*function)?)
                    };
                    self.merge_selection(selections, slot, selection)?;
                }
            }
        }
        Ok(())
    }

    fn merge_selection(
        &mut self,
        selections: &mut Selections,
        slot: PersistentDispatchSlotId,
        selection: Selection,
    ) -> Result<(), Error> {
        merge::insert(self.owner, selections, slot, selection, self.meter)
    }
}
