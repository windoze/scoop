//! Callable references emitted independently of ordinary function bodies.

use super::mir;

pub(super) fn roots(module: &mir::Module, retain: &mut impl FnMut(mir::FunctionId)) {
    if let mir::MirOutput::Executable { entry, arguments } = module.output {
        retain(entry);
        if let mir::ProgramArguments::Local(builder) = arguments {
            retain(builder);
        }
    }
    for (_, unit) in module.initialization_units.iter() {
        retain(unit.initializer);
        retain(unit.ensure);
    }
    // The callback directories themselves are still emitted, including any
    // FunctionAddress users; keep both the source and native storage bridge.
    for (_, callback) in module.callback_bridges.iter() {
        if let Some((source, bridge)) = callback.local_definition() {
            retain(source);
            retain(bridge);
        }
    }
    for (_, callback) in module.foreign_callback_adapters.iter() {
        retain(callback.function);
    }
    for (_, class) in module.classes.iter() {
        for slot in class
            .vtable
            .iter()
            .chain(class.itables.iter().flat_map(|table| &table.slots))
        {
            if let mir::TableSlot::Function(function) = slot {
                retain(*function);
            }
        }
    }
    for table in &module.meta.dispatch_tables {
        retain(table.owner);
        for &entry in &table.entries {
            retain(entry);
        }
    }
    for (_, coroutine) in module.meta.coroutine_functions.iter() {
        retain(coroutine.function);
        if let mir::CoroutineLowering::StateMachine { driver, .. } = coroutine.lowering {
            retain(driver);
        }
    }
    for (_, point) in module.meta.coroutine_resume_points.iter() {
        retain(point.resume());
        retain(point.resume_with_exception());
    }
    for start in &module.meta.coroutine_starts {
        retain(start.function());
    }
}
