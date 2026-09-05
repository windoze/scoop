use super::*;

pub struct Module {
    pub globals: Arena<Global>,
    pub initialization_units: Arena<InitializationUnit>,
    /// Struct definitions with complete physical layouts (indexed by
    /// `StructDefId`; ids align with MIR struct ids).
    pub structs: StructDefs,
    /// Enum definitions with fixed representations (indexed by
    /// `EnumDefId`).
    pub enums: EnumDefs,
    pub functions: Vec<Function>,
    /// Native declarations and C-bridge descriptions, transposed from MIR.
    /// ABI-refined references can only be minted while declarations are
    /// inserted into this registry.
    pub extern_functions: ExternFunctions,
    /// C data imports accessed only through generated get/set/address bridges.
    pub native_globals: Arena<NativeGlobal>,
    /// Typed bridge entities used by native-global access records. Separate id
    /// families make get/set/address roles impossible to interchange.
    pub native_global_bridges: NativeGlobalBridges,
    /// Inbound C trampolines that adapt a native signature to a NoGC
    /// Scoop storage-ABI bridge.
    pub callback_bridges: Arena<CallbackBridge>,
    /// GC-aware managed callback trampolines. These are disjoint from M12's
    /// NoGC static callback bridges at the type level.
    pub foreign_callback_families: Arena<ForeignCallbackFamily>,
    pub foreign_callback_bridges: Arena<ForeignCallbackBridge>,
    /// Symbol of the entry function (`scoop_main`).
    pub entry_symbol: String,
    pub meta: LirMeta,
}

#[derive(Debug)]
pub struct InitializationUnit {
    pub stable_key: String,
    pub schedule: InitializationSchedule,
    pub kind: InitializationUnitKind,
    pub failure_root: GlobalId,
    pub initializer: ManagedLocalFunctionRef,
    pub ensure: ManagedLocalFunctionRef,
    pub dependencies: Vec<InitializationUnitId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitializationUnitKind {
    EagerTopLevel { storage: GlobalId },
    LazySingleton { published_root: GlobalId },
}

impl InitializationUnitKind {
    pub const fn storage(self) -> GlobalId {
        match self {
            Self::EagerTopLevel { storage } => storage,
            Self::LazySingleton { published_root } => published_root,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitializationSchedule {
    EagerStartup,
    LazyAccess,
}

#[derive(Debug)]
pub struct CallbackBridge {
    pub source_name: String,
    pub bridge_symbol: String,
    pub trampoline_symbol: String,
    pub params: Vec<CType>,
    pub return_type: CReturnType,
}

/// One concrete managed-callback protocol family. All three ids are nominal:
/// representation-equivalent structs/enums are not interchangeable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ForeignCallbackFamily {
    pub callback: StructDefId,
    pub state: EnumDefId,
    pub failure: EnumDefId,
}

#[derive(Debug)]
pub struct ForeignCallbackBridge {
    pub family: ForeignCallbackFamilyId,
    pub adapter_symbol: String,
    pub trampoline_symbol: String,
    pub signature_symbol: String,
    pub params: Vec<CType>,
    pub return_type: CReturnType,
    pub context_index: u32,
    pub mode: ForeignCallbackMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForeignCallbackMode {
    Reusable,
    OneShot,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForeignCallbackOperation {
    Retain {
        family: ForeignCallbackFamilyId,
        out: TempId,
        callback: Value,
    },
    Release {
        family: ForeignCallbackFamilyId,
        callback: Value,
    },
    State {
        family: ForeignCallbackFamilyId,
        out: TempId,
        callback: Value,
    },
    Failure {
        family: ForeignCallbackFamilyId,
        out: TempId,
        callback: Value,
    },
}

impl ForeignCallbackOperation {
    pub fn callback(self) -> Value {
        match self {
            Self::Retain { callback, .. }
            | Self::Release { callback, .. }
            | Self::State { callback, .. }
            | Self::Failure { callback, .. } => callback,
        }
    }

    pub fn family(self) -> ForeignCallbackFamilyId {
        match self {
            Self::Retain { family, .. }
            | Self::Release { family, .. }
            | Self::State { family, .. }
            | Self::Failure { family, .. } => family,
        }
    }

    pub fn out(self) -> Option<TempId> {
        match self {
            Self::Retain { out, .. } | Self::State { out, .. } | Self::Failure { out, .. } => {
                Some(out)
            }
            Self::Release { .. } => None,
        }
    }
}

#[derive(Debug)]
pub struct NativeGlobal {
    pub source_name: String,
    pub native_symbol: String,
    pub library: String,
    pub c_type: CType,
    pub thread_local: bool,
    pub access: NativeGlobalAccess,
}

impl NativeGlobal {
    pub fn storage_type(&self) -> LirType {
        self.c_type.storage_type()
    }
}

#[derive(Debug, Default)]
pub struct NativeGlobalBridges {
    pub gets: Arena<NativeGlobalGetBridge>,
    pub sets: Arena<NativeGlobalSetBridge>,
    pub addresses: Arena<NativeGlobalAddressBridge>,
}

#[derive(Debug)]
pub struct NativeGlobalGetBridge {
    pub symbol: String,
}

#[derive(Debug)]
pub struct NativeGlobalSetBridge {
    pub symbol: String,
}

#[derive(Debug)]
pub struct NativeGlobalAddressBridge {
    pub symbol: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeGlobalAccess {
    ReadOnly {
        get: NativeGlobalGetBridgeId,
        address: NativeGlobalAddressBridgeId,
    },
    Mutable {
        get: NativeGlobalGetBridgeId,
        set: NativeGlobalSetBridgeId,
        address: NativeGlobalAddressBridgeId,
    },
}

impl NativeGlobalAccess {
    pub fn get(self) -> NativeGlobalGetBridgeId {
        match self {
            Self::ReadOnly { get, .. } | Self::Mutable { get, .. } => get,
        }
    }

    pub fn set(self) -> Option<NativeGlobalSetBridgeId> {
        match self {
            Self::ReadOnly { .. } => None,
            Self::Mutable { set, .. } => Some(set),
        }
    }

    pub fn address(self) -> NativeGlobalAddressBridgeId {
        match self {
            Self::ReadOnly { address, .. } | Self::Mutable { address, .. } => address,
        }
    }
}
