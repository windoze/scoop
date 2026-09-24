//! Types attached to the actual materialized declarations, including unread locals.

use super::ExternalHirReferenceProductionError as Error;
use crate::concrete::{FunctionKind, Module, PropertyStorageOwner, TypeId};
use crate::{
    HirCallableTypePositionV1 as Part, HirDependencyTypePositionV1 as Position,
    HirDependencyTypeSiteV1 as Site,
};
use scoop_identity::{CallableMaterialization, PersistentExactTypeId, PersistentLocalValueId};
use scoop_wire::{BudgetMeter, WireError, WireErrorKind, WirePath};
use std::collections::BTreeMap;

mod constructors;
mod storage;

pub(super) fn collect<E>(
    local: &crate::LocalConcreteHirOutput,
    meter: &mut BudgetMeter,
) -> Result<Vec<Site>, Error<E>> {
    let module = local.module();
    let mut output = Collector {
        module,
        meter,
        sites: BTreeMap::new(),
    };
    for (id, function) in module.functions.iter() {
        output
            .meter
            .charge_work(1, &WirePath::root())
            .map_err(Error::Resource)?;
        if matches!(function.kind, FunctionKind::Intrinsic(_)) {
            continue;
        }
        let root = function.materialization;
        let receiver = function.receiver.value_type();
        if let Some(ty) = receiver {
            output.signature(root, Part::Receiver, ty)?;
        }
        for (index, parameter) in function
            .params
            .iter()
            .skip(usize::from(receiver.is_some()))
            .enumerate()
        {
            output.signature(root, Part::Parameter(parameter_index(index)?), parameter.ty)?;
        }
        output.signature(root, Part::Result, function.return_ty)?;
        if let FunctionKind::User(body) = &function.kind {
            output
                .meter
                .charge_work(body.locals.len() as u64, &WirePath::root())
                .map_err(Error::Resource)?;
            for (local_id, local) in body.locals.iter() {
                if matches!(local.definition, crate::LocalValueDefinitionSite::Source(_)) {
                    output.local(
                        module
                            .local_value_identities
                            .function_local(id, local_id)
                            .id(),
                        local.ty,
                    )?;
                }
            }
        }
    }
    constructors::collect(&mut output)?;
    storage::collect(local, &mut output)?;
    for (_, global) in module.globals.iter() {
        output.add(global.ty, |exact| match global.storage_owner {
            PropertyStorageOwner::Backing(property) => Site::BackingStorage { property, exact },
            PropertyStorageOwner::Delegate(property) => Site::DelegateStorage { property, exact },
        })?;
    }
    let mut sites = Vec::new();
    output
        .meter
        .try_reserve_collection_slots(&mut sites, output.sites.len(), &WirePath::root())
        .map_err(Error::Resource)?;
    sites.extend(output.sites.into_values());
    Ok(sites)
}

struct Collector<'a> {
    module: &'a Module,
    meter: &'a mut BudgetMeter,
    sites: BTreeMap<Position, Site>,
}

impl Collector<'_> {
    fn signature<E>(
        &mut self,
        root: CallableMaterialization,
        position: Part,
        ty: TypeId,
    ) -> Result<(), Error<E>> {
        self.add(ty, |exact| Site::CallableSignature {
            root,
            position,
            exact,
        })
    }

    fn local<E>(&mut self, local: PersistentLocalValueId, ty: TypeId) -> Result<(), Error<E>> {
        self.add(ty, |exact| Site::LocalValue { local, exact })
    }

    fn add<E>(
        &mut self,
        ty: TypeId,
        make: impl FnOnce(PersistentExactTypeId) -> Site,
    ) -> Result<(), Error<E>> {
        let path = WirePath::root();
        self.meter
            .charge_work(1 + u64::from(self.sites.len().max(1).ilog2()), &path)
            .map_err(Error::Resource)?;
        let exact = self
            .module
            .exact_type_identities
            .get(ty)
            .ok_or(Error::DeclarationType(ty))?
            .id();
        let site = make(exact);
        let position = site.position();
        if let Some(previous) = self.sites.get(&position) {
            return if previous == &site {
                Ok(())
            } else {
                Err(Error::ConflictingDeclarationType(position))
            };
        }
        self.meter
            .charge_owned_bytes((std::mem::size_of::<(Position, Site)>() + 32) as u64, &path)
            .map_err(Error::Resource)?;
        self.meter
            .charge_collection_slots(1, &path)
            .map_err(Error::Resource)?;
        self.sites.insert(position, site);
        Ok(())
    }
}

fn parameter_index<E>(index: usize) -> Result<u32, Error<E>> {
    u32::try_from(index).map_err(|_| {
        Error::Resource(WireError::new(
            WireErrorKind::IntegerOutOfRange,
            WirePath::root(),
            None,
        ))
    })
}
