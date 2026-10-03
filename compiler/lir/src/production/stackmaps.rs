//! Complete LIR-side semantic expectations for LLVM stackmap records.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use scoop_identity::{
    ConeIdentity, PersistentCallableBodyId, PersistentSafepointSiteId, SafepointId,
    SafepointSiteRole,
};

use crate::{Function, GcEffect, Instruction, Module, SafepointSiteRef, StatepointLiveSet};

/// Member-independent semantics that one LLVM v3 stackmap record must prove.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StrongSafepointSemanticPlanV1 {
    site: PersistentSafepointSiteId,
    safepoint: SafepointId,
    owner: PersistentCallableBodyId,
    role: SafepointSiteRole,
    root_pair_count: u32,
}

impl StrongSafepointSemanticPlanV1 {
    pub(crate) const fn from_artifact(
        site: PersistentSafepointSiteId,
        safepoint: SafepointId,
        owner: PersistentCallableBodyId,
        role: SafepointSiteRole,
        root_pair_count: u32,
    ) -> Self {
        Self {
            site,
            safepoint,
            owner,
            role,
            root_pair_count,
        }
    }

    pub const fn site(self) -> PersistentSafepointSiteId {
        self.site
    }

    pub const fn safepoint(self) -> SafepointId {
        self.safepoint
    }

    pub const fn owner(self) -> PersistentCallableBodyId {
        self.owner
    }

    pub const fn role(self) -> SafepointSiteRole {
        self.role
    }

    pub const fn root_pair_count(self) -> u32 {
        self.root_pair_count
    }
}

/// Proof that every final LIR safepoint identity has exactly one semantic use.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongSafepointSemanticPlanSetV1 {
    producer: ConeIdentity,
    sites: Vec<StrongSafepointSemanticPlanV1>,
}

impl StrongSafepointSemanticPlanSetV1 {
    pub fn from_module(module: &Module) -> Result<Self, StrongSafepointSemanticPlanError> {
        Self::from_functions(module.cone, &module.functions)
    }

    pub(crate) const fn from_artifact(
        producer: ConeIdentity,
        sites: Vec<StrongSafepointSemanticPlanV1>,
    ) -> Self {
        Self { producer, sites }
    }

    fn from_functions(
        producer: ConeIdentity,
        functions: &[Function],
    ) -> Result<Self, StrongSafepointSemanticPlanError> {
        let mut function_owners = BTreeSet::new();
        let mut sites = BTreeMap::new();
        let mut runtime_ids = BTreeMap::new();
        for (function_index, function) in functions.iter().enumerate() {
            let owner = function.callable_body.id();
            if !function_owners.insert(owner) {
                return Err(StrongSafepointSemanticPlanError::DuplicateCallableBody {
                    function: function_index,
                    owner,
                });
            }
            collect_function_sites(function_index, function, &mut sites, &mut runtime_ids)?;
        }
        Ok(Self {
            producer,
            sites: sites.into_values().collect(),
        })
    }

    pub const fn producer(&self) -> ConeIdentity {
        self.producer
    }

    pub fn sites(&self) -> &[StrongSafepointSemanticPlanV1] {
        &self.sites
    }
}

fn collect_function_sites(
    function_index: usize,
    function: &Function,
    sites: &mut BTreeMap<PersistentSafepointSiteId, StrongSafepointSemanticPlanV1>,
    runtime_ids: &mut BTreeMap<SafepointId, PersistentSafepointSiteId>,
) -> Result<(), StrongSafepointSemanticPlanError> {
    let owner = function.callable_body.id();
    let mut references = BTreeSet::new();
    let mut function_sites = BTreeSet::new();
    for (_, block) in function.blocks.iter() {
        for instruction in &block.instructions {
            let Some((role, reference, root_pair_count)) = stackmap_semantics(instruction)? else {
                continue;
            };
            if function.gc_effect == GcEffect::NoGc {
                return Err(StrongSafepointSemanticPlanError::SafepointInNoGcFunction {
                    function: function_index,
                    reference,
                });
            }
            if !references.insert(reference) {
                return Err(StrongSafepointSemanticPlanError::ReusedFunctionReference {
                    function: function_index,
                    reference,
                });
            }
            let identity = function.safepoints.get(reference).ok_or(
                StrongSafepointSemanticPlanError::MissingIdentity {
                    function: function_index,
                    reference,
                },
            )?;
            if identity.owner() != owner {
                return Err(StrongSafepointSemanticPlanError::OwnerMismatch {
                    function: function_index,
                    site: identity.site_id(),
                    expected: owner,
                    actual: identity.owner(),
                });
            }
            if identity.role() != role {
                return Err(StrongSafepointSemanticPlanError::RoleMismatch {
                    function: function_index,
                    site: identity.site_id(),
                    expected: role,
                    actual: identity.role(),
                });
            }
            let plan = StrongSafepointSemanticPlanV1 {
                site: identity.site_id(),
                safepoint: identity.runtime_id(),
                owner,
                role,
                root_pair_count,
            };
            if sites.insert(plan.site, plan).is_some() {
                return Err(StrongSafepointSemanticPlanError::DuplicateSite(plan.site));
            }
            if let Some(first) = runtime_ids.insert(plan.safepoint, plan.site) {
                return Err(StrongSafepointSemanticPlanError::DuplicateRuntimeId {
                    first,
                    second: plan.site,
                    safepoint: plan.safepoint,
                });
            }
            function_sites.insert(plan.site);
        }
    }
    if let Some(identity) = function
        .safepoints
        .iter()
        .find(|identity| !function_sites.contains(&identity.site_id()))
    {
        return Err(StrongSafepointSemanticPlanError::UnreferencedIdentity {
            function: function_index,
            site: identity.site_id(),
        });
    }
    Ok(())
}

fn stackmap_semantics(
    instruction: &Instruction,
) -> Result<Option<(SafepointSiteRole, SafepointSiteRef, u32)>, StrongSafepointSemanticPlanError> {
    let Some((role, reference)) = instruction.safepoint() else {
        return Ok(None);
    };
    let live = match instruction {
        Instruction::ManagedPoll { site } => Some(&site.live),
        Instruction::Call {
            site: crate::CallSite::Managed(site),
        } => Some(&site.live),
        Instruction::BoxValue { live, .. }
        | Instruction::ArrayAlloc { live, .. }
        | Instruction::ArrayAssembly { live, .. }
        | Instruction::ArrayClone { live, .. } => Some(live),
        Instruction::Invoke {
            site: crate::InvokeSite::Managed(_),
        }
        | Instruction::Call {
            site: crate::CallSite::NativeSafe(_) | crate::CallSite::NativeBorrowed(_),
        }
        | Instruction::NativeGlobalLoad { .. }
        | Instruction::NativeGlobalStore { .. }
        | Instruction::NativeGlobalAddress { .. } => None,
        _ => {
            return Err(
                StrongSafepointSemanticPlanError::UnsupportedSafepointShape { role, reference },
            );
        }
    };
    let root_pair_count = live.map_or(Ok(0), count_managed_leaves)?;
    Ok(Some((role, reference, root_pair_count)))
}

fn count_managed_leaves(live: &StatepointLiveSet) -> Result<u32, StrongSafepointSemanticPlanError> {
    let count = live.as_slice().iter().try_fold(0_usize, |count, value| {
        count.checked_add(value.leaves.as_slice().len())
    });
    count
        .and_then(|count| u32::try_from(count).ok())
        .ok_or(StrongSafepointSemanticPlanError::RootPairCountOverflow)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StrongSafepointSemanticPlanError {
    DuplicateCallableBody {
        function: usize,
        owner: PersistentCallableBodyId,
    },
    MissingIdentity {
        function: usize,
        reference: SafepointSiteRef,
    },
    UnreferencedIdentity {
        function: usize,
        site: PersistentSafepointSiteId,
    },
    ReusedFunctionReference {
        function: usize,
        reference: SafepointSiteRef,
    },
    SafepointInNoGcFunction {
        function: usize,
        reference: SafepointSiteRef,
    },
    OwnerMismatch {
        function: usize,
        site: PersistentSafepointSiteId,
        expected: PersistentCallableBodyId,
        actual: PersistentCallableBodyId,
    },
    RoleMismatch {
        function: usize,
        site: PersistentSafepointSiteId,
        expected: SafepointSiteRole,
        actual: SafepointSiteRole,
    },
    DuplicateSite(PersistentSafepointSiteId),
    DuplicateRuntimeId {
        first: PersistentSafepointSiteId,
        second: PersistentSafepointSiteId,
        safepoint: SafepointId,
    },
    RootPairCountOverflow,
    UnsupportedSafepointShape {
        role: SafepointSiteRole,
        reference: SafepointSiteRef,
    },
}

impl fmt::Display for StrongSafepointSemanticPlanError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid strong safepoint semantic plan: {self:?}"
        )
    }
}

impl std::error::Error for StrongSafepointSemanticPlanError {}

#[cfg(test)]
mod tests;
