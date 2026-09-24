use super::*;
use scoop_identity::{ConeIdentity, PersistentExactTypeId, RepresentationRole};
use std::collections::BTreeSet;

pub(super) struct References<'a> {
    types: &'a mir::CanonicalParamFreeMirTypeExportsV1,
    inputs: SharedLirDescriptorInputsV1<'a>,
}
impl<'a> References<'a> {
    pub(super) fn new(
        target: lir::LirTargetProfile,
        types: &'a mir::CanonicalParamFreeMirTypeExportsV1,
        inputs: SharedLirDescriptorInputsV1<'a>,
        provider: ConeIdentity,
        meter: &mut BudgetMeter,
    ) -> Result<Self, Error> {
        let path = WirePath::root();
        meter.charge_work(1, &path)?;
        if inputs.layouts.provider() != provider || inputs.dispatch.provider() != provider {
            return Err(Error::LocalProvider);
        }
        if inputs.layouts.target() != target || inputs.dispatch.target() != target {
            return Err(Error::LocalTarget);
        }
        let mut seen = BTreeSet::new();
        for table in inputs.dependencies {
            meter.charge_work(u64::from(seen.len().max(1).ilog2()) + 1, &path)?;
            let origin = table.provider();
            if origin == provider || seen.contains(&origin) {
                return Err(Error::DependencyProvider(origin));
            }
            if table.target() != target {
                return Err(Error::DependencyTarget(origin));
            }
            meter.charge_collection_slots(1, &path)?;
            meter.charge_owned_bytes(std::mem::size_of::<ConeIdentity>() as u64, &path)?;
            seen.insert(origin);
        }
        Ok(Self { types, inputs })
    }

    pub(super) fn get(
        &self,
        exact: PersistentExactTypeId,
        meter: &mut BudgetMeter,
    ) -> Result<lir::StrongTypeDescriptorRefV2, Error> {
        let path = WirePath::root();
        meter.charge_work(
            u64::from(self.types.records().len().max(1).ilog2()) + 1,
            &path,
        )?;
        let mut found = None;
        if self.types.get(exact).is_some_and(|ty| {
            !matches!(
                ty.representation(),
                mir::MirTypeRepresentationV1::ObjectBacking { .. }
            )
        }) {
            meter.charge_work(self.inputs.layouts.records().len() as u64, &path)?;
            self.inputs
                .layouts
                .find_exact_role(exact, RepresentationRole::ManagedObject)
                .ok_or(Error::MissingDescriptor(exact))?;
            found = Some(lir::StrongTypeDescriptorRefV2::Local(exact));
        }
        for table in self.inputs.dependencies {
            meter.charge_work(u64::from(table.records().len().max(1).ilog2()) + 1, &path)?;
            if table.get(exact).is_some() {
                let reference = lir::StrongTypeDescriptorRefV2::DependencyExternal {
                    provider: table.provider(),
                    exact,
                };
                if found.replace(reference).is_some() {
                    return Err(Error::DuplicateDescriptor(exact));
                }
            }
        }
        found.ok_or(Error::MissingDescriptor(exact))
    }
}
