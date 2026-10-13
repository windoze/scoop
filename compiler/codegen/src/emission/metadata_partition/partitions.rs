//! Project smaller metadata groups before visiting individual physical objects.

use super::*;
use crate::{ScoopLirObjectKindV1, ScoopLirObjectPartitionV1, ScoopLirObjectUnitSetV1};
use std::collections::BTreeMap;

impl<'ctx> MetadataModule<'ctx> {
    pub(in crate::emission) fn map_partitions<T>(
        &self,
        machine: &TargetMachine,
        profile: ValidatedBackendProfile,
        partition: &ScoopLirObjectPartitionV1,
        mut emit: impl FnMut(
            &ScoopLirObjectUnitSetV1,
            Module<'ctx>,
            EmittedStrongRuntimeMetadataV1,
        ) -> Result<T, CodegenError>,
    ) -> Result<BTreeMap<ObjectDefinitionPlanId, T>, CodegenError> {
        let units = partition
            .objects()
            .iter()
            .filter(|units| units.kind() == ScoopLirObjectKindV1::NonCallable)
            .collect::<Vec<_>>();
        let mut output = BTreeMap::new();
        self.visit(machine, profile, &units, &mut |units, llvm, metadata| {
            output.insert(units.definition_plans()[0], emit(units, llvm, metadata)?);
            Ok(())
        })?;
        Ok(output)
    }

    fn visit(
        &self,
        machine: &TargetMachine,
        profile: ValidatedBackendProfile,
        units: &[&ScoopLirObjectUnitSetV1],
        emit: &mut impl FnMut(
            &ScoopLirObjectUnitSetV1,
            Module<'ctx>,
            EmittedStrongRuntimeMetadataV1,
        ) -> Result<(), CodegenError>,
    ) -> Result<(), CodegenError> {
        if units.is_empty() {
            return Ok(());
        }
        let (left, right) = units.split_at(units.len().div_ceil(2));
        for group in [left, right].into_iter().filter(|group| !group.is_empty()) {
            let mut plans = group
                .iter()
                .flat_map(|units| units.definition_plans().iter().copied())
                .collect::<Vec<_>>();
            plans.sort_unstable();
            let (llvm, metadata) = self.project(machine, profile, &plans)?;
            if let [unit] = group {
                emit(unit, llvm, metadata)?;
            } else {
                let selected = plans.into_iter().collect::<BTreeSet<_>>();
                let definitions = self
                    .definitions
                    .iter()
                    .filter(|entry| selected.contains(&entry.plan.definition_plan()))
                    .cloned()
                    .collect();
                Self {
                    llvm,
                    metadata,
                    definitions,
                }
                .visit(machine, profile, group, emit)?;
            }
        }
        Ok(())
    }
}
