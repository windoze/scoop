use std::collections::{BTreeMap, BTreeSet};

use scoop_identity::{ConeIdentity, SignatureTypeKey};
use scoop_wire::WirePath;

use super::{
    ExternalHirBindingWitnessRole, ExternalHirBindingWitnessUse,
    ExternalHirReferenceProductionError,
};
use crate::{
    CanonicalDependencyBindingWitnessesV1, CanonicalExternalHirReferenceRolesV1,
    CanonicalExternalHirReferencesV1, DependencyBindingWitnessV1, ExternalHirReferenceRoleV1,
    ExternalHirReferenceSemanticAuthority, ExternalHirReferenceV1, ExternalHirTargetV1,
    cross_cone_interface::SignatureNominalWalker,
};

mod declaration_types;
mod finish;
mod type_sites;

struct PendingReference<'a> {
    origin: ConeIdentity,
    roles: BTreeSet<ExternalHirReferenceRoleV1>,
    witnessed_roles: BTreeSet<ExternalHirReferenceRoleV1>,
    witnesses: BTreeSet<DependencyBindingWitnessV1>,
    call_sites: Vec<super::calls::PendingCallSite<'a>>,
    type_sites: Vec<crate::HirDependencyTypeSiteV1>,
}

impl PendingReference<'_> {
    fn new(origin: ConeIdentity, role: ExternalHirReferenceRoleV1) -> Self {
        Self {
            origin,
            roles: BTreeSet::from([role]),
            witnessed_roles: BTreeSet::new(),
            witnesses: BTreeSet::new(),
            call_sites: Vec::new(),
            type_sites: Vec::new(),
        }
    }
}

pub(super) struct ExternalReferenceAccumulator<'authority, A> {
    current: ConeIdentity,
    authority: &'authority mut A,
    origins: BTreeMap<ExternalHirTargetV1, ConeIdentity>,
    references: BTreeMap<ExternalHirTargetV1, PendingReference<'authority>>,
}

impl<'authority, A> ExternalReferenceAccumulator<'authority, A> {
    pub(super) fn new<E>(authority: &'authority mut A) -> Self
    where
        A: ExternalHirReferenceSemanticAuthority<E>,
    {
        Self {
            current: authority.current_cone(),
            authority,
            origins: BTreeMap::new(),
            references: BTreeMap::new(),
        }
    }

    pub(super) fn authority(&self) -> &A {
        self.authority
    }

    pub(super) fn observe<E>(
        &mut self,
        target: ExternalHirTargetV1,
        role: ExternalHirReferenceRoleV1,
    ) -> Result<bool, ExternalHirReferenceProductionError<E>>
    where
        A: ExternalHirReferenceSemanticAuthority<E>,
    {
        Ok(self.observe_pending(target, role)?.is_some())
    }

    pub(super) fn observe_signature<E>(
        &mut self,
        signature: &SignatureTypeKey,
        role: ExternalHirReferenceRoleV1,

        path: &WirePath,
    ) -> Result<(), ExternalHirReferenceProductionError<E>>
    where
        A: ExternalHirReferenceSemanticAuthority<E>,
    {
        let mut walker = SignatureNominalWalker::new(signature, path)
            .map_err(ExternalHirReferenceProductionError::Resource)?;
        while let Some(declaration) = walker
            .next(path)
            .map_err(ExternalHirReferenceProductionError::Resource)?
        {
            self.observe(ExternalHirTargetV1::from(declaration), role)?;
        }
        Ok(())
    }

    pub(super) fn add_reexport_witness<E>(
        &mut self,
        target: ExternalHirTargetV1,
        witness: DependencyBindingWitnessV1,
    ) -> Result<bool, ExternalHirReferenceProductionError<E>>
    where
        A: ExternalHirReferenceSemanticAuthority<E>,
    {
        let Some(pending) =
            self.observe_pending(target, ExternalHirReferenceRoleV1::ReexportTarget)?
        else {
            return Ok(false);
        };
        pending.witnesses.insert(witness);
        Ok(true)
    }

    pub(super) fn add_witness_use<E>(
        &mut self,
        use_: &ExternalHirBindingWitnessUse,
    ) -> Result<(), ExternalHirReferenceProductionError<E>>
    where
        A: ExternalHirReferenceSemanticAuthority<E>,
    {
        let target = use_.target();
        let role = use_.role().reference_role();
        let external = if use_.role() == ExternalHirBindingWitnessRole::ConcreteSelectedUse {
            self.observe_pending(target, role)?.is_some()
        } else {
            let origin = self.target_origin(target, role)?;
            if origin == self.current {
                false
            } else {
                self.references
                    .get(&target)
                    .is_some_and(|pending| pending.roles.contains(&role))
            }
        };
        if !external {
            let origin = self.target_origin(target, role)?;
            return Err(if origin == self.current {
                ExternalHirReferenceProductionError::CurrentWitnessTarget { target, role }
            } else {
                ExternalHirReferenceProductionError::UnexpectedWitnessUse { target, role }
            });
        }

        let Some(pending) = self.references.get_mut(&target) else {
            return Err(ExternalHirReferenceProductionError::UnexpectedWitnessUse { target, role });
        };
        pending.witnessed_roles.insert(role);
        pending.witnesses.insert(use_.witness().clone());
        Ok(())
    }

    pub(super) fn add_call_sites<E>(
        &mut self,
        output: &'authority crate::DependencyHirOutput,
    ) -> Result<(), ExternalHirReferenceProductionError<E>>
    where
        A: ExternalHirReferenceSemanticAuthority<E>,
    {
        use ExternalHirReferenceProductionError as Error;
        let role = ExternalHirReferenceRoleV1::ConcreteSelectedUse;
        let path = WirePath::root();
        for call in output
            .committed_dependency_call_occurrences()
            .map_err(Error::CallOccurrences)?
        {
            let target = ExternalHirTargetV1::Callable(call.callable().interface().declaration());
            let projected = super::calls::project(output, call)?;
            let pending = self
                .observe_pending(target, role)?
                .ok_or(Error::CurrentWitnessTarget { target, role })?;
            if let Some(binding) = call.binding() {
                for source in binding.sources() {
                    pending
                        .witnesses
                        .insert(source.witness().dependency().clone());
                }
            }

            scoop_wire::allocation::try_reserve(&mut pending.call_sites, 1, &path)
                .map_err(Error::Resource)?;
            pending.call_sites.push(projected);
        }
        Ok(())
    }

    fn target_origin<E>(
        &mut self,
        target: ExternalHirTargetV1,
        role: ExternalHirReferenceRoleV1,
    ) -> Result<ConeIdentity, ExternalHirReferenceProductionError<E>>
    where
        A: ExternalHirReferenceSemanticAuthority<E>,
    {
        if let Some(origin) = self.origins.get(&target) {
            return Ok(*origin);
        }
        let origin = self
            .authority
            .external_hir_target_origin(target)
            .map_err(|source| ExternalHirReferenceProductionError::TargetOrigin {
                target,
                role,
                source,
            })?;
        self.origins.insert(target, origin);
        Ok(origin)
    }

    fn observe_pending<E>(
        &mut self,
        target: ExternalHirTargetV1,
        role: ExternalHirReferenceRoleV1,
    ) -> Result<Option<&mut PendingReference<'authority>>, ExternalHirReferenceProductionError<E>>
    where
        A: ExternalHirReferenceSemanticAuthority<E>,
    {
        let origin = self.target_origin(target, role)?;
        if origin == self.current {
            return Ok(None);
        }
        let pending = self
            .references
            .entry(target)
            .or_insert_with(|| PendingReference::new(origin, role));
        pending.roles.insert(role);
        Ok(Some(pending))
    }
}
