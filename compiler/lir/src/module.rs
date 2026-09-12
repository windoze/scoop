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
    /// Effect-refined identity of the Scoop entry body. The fixed native
    /// executable entry shim is emitted separately and is not a Scoop body.
    pub entry: LocalFunctionRef,
    pub meta: LirMeta,
}

#[derive(Debug)]
pub struct InitializationUnit {
    pub identity: InitializationUnitIdentityRecord,
    pub display_name: String,
    pub schedule: InitializationSchedule,
    pub kind: InitializationUnitKind,
    pub failure_root: GlobalId,
    pub initializer: ManagedLocalFunctionRef,
    pub ensure: ManagedLocalFunctionRef,
    pub dependencies: Vec<InitializationUnitId>,
}

pub type InitializationUnitIdentityRecord = scoop_identity::CborIdentityRecord<
    scoop_identity::PersistentInitializationUnitId,
    scoop_identity::InitializationUnitKey,
>;

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
    pub bridge: NoGcLocalFunctionRef,
    pub trampoline_symbol: String,
    pub params: Vec<CType>,
    pub return_type: CReturnType,
}

/// One concrete managed-callback protocol family. All three ids are nominal:
/// representation-equivalent structs/enums are not interchangeable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ForeignCallbackFamily {
    pub callback: StructDefId,
    pub modes: ForeignCallbackModes,
    pub states: ForeignCallbackStates,
    pub failure_result: ForeignCallbackFailureResult,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ForeignCallbackModes {
    reusable: LirVariantRef,
    one_shot: LirVariantRef,
}

impl ForeignCallbackModes {
    pub fn checked(
        enums: &EnumDefs,
        reusable: LirVariantRef,
        one_shot: LirVariantRef,
    ) -> Option<Self> {
        if !enums.contains_variant(reusable)
            || !enums.contains_variant(one_shot)
            || reusable.definition() != one_shot.definition()
            || reusable == one_shot
            || reusable.index() != 0
            || one_shot.index() != 1
        {
            return None;
        }
        let EnumRepr::Tagged { variants, .. } = &enums[reusable.definition()].repr else {
            return None;
        };
        (variants.len() == 2 && variants.iter().all(|variant| variant.fields.is_empty()))
            .then_some(Self { reusable, one_shot })
    }

    pub const fn reusable(self) -> LirVariantRef {
        self.reusable
    }

    pub const fn one_shot(self) -> LirVariantRef {
        self.one_shot
    }

    pub const fn definition(self) -> EnumDefId {
        self.reusable.definition()
    }

    pub fn runtime_code(self, mode: LirVariantRef) -> Option<u32> {
        if mode == self.reusable {
            Some(0)
        } else if mode == self.one_shot {
            Some(1)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ForeignCallbackStates {
    registered: LirVariantRef,
    active: LirVariantRef,
    completed: LirVariantRef,
    failed: LirVariantRef,
}

impl ForeignCallbackStates {
    pub fn checked(
        enums: &EnumDefs,
        registered: LirVariantRef,
        active: LirVariantRef,
        completed: LirVariantRef,
        failed: LirVariantRef,
    ) -> Option<Self> {
        let variants = [registered, active, completed, failed];
        if variants.iter().any(|variant| {
            !enums.contains_variant(*variant) || variant.definition() != registered.definition()
        }) {
            return None;
        }
        for (index, variant) in variants.iter().enumerate() {
            if variants[..index].contains(variant) || variant.index() as usize != index {
                return None;
            }
        }
        let EnumRepr::Tagged {
            variants: definitions,
            ..
        } = &enums[registered.definition()].repr
        else {
            return None;
        };
        (definitions.len() == 4 && definitions.iter().all(|variant| variant.fields.is_empty()))
            .then_some(Self {
                registered,
                active,
                completed,
                failed,
            })
    }

    pub const fn registered(self) -> LirVariantRef {
        self.registered
    }

    pub const fn active(self) -> LirVariantRef {
        self.active
    }

    pub const fn completed(self) -> LirVariantRef {
        self.completed
    }

    pub const fn failed(self) -> LirVariantRef {
        self.failed
    }

    pub const fn definition(self) -> EnumDefId {
        self.registered.definition()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ForeignCallbackFailureResult {
    some_payload: LirVariantFieldRef,
    none: LirVariantRef,
}

impl ForeignCallbackFailureResult {
    pub fn checked(
        enums: &EnumDefs,
        some_payload: LirVariantFieldRef,
        none: LirVariantRef,
    ) -> Option<Self> {
        if !enums.contains_variant_field(some_payload)
            || !enums.contains_variant(none)
            || some_payload.definition() != none.definition()
            || some_payload.variant() == none
        {
            return None;
        }
        let EnumRepr::Niche {
            kind: NichePointerKind::Managed,
            payload_variant,
        } = &enums[some_payload.definition()].repr
        else {
            return None;
        };
        (some_payload.variant().index() == *payload_variant
            && some_payload.index() == 0
            && enums.variant_field_ref(none, 0).is_none())
        .then_some(Self { some_payload, none })
    }

    pub const fn some_payload(self) -> LirVariantFieldRef {
        self.some_payload
    }

    pub const fn none(self) -> LirVariantRef {
        self.none
    }

    pub const fn definition(self) -> EnumDefId {
        self.some_payload.definition()
    }
}

#[derive(Debug)]
pub struct ForeignCallbackBridge {
    /// Persistent identity shared with the managed adapter materialization.
    pub application: scoop_identity::PersistentCallbackApplicationId,
    pub family: ForeignCallbackFamilyId,
    pub adapter: ManagedLocalFunctionRef,
    pub trampoline_symbol: String,
    pub signature_symbol: String,
    pub params: Vec<CType>,
    pub return_type: CReturnType,
    pub context_index: u32,
    pub mode: LirVariantRef,
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
