use std::collections::{BTreeMap, BTreeSet};

use scoop_identity::{ConeIdentity, SignatureTypeKey};
use scoop_wire::{BudgetMeter, WirePath};

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

struct PendingReference {
    origin: ConeIdentity,
    roles: BTreeSet<ExternalHirReferenceRoleV1>,
    witnessed_roles: BTreeSet<ExternalHirReferenceRoleV1>,
    witnesses: BTreeSet<DependencyBindingWitnessV1>,
}

impl PendingReference {
    fn new(origin: ConeIdentity, role: ExternalHirReferenceRoleV1) -> Self {
        Self {
            origin,
            roles: BTreeSet::from([role]),
            witnessed_roles: BTreeSet::new(),
            witnesses: BTreeSet::new(),
        }
    }
}

pub(super) struct ExternalReferenceAccumulator<'authority, A> {
    current: ConeIdentity,
    authority: &'authority mut A,
    origins: BTreeMap<ExternalHirTargetV1, ConeIdentity>,
    references: BTreeMap<ExternalHirTargetV1, PendingReference>,
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
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), ExternalHirReferenceProductionError<E>>
    where
        A: ExternalHirReferenceSemanticAuthority<E>,
    {
        let mut walker = SignatureNominalWalker::new(signature, meter, path)
            .map_err(ExternalHirReferenceProductionError::Resource)?;
        while let Some(declaration) = walker
            .next(meter, path)
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

    pub(super) fn add_implicit_core_witnesses(
        &mut self,
        core: &crate::SelectedImportedCoreSet<'_>,
    ) {
        let required = self
            .references
            .iter()
            .filter(|(_, pending)| pending.origin == ConeIdentity::CORE)
            .filter_map(|(&target, pending)| {
                pending
                    .roles
                    .contains(&ExternalHirReferenceRoleV1::DefaultDependency)
                    .then_some((target, ExternalHirReferenceRoleV1::DefaultDependency))
            })
            .collect::<Vec<_>>();
        for (target, role) in required {
            let Some(witness) = core.implicit_binding_witness(target) else {
                continue;
            };
            let pending = self
                .references
                .get_mut(&target)
                .expect("an observed core target remains in the accumulator");
            pending.witnessed_roles.insert(role);
            pending.witnesses.insert(witness);
        }
    }

    pub(super) fn finish<E>(
        self,
    ) -> Result<CanonicalExternalHirReferencesV1, ExternalHirReferenceProductionError<E>> {
        let mut records = Vec::with_capacity(self.references.len());
        for (target, pending) in self.references {
            for role in [
                ExternalHirReferenceRoleV1::AliasTarget,
                ExternalHirReferenceRoleV1::DefaultDependency,
                ExternalHirReferenceRoleV1::ConcreteSelectedUse,
            ] {
                if pending.roles.contains(&role) && !pending.witnessed_roles.contains(&role) {
                    return Err(ExternalHirReferenceProductionError::MissingWitnessUse {
                        target,
                        role,
                    });
                }
            }
            let roles =
                CanonicalExternalHirReferenceRolesV1::try_new(pending.roles.into_iter().collect())
                    .map_err(ExternalHirReferenceProductionError::Roles)?;
            let witnesses = CanonicalDependencyBindingWitnessesV1::try_new(
                pending.witnesses.into_iter().collect(),
            )
            .map_err(ExternalHirReferenceProductionError::Witnesses)?;
            records.push(
                ExternalHirReferenceV1::try_new(pending.origin, target, roles, witnesses)
                    .map_err(ExternalHirReferenceProductionError::Record)?,
            );
        }
        CanonicalExternalHirReferencesV1::try_new(records)
            .map_err(ExternalHirReferenceProductionError::Table)
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
    ) -> Result<Option<&mut PendingReference>, ExternalHirReferenceProductionError<E>>
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
