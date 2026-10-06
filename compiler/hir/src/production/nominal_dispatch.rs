//! Project dispatch relationships already resolved in the sealed source HIR.

use std::collections::{BTreeMap, BTreeSet};

use scoop_identity::PersistentDispatchSlotId;
use scoop_wire::WirePath;

use super::type_semantics::inheritance::source_errors::{invalid, resource};
use crate::{CrossConeTypeSemanticsProductionError as Error, *};

mod implementations;
mod interfaces;
mod selections;
mod targets;

type Selection = InheritanceSourceSlotSelectionV1;
type SelectionRole = NominalDispatchSelectionRoleV1;
type Selections = BTreeMap<
    (SelectionRole, PersistentDispatchSlotId),
    (Selection, scoop_identity::SignatureTypeKey),
>;

pub(super) fn project(
    export: &ExportHir,
    owner: NominalOwner,
) -> Result<CanonicalNominalDispatchSelectionsV1, Error> {
    let mut projection = Projection::new(export);
    let mut records = Vec::new();
    for ((role, slot), (selection, receiver)) in projection.selections(owner)? {
        projection.push(
            &mut records,
            NominalDispatchSelectionV1::new(role, receiver, slot, selection),
        )?;
    }
    CanonicalNominalDispatchSelectionsV1::try_new(records).map_err(|error| match error {
        NominalDispatchSelectionError::Resource(error) => resource(error),
        error => invalid(error),
    })
}

pub(super) struct Projection<'a> {
    export: &'a ExportHir,
    binders: Vec<HirSignatureBinder>,
}

impl<'a> Projection<'a> {
    pub(super) fn new(export: &'a ExportHir) -> Self {
        Self {
            export,
            binders: Vec::new(),
        }
    }

    fn interface_role(&self, ty: TypeId) -> Result<SelectionRole, Error> {
        let interface = self.type_key(ty)?;
        Ok(SelectionRole::Interface { interface })
    }

    fn type_key(&self, ty: TypeId) -> Result<scoop_identity::SignatureTypeKey, Error> {
        super::signatures::HirInterfaceSignatureProjector::new(self.export)
            .map_type(ty, &self.binders)
            .map_err(invalid)
    }

    fn declaration_parameters(&self, owner: NominalOwner) -> &[TypeParamDecl] {
        match owner {
            NominalOwner::Class(id) => &self.export.classes[id].type_params,
            NominalOwner::Interface(id) => &self.export.interfaces[id].type_params,
            NominalOwner::Struct(id) => &self.export.structs[id].type_params,
            NominalOwner::Enum(id) => &self.export.enums[id].type_params,
            NominalOwner::Object(id) => {
                &self.export.classes[self.export.objects[id].backing_class].type_params
            }
        }
    }

    fn push<T>(&mut self, values: &mut Vec<T>, value: T) -> Result<(), Error> {
        scoop_wire::allocation::try_reserve(values, 1, &WirePath::root()).map_err(resource)?;
        values.push(value);
        Ok(())
    }

    pub(super) fn class_chain(&mut self, class: ClassId) -> Result<Vec<ClassChainEntry>, Error> {
        let mut result = Vec::new();
        let mut seen = BTreeSet::new();
        let mut current = ClassChainEntry::Local(class);
        loop {
            if !seen.insert(current) {
                return Err(invalid("cycle in the source class base chain"));
            }
            self.push(&mut result, current)?;
            let base = match current {
                ClassChainEntry::Local(class) => self.export.classes[class].base_class,
                // The dependency's selected virtual table already includes
                // its entire base chain. Reuse that checked selection.
                ClassChainEntry::Imported(_) => None,
            };
            let Some(base) = base else {
                return Ok(result);
            };
            current = match &self.export.types[base] {
                Type::Class(application) => match self
                    .export
                    .nominal_identities
                    .class_id(self.export.class_applications[*application].template)
                {
                    Some(class) => ClassChainEntry::Local(class),
                    None => ClassChainEntry::Imported(base),
                },
                _ => return Err(invalid("class base does not resolve to a class type")),
            };
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum ClassChainEntry {
    Local(ClassId),
    Imported(TypeId),
}
