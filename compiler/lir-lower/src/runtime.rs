use super::*;

pub(super) fn lower_runtime_function(function: mir::RuntimeFn) -> lir::RuntimeFunction {
    match function {
        mir::RuntimeFn::Box => lir::RuntimeFunction::Managed(lir::ManagedRuntimeFunction::Box),
        mir::RuntimeFn::IsInstance => {
            lir::RuntimeFunction::NoGc(lir::NoGcRuntimeFunction::IsInstance)
        }
        mir::RuntimeFn::ITableLookup => {
            lir::RuntimeFunction::NoGc(lir::NoGcRuntimeFunction::ITableLookup)
        }
        mir::RuntimeFn::Pin => lir::RuntimeFunction::NoGc(lir::NoGcRuntimeFunction::Pin),
        mir::RuntimeFn::Unpin => lir::RuntimeFunction::NoGc(lir::NoGcRuntimeFunction::Unpin),
        mir::RuntimeFn::GetHandle => {
            lir::RuntimeFunction::NoGc(lir::NoGcRuntimeFunction::GetHandle)
        }
        mir::RuntimeFn::ReleaseHandle => {
            lir::RuntimeFunction::NoGc(lir::NoGcRuntimeFunction::ReleaseHandle)
        }
        mir::RuntimeFn::GcCollect => {
            lir::RuntimeFunction::Managed(lir::ManagedRuntimeFunction::GcCollect)
        }
        mir::RuntimeFn::GcStats => lir::RuntimeFunction::NoGc(lir::NoGcRuntimeFunction::GcStats),
        mir::RuntimeFn::MaterializeException => {
            lir::RuntimeFunction::Managed(lir::ManagedRuntimeFunction::MaterializeException)
        }
        mir::RuntimeFn::StringConcat => {
            lir::RuntimeFunction::Managed(lir::ManagedRuntimeFunction::StringConcat)
        }
        mir::RuntimeFn::StringCompare => {
            lir::RuntimeFunction::NoGc(lir::NoGcRuntimeFunction::StringCompare)
        }
        mir::RuntimeFn::Trap => lir::RuntimeFunction::NoGc(lir::NoGcRuntimeFunction::Trap),
    }
}
