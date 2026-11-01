//! Transfer disjoint metadata subtrees to independent LLVM contexts.

use super::*;
use crate::{ScoopLirObjectKindV1, ScoopLirObjectPartitionV1, ScoopLirObjectUnitSetV1};
use inkwell::context::Context;
use inkwell::memory_buffer::MemoryBuffer;
use std::collections::BTreeMap;
use std::num::NonZeroUsize;

struct PreparedGroup<'units> {
    units: &'units [&'units ScoopLirObjectUnitSetV1],
    module_name: String,
    bitcode: Vec<u8>,
    definitions: Vec<index::MetadataDefinition>,
}

impl MetadataModule<'_> {
    pub(in crate::emission) fn map_partitions_parallel<T: Send>(
        self,
        machine: &TargetMachine,
        profile: ValidatedBackendProfile,
        partition: &ScoopLirObjectPartitionV1,
        jobs: NonZeroUsize,
        emit: impl for<'worker> Fn(
            &ScoopLirObjectUnitSetV1,
            Module<'worker>,
            &TargetMachine,
        ) -> Result<T, CodegenError>
        + Sync,
    ) -> Result<BTreeMap<ObjectDefinitionPlanId, T>, CodegenError> {
        let units = partition
            .objects()
            .iter()
            .filter(|units| units.kind() == ScoopLirObjectKindV1::NonCallable)
            .collect::<Vec<_>>();
        let workers = jobs.get().min(units.len());
        if workers <= 1 {
            return self.map_partitions(machine, profile, partition, |units, llvm| {
                emit(units, llvm, machine)
            });
        }
        let mut groups = Vec::with_capacity(workers);
        if let Err(error) = self.prepare_groups(machine, profile, &units, workers, &mut groups) {
            // Earlier subtrees still need to finish so their errors retain priority.
            groups.push(Err(error));
        }
        std::thread::scope(|scope| {
            let emit = &emit;
            let mut groups = groups.into_iter();
            let first = groups
                .next()
                .expect("nonempty metadata has a prepared group");
            let mut handles = Vec::with_capacity(workers - 1);
            let mut spawn_error = None;
            for (index, group) in groups.enumerate() {
                match std::thread::Builder::new()
                    .name(format!("scoop-metadata-{}", index + 1))
                    .spawn_scoped(scope, move || emit_group(group, profile, emit))
                {
                    Ok(handle) => handles.push(handle),
                    Err(error) => {
                        spawn_error = Some(CodegenError(format!(
                            "cannot start LLVM metadata worker: {error}"
                        )));
                        break;
                    }
                }
            }
            let mut batches = vec![emit_group(first, profile, emit)];
            for handle in handles {
                batches.push(
                    handle.join().unwrap_or_else(|_| {
                        Err(CodegenError("LLVM metadata worker panicked".into()))
                    }),
                );
            }
            if let Some(error) = spawn_error {
                return Err(error);
            }
            let mut output = BTreeMap::new();
            for batch in batches {
                output.extend(batch?);
            }
            Ok(output)
        })
    }

    fn prepare_groups<'units>(
        self,
        machine: &TargetMachine,
        profile: ValidatedBackendProfile,
        units: &'units [&'units ScoopLirObjectUnitSetV1],
        workers: usize,
        groups: &mut Vec<Result<PreparedGroup<'units>, CodegenError>>,
    ) -> Result<(), CodegenError> {
        if workers == 1 {
            let module_name = self
                .llvm
                .get_name()
                .to_str()
                .expect("Scoop metadata module names are UTF-8")
                .to_owned();
            groups.push(Ok(PreparedGroup {
                units,
                module_name,
                bitcode: self.llvm.write_bitcode_to_memory().as_slice().to_vec(),
                definitions: self.definitions,
            }));
            return Ok(());
        }
        let (left, right) = units.split_at(units.len().div_ceil(2));
        for (group, workers) in [(left, workers.div_ceil(2)), (right, workers / 2)] {
            let mut plans = group
                .iter()
                .flat_map(|units| units.definition_plans().iter().copied())
                .collect::<Vec<_>>();
            plans.sort_unstable();
            let llvm = self.project(machine, profile, &plans)?;
            let selected = plans.into_iter().collect::<BTreeSet<_>>();
            let definitions = self
                .definitions
                .iter()
                .filter(|entry| selected.contains(&entry.plan.definition_plan()))
                .cloned()
                .collect();
            Self { llvm, definitions }.prepare_groups(machine, profile, group, workers, groups)?;
        }
        Ok(())
    }
}

fn emit_group<T>(
    group: Result<PreparedGroup<'_>, CodegenError>,
    profile: ValidatedBackendProfile,
    emit: &impl for<'worker> Fn(
        &ScoopLirObjectUnitSetV1,
        Module<'worker>,
        &TargetMachine,
    ) -> Result<T, CodegenError>,
) -> Result<BTreeMap<ObjectDefinitionPlanId, T>, CodegenError> {
    let group = group?;
    // Target machines can retain metadata, so they must die before the context.
    let context = Context::create();
    let machine = profile.create_target_machine()?;
    let buffer = MemoryBuffer::create_from_memory_range(&group.bitcode, &group.module_name);
    let llvm = Module::parse_bitcode_from_buffer(&buffer, &context)
        .map_err(|error| CodegenError(format!("cannot load metadata subtree: {error}")))?;
    llvm.set_name(&group.module_name);
    let metadata = MetadataModule {
        llvm,
        definitions: group.definitions,
    };
    let mut output = BTreeMap::new();
    if let [unit] = group.units {
        // The parent already projected this leaf before transferring it.
        output.insert(
            unit.definition_plans()[0],
            emit(unit, metadata.llvm, &machine)?,
        );
    } else {
        metadata.visit(&machine, profile, group.units, &mut |units, llvm| {
            output.insert(units.definition_plans()[0], emit(units, llvm, &machine)?);
            Ok(())
        })?;
    }
    Ok(output)
}
