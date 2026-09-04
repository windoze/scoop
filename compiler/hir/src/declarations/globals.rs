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
    Managed {
        initializer: ManagedGlobalInitializer,
    },
    /// Explicitly addressable M12 `@Global` / `@ThreadLocal` raw storage.
    Local {
        thread_local: bool,
        initializer: ConstantValue,
    },
    Extern {
        library: String,
        native_symbol: String,
        thread_local: bool,
    },
}

#[derive(Debug, Clone)]
pub enum ManagedGlobalInitializer {
    Image(ConstantValue),
    RuntimeZeroed(InitializationUnitId),
}

/// A typed initializer whose complete representation can be emitted directly
/// into the program image. Raw storage accepts only its GC-free subset.
#[derive(Debug, Clone)]
pub enum ConstantValue {
    Int(i64),
    Bool(bool),
    String(String),
    NullPtr,
    NullFunPtr,
    EnumUnit {
        application: EnumApplicationId,
        variant: u32,
    },
    Struct {
        application: StructApplicationId,
        fields: Vec<ConstantValue>,
    },
}
