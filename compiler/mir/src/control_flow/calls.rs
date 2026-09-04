//! Fully resolved call targets and compiler-runtime destinations.

use super::*;

#[derive(Debug, Clone)]
pub struct Call {
    pub target: CallTarget,
    pub args: Vec<Expr>,
}

#[derive(Debug, Clone)]
pub struct CallTarget {
    pub kind: CallKind,
    /// Fully resolved callee.
    pub callee: Callee,
}

#[derive(Debug, Clone)]
pub enum CallKind {
    Direct,
    /// vtable slot (load `td` from the receiver, load `vtable[slot]`).
    Virtual {
        slot: u32,
    },
    /// itable lookup (`scoop_rt_itable_lookup(td, iface_td)`), then
    /// `slot` within the returned table.
    Interface {
        interface: InterfaceId,
        slot: u32,
    },
    /// Managed function-value invocation. Argument 0 is the closure ref;
    /// LIR loads its code pointer and calls it with the exact signature.
    Closure {
        function_type: FunctionTypeId,
    },
    /// Runtime-selected function-type bridge from the source closure's
    /// TypeDescriptor table. Argument 0 is the source closure reference.
    FunctionBridge {
        function_type: FunctionTypeId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Callee {
    /// A user function defined in this Cone.
    User(FunctionId),
    /// A monomorphized generic function defined in this Cone.
    Monomorphized(MonomorphizedFunctionId),
    /// A bodyless native declaration in the independent extern arena.
    Extern(ExternFunctionId),
    /// Typed marker used only between CFG construction and the coroutine
    /// state-machine pass. The final MIR handed to LIR contains no such
    /// callee; `register` identifies the concrete protocol method shell.
    CoroutineSuspend { register: MonomorphizedFunctionId },
    /// There is no statically selected function; the exact signature is the
    /// complete typed call target carried through CFG construction.
    Closure(FunctionTypeId),
    /// No source signature is statically known (an `Any`/interface cast).
    /// The target signature selects one bridge-table entry at runtime.
    FunctionBridge(FunctionTypeId),
    /// A runtime function (see `RuntimeFn::symbol`).
    Runtime(RuntimeFn),
}

/// Runtime functions called directly by compiler-generated operations.
/// Source-level core capabilities use ordinary declarations and extern calls;
/// they do not acquire entries in this enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeFn {
    /// Box one value; LIR supplies the addressable payload and its complete
    /// recursive scan program to the managed runtime entry.
    Box,
    /// `scoop_rt_is_instance(obj, td)`
    IsInstance,
    /// `scoop_rt_itable_lookup(td, iface_td)`
    ITableLookup,
    /// GC facilities (spec 14.1; M9 via intrinsics, see
    /// docs/milestone9/DESIGN.md 5.2).
    Pin,
    Unpin,
    GetHandle,
    ReleaseHandle,
    /// Test-only GC hooks (not in the spec, milestone9 DESIGN 5.2).
    GcCollect,
    GcStats,
    /// ABI exception buffer -> ordinary managed object (runtime spec 5).
    MaterializeException,
    StringConcat,
    StringCompare,
    InitializationEnter,
    InitializationSucceed,
    InitializationFail,
    InitializationFailure,
    InitializationCycleMessage,
    /// Noreturn runtime trap, called with a message string constant
    /// (M4: `!!` on `None`; M8: real exceptions).
    Trap,
}

impl RuntimeFn {
    pub fn symbol(self) -> &'static str {
        match self {
            RuntimeFn::Box => "scoop_rt_box",
            RuntimeFn::IsInstance => "scoop_rt_is_instance",
            RuntimeFn::ITableLookup => "scoop_rt_itable_lookup",
            RuntimeFn::Pin => "scoop_rt_pin",
            RuntimeFn::Unpin => "scoop_rt_unpin",
            RuntimeFn::GetHandle => "scoop_rt_get_handle",
            RuntimeFn::ReleaseHandle => "scoop_rt_release_handle",
            RuntimeFn::GcCollect => "scoop_rt_gc_collect",
            RuntimeFn::GcStats => "scoop_rt_gc_stats",
            RuntimeFn::MaterializeException => "scoop_rt_materialize_exception",
            RuntimeFn::StringConcat => "scoop_rt_string_concat",
            RuntimeFn::StringCompare => "scoop_rt_string_compare",
            RuntimeFn::InitializationEnter => "scoop_rt_init_enter",
            RuntimeFn::InitializationSucceed => "scoop_rt_init_succeed",
            RuntimeFn::InitializationFail => "scoop_rt_init_fail",
            RuntimeFn::InitializationFailure => "scoop_rt_init_failure",
            RuntimeFn::InitializationCycleMessage => "scoop_rt_init_cycle_message",
            RuntimeFn::Trap => "scoop_rt_trap",
        }
    }
}
