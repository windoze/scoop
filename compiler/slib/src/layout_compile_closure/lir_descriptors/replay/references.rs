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
    ) -> Result<Self, Error> {
        if inputs.layouts.provider() != provider || inputs.dispatch.provider() != provider {
            return Err(Error::LocalProvider);
        }
        if inputs.layouts.target() != target || inputs.dispatch.target() != target {
            return Err(Error::LocalTarget);
        }
        let mut seen = BTreeSet::new();
        for table in inputs.dependencies {
            let origin = table.provider();
            if origin == provider || seen.contains(&origin) {
                return Err(Error::DependencyProvider(origin));
            }
            if table.target() != target {
                return Err(Error::DependencyTarget(origin));
            }

            seen.insert(origin);
        }
        Ok(Self { types, inputs })
    }

    pub(super) fn get(
        &self,
        exact: PersistentExactTypeId,
    ) -> Result<lir::StrongTypeDescriptorRefV2, Error> {
        let mut found = None;
        if self.types.get(exact).is_some_and(|ty| {
            !matches!(
                ty.representation(),
                mir::MirTypeRepresentationV1::ObjectBacking { .. }
            )
        }) {
            self.inputs
                .layouts
                .find_exact_role(exact, RepresentationRole::ManagedObject)
                .ok_or(Error::MissingDescriptor(exact))?;
            found = Some(lir::StrongTypeDescriptorRefV2::Local(exact));
        }
        for table in self.inputs.dependencies {
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
