use super::*;

#[derive(Debug, Clone)]
pub struct Global {
    /// Link/storage name. Source lookup is owned by the logical property.
    pub name: String,
    pub property: PropertyId,
    pub ty: TypeId,
    /// Physical storage mutability. Hidden delegate slots remain immutable
    /// even when their logical property exposes a setter.
    pub mutable: bool,
    pub storage: GlobalStorage,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum GlobalStorage {
    /// Compiler-managed backing storage for an ordinary top-level property.
    /// Unlike M12 raw storage, this may contain managed references and is
    /// never exposed through `addressOf`.
    Managed { state: HirStaticInitialState },
    /// Explicitly addressable M12 `@Global` / `@ThreadLocal` raw storage.
    Local {
        thread_local: bool,
        initializer: HirConstantImage,
    },
    Extern {
        library: String,
        native_symbol: String,
        thread_local: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HirStaticInitialState {
    ZeroedForRuntimeUnit { unit: InitializationUnitId },
    EncodedStaticValue { payload: HirConstantImage },
}

/// A typed initializer whose complete representation can be emitted directly
/// into the program image. Raw storage accepts only its GC-free subset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HirConstantImage {
    Integer(HirIntegerConstant),
    Boolean(bool),
    String(String),
    NullPointer(HirPointerNullKind),
    EnumUnit {
        variant: AppliedEnumVariantRef,
    },
    Struct {
        application: StructApplicationId,
        fields: Vec<HirConstantImage>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HirPointerNullKind {
    Raw,
    Code,
}
