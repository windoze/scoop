//! Fully resolved call targets and compiler-runtime destinations.

use super::*;

#[derive(Debug, Clone)]
pub struct Call {
    pub target: CallTarget,
    pub args: Vec<Expr>,
    /// Transient structured-cleanup context. Coroutine conversion consumes
    /// this value and every final MIR call must carry
    /// [`CoroutinePendingContext::Root`].
    pub pending: CoroutinePendingContext,
}

#[derive(Debug, Clone, Default)]
pub enum CoroutinePendingContext {
    /// The call is not executing a cleanup on behalf of an outer transfer.
    #[default]
    Root,
    /// Outer-to-inner transfers whose cleanup bodies contain this call.
    Chain(NonEmptyCoroutinePendingChain),
}

impl CoroutinePendingContext {
    pub const fn is_root(&self) -> bool {
        matches!(self, Self::Root)
    }

    pub fn from_transfers(transfers: Vec<CoroutinePendingSourceTransfer>) -> Option<Self> {
        NonEmptyCoroutinePendingChain::from_vec(transfers).map(Self::Chain)
    }
}

/// A structurally non-empty, outer-to-inner pending-transfer chain retained
/// only until coroutine conversion has generated final saved-slot metadata.
#[derive(Debug, Clone)]
pub struct NonEmptyCoroutinePendingChain {
    first: CoroutinePendingSourceTransfer,
    rest: Vec<CoroutinePendingSourceTransfer>,
}

impl NonEmptyCoroutinePendingChain {
    pub fn new(
        first: CoroutinePendingSourceTransfer,
        rest: Vec<CoroutinePendingSourceTransfer>,
    ) -> Self {
        Self { first, rest }
    }

    pub fn from_vec(mut transfers: Vec<CoroutinePendingSourceTransfer>) -> Option<Self> {
        if transfers.is_empty() {
            return None;
        }
        let rest = transfers.split_off(1);
        let first = transfers
            .pop()
            .expect("the non-empty chain has a first item");
        Some(Self { first, rest })
    }

    pub fn iter(&self) -> impl Iterator<Item = &CoroutinePendingSourceTransfer> {
        std::iter::once(&self.first).chain(self.rest.iter())
    }

    pub fn into_vec(self) -> Vec<CoroutinePendingSourceTransfer> {
        std::iter::once(self.first).chain(self.rest).collect()
    }
}

#[derive(Debug, Clone)]
pub enum CoroutinePendingSourceTransfer {
    Fallthrough(CoroutineCleanupFallthroughTarget),
    Return(CoroutinePendingSourceReturn),
    Break(CoroutineLoopExitTarget),
    Continue(CoroutineLoopHeaderTarget),
    ManagedThrow(CoroutinePendingSourceManagedThrow),
}

#[derive(Debug, Clone)]
pub enum CoroutinePendingSourceReturn {
    Unit,
    Value(CoroutinePendingSourceValue),
}

impl CoroutinePendingSourceReturn {
    pub fn value(expression: Expr) -> Option<Self> {
        CoroutinePendingSourceValue::checked(expression).map(Self::Value)
    }

    pub const fn expression(&self) -> Option<&Expr> {
        match self {
            Self::Unit => None,
            Self::Value(value) => Some(value.expression()),
        }
    }
}

/// A pending dynamic payload materialized into an immutable CFG local before
/// entering cleanup.  This closed wrapper prevents later coroutine analysis
/// from trying to identify an arbitrary expression structurally.
#[derive(Debug, Clone)]
pub struct CoroutinePendingSourceValue {
    expression: Expr,
}

impl CoroutinePendingSourceValue {
    pub fn checked(expression: Expr) -> Option<Self> {
        matches!(&expression.kind, ExprKind::Local(_)).then_some(Self { expression })
    }

    pub const fn expression(&self) -> &Expr {
        &self.expression
    }

    pub fn local(&self) -> LocalId {
        let ExprKind::Local(local) = &self.expression.kind else {
            unreachable!()
        };
        *local
    }

    pub fn into_expression(self) -> Expr {
        self.expression
    }
}

#[derive(Debug, Clone)]
pub struct CoroutinePendingSourceManagedThrow {
    exception: Expr,
    unwind: Option<CoroutineUnwindTarget>,
}

impl CoroutinePendingSourceManagedThrow {
    pub fn checked(exception: Expr, unwind: Option<CoroutineUnwindTarget>) -> Option<Self> {
        matches!(&exception.kind, ExprKind::Local(_)).then_some(Self { exception, unwind })
    }

    pub const fn exception(&self) -> &Expr {
        &self.exception
    }

    pub fn exception_local(&self) -> LocalId {
        let ExprKind::Local(local) = &self.exception.kind else {
            unreachable!()
        };
        *local
    }

    pub const fn unwind(&self) -> Option<CoroutineUnwindTarget> {
        self.unwind
    }

    pub fn into_parts(self) -> (Expr, Option<CoroutineUnwindTarget>) {
        (self.exception, self.unwind)
    }
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
    /// A direct call through the trusted-core external bridge. The id belongs
    /// to this MIR module's imported-callable arena, not to a local function
    /// or extern declaration arena.
    CoreExternal(ImportedCoreCallableUseId),
    /// A direct call to a Strong definition owned by an ordinary dependency.
    /// The id belongs to this module's independent dependency-use arena; it is
    /// neither a local Strong function nor a native declaration.
    DependencyStrong(ImportedDependencyMirCallableId),
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
