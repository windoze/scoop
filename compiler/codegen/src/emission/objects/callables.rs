use super::*;
use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

pub(super) struct PreparedCallableObject {
    pub bytes: Vec<u8>,
    pub plan: CallableCodePlan,
    pub backend_constants: BTreeSet<scoop_lir::CallableConstantKind>,
}

type CallableResult =
    Result<(scoop_lir::PersistentCallableBodyId, PreparedCallableObject), CodegenError>;
type WorkerResult = Result<Vec<(usize, CallableResult)>, CodegenError>;

pub(super) fn emit_callables<D: Sync, C: Sync, I: Sync>(
    module: &Module,
    production: &scoop_lir::ConeProductionSection<D, C, I>,
    symbol_plans: &crate::symbol_plan::SymbolPlanIndex,
    declarations: &ConeDeclarations,
    mut safepoints: BTreeMap<String, statepoint::ExpectedSafepoints>,
    profile: ValidatedBackendProfile,
    jobs: NonZeroUsize,
) -> Result<BTreeMap<scoop_lir::PersistentCallableBodyId, PreparedCallableObject>, CodegenError> {
    let functions = module
        .callable_bodies()
        .map(|function| {
            let selected = safepoints
                .remove(function.symbol())
                .expect("the manifest collects every callable body");
            (function, selected)
        })
        .collect::<Vec<_>>();
    if functions.is_empty() {
        return Ok(BTreeMap::new());
    }
    let definitions = production
        .canonical_definitions()
        .plans()
        .iter()
        .filter(|plan| plan.definition_role() == scoop_lir::StrongDefinitionRole::CallableBody)
        .map(|plan| {
            let scoop_lir::StrongDefinitionEntityKind::CallableBody(body) = plan.owner().kind()
            else {
                unreachable!("callable definition has a typed body owner");
            };
            (body, plan)
        })
        .collect::<BTreeMap<_, _>>();
    let next = AtomicUsize::new(0);
    let stopped = AtomicBool::new(false);
    let emit_worker = || -> WorkerResult {
        let machine = profile.create_target_machine()?;
        let context = Context::create();
        let mut completed = Vec::new();
        while !stopped.load(Ordering::Relaxed) {
            let index = next.fetch_add(1, Ordering::Relaxed);
            let Some((function, selected)) = functions.get(index) else {
                break;
            };
            let result: CallableResult = (|| {
                let body = function.callable_body.id();
                let prepared = prepare_callable_strong_llvm_module(
                    &context,
                    module,
                    production,
                    symbol_plans,
                    &machine,
                    profile,
                    selected,
                    function,
                    declarations,
                )?;
                let mut bytes = emit_object(&machine, &prepared.llvm)?;
                if profile.lir_target_profile().native_object_format()
                    == scoop_lir::NativeObjectFormat::Coff64
                {
                    crate::coff_object::merge_callable_constants(&mut bytes)?;
                }
                let backend_constants = crate::callable_constants::present(
                    &bytes,
                    profile.lir_target_profile(),
                    definitions[&body],
                )?;
                Ok((
                    body,
                    PreparedCallableObject {
                        bytes,
                        plan: prepared.plan,
                        backend_constants,
                    },
                ))
            })();
            if result.is_err() {
                stopped.store(true, Ordering::Relaxed);
            }
            completed.push((index, result));
        }
        Ok(completed)
    };
    let workers = jobs.get().min(functions.len());
    std::thread::scope(|scope| {
        let mut handles = Vec::with_capacity(workers - 1);
        let mut spawn_error = None;
        for index in 1..workers {
            match std::thread::Builder::new()
                .name(format!("scoop-codegen-{index}"))
                .spawn_scoped(scope, emit_worker)
            {
                Ok(handle) => handles.push(handle),
                Err(error) => {
                    stopped.store(true, Ordering::Relaxed);
                    spawn_error = Some(CodegenError(format!(
                        "cannot start LLVM object emission worker: {error}"
                    )));
                    break;
                }
            }
        }
        // The calling thread participates in the same worker limit. Join every
        // started worker before selecting an error or finalizing production.
        let mut batches = vec![emit_worker()];
        for handle in handles {
            batches.push(handle.join().unwrap_or_else(|_| {
                Err(CodegenError("LLVM object emission worker panicked".into()))
            }));
        }
        if let Some(error) = spawn_error {
            return Err(error);
        }
        let mut completed = Vec::new();
        for batch in batches {
            completed.extend(batch?);
        }
        completed.sort_unstable_by_key(|(index, _)| *index);
        completed.into_iter().map(|(_, result)| result).collect()
    })
}
