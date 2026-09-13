use super::*;

#[derive(Debug)]
pub struct ExternFunction {
    pub source_name: String,
    pub native_symbol: String,
    pub library: String,
    pub calling_convention: CallingConvention,
    pub kind: ExternFunctionKind,
}

#[derive(Debug)]
pub struct ExternFunctionIdentity {
    pub source_name: String,
    pub native_symbol: String,
    pub library: String,
    pub calling_convention: CallingConvention,
}

#[derive(Debug)]
pub struct CExternFunction {
    pub identity: ExternFunctionIdentity,
    pub bridge: GeneratedBridgeEntryIdentity,
    pub signature: CFunctionType,
}

#[derive(Debug)]
pub struct ScoopExternFunction {
    pub identity: ExternFunctionIdentity,
    pub gc_effect: GcEffect,
    pub signature: ScoopAbiSignature,
}

/// ABI-refined identities into `Module::extern_functions`. The shared arena
/// remains the declaration store, while call destinations can only carry the
/// identity family admitted by their native transition protocol. Their private
/// constructors prevent downstream consumers from reclassifying a plain id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CExternFunctionRef(ExternFunctionId);

impl CExternFunctionRef {
    pub fn declaration(self) -> ExternFunctionId {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScoopExternFunctionRef(ExternFunctionId);

impl ScoopExternFunctionRef {
    pub fn declaration(self) -> ExternFunctionId {
        self.0
    }
}

/// Declaration store and the sole producer of ABI-refined extern identities.
#[derive(Debug, Default)]
pub struct ExternFunctions {
    declarations: Arena<ExternFunction>,
}

impl ExternFunctions {
    pub fn alloc_c(&mut self, function: CExternFunction) -> CExternFunctionRef {
        let identity = function.identity;
        CExternFunctionRef(self.declarations.alloc(ExternFunction {
            source_name: identity.source_name,
            native_symbol: identity.native_symbol,
            library: identity.library,
            calling_convention: identity.calling_convention,
            kind: ExternFunctionKind::C {
                bridge: Box::new(function.bridge),
                signature: function.signature,
            },
        }))
    }

    pub fn alloc_scoop(&mut self, function: ScoopExternFunction) -> ScoopExternFunctionRef {
        let identity = function.identity;
        ScoopExternFunctionRef(self.declarations.alloc(ExternFunction {
            source_name: identity.source_name,
            native_symbol: identity.native_symbol,
            library: identity.library,
            calling_convention: identity.calling_convention,
            kind: ExternFunctionKind::Scoop {
                gc_effect: function.gc_effect,
                signature: function.signature,
            },
        }))
    }

    pub fn iter(&self) -> impl Iterator<Item = (ExternFunctionId, &ExternFunction)> {
        self.declarations.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.declarations.is_empty()
    }
}

impl std::ops::Index<ExternFunctionId> for ExternFunctions {
    type Output = ExternFunction;

    fn index(&self, index: ExternFunctionId) -> &Self::Output {
        &self.declarations[index]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallingConvention {
    Cdecl,
}

impl scoop_wire::WireEncode for CallingConvention {
    fn encode(
        &self,
        encoder: &mut scoop_wire::Encoder,
    ) -> Result<(), scoop_wire::cbor::EncodeError> {
        encoder.unsigned(match self {
            Self::Cdecl => 1,
        })
    }
}

impl scoop_wire::WireDecode for CallingConvention {
    fn decode(decoder: &mut scoop_wire::Decoder<'_, '_>) -> Result<Self, scoop_wire::WireError> {
        match decoder.unsigned()? {
            1 => Ok(Self::Cdecl),
            tag => Err(scoop_wire::WireError::new(
                scoop_wire::WireErrorKind::UnknownTag { tag },
                decoder.path().clone(),
                Some(decoder.position()),
            )),
        }
    }
}

#[derive(Debug)]
pub enum ExternFunctionKind {
    C {
        bridge: Box<GeneratedBridgeEntryIdentity>,
        signature: CFunctionType,
    },
    Scoop {
        gc_effect: GcEffect,
        signature: ScoopAbiSignature,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CStructRef(StructDefId);

impl CStructRef {
    pub(crate) const fn from_validated_definition(definition: StructDefId) -> Self {
        Self(definition)
    }

    pub const fn definition(self) -> StructDefId {
        self.0
    }
}

/// Refined identity of a nullable raw-pointer enum together with the exact C
/// object type carried by its non-null payload.  Keeping the pointee on the
/// refinement prevents an `Option<Ptr<P>>` storage identity from being reused
/// with an unrelated `Ptr<Q>` spelling at a C boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NullableDataPointerEnumRef {
    definition: EnumDefId,
    pointee: Box<CDataPointee>,
}

impl NullableDataPointerEnumRef {
    pub(crate) fn from_validated_definition(definition: EnumDefId, pointee: CDataPointee) -> Self {
        Self {
            definition,
            pointee: Box::new(pointee),
        }
    }

    pub const fn definition(&self) -> EnumDefId {
        self.definition
    }

    pub fn pointee(&self) -> &CDataPointee {
        &self.pointee
    }
}

/// Refined identity of a nullable code-pointer enum together with the exact C
/// function type carried by its non-null payload.  Function-pointer storage is
/// therefore inseparable from the signature that justified the refinement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NullableCodePointerEnumRef {
    definition: EnumDefId,
    signature: Box<CFunctionType>,
}

impl NullableCodePointerEnumRef {
    pub(crate) fn from_validated_definition(
        definition: EnumDefId,
        signature: CFunctionType,
    ) -> Self {
        Self {
            definition,
            signature: Box::new(signature),
        }
    }

    pub const fn definition(&self) -> EnumDefId {
        self.definition
    }

    pub fn signature(&self) -> &CFunctionType {
        &self.signature
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CDataPointee {
    OpaqueVoid,
    Object(Box<CType>),
}

impl CDataPointee {
    pub fn dump(&self) -> String {
        match self {
            Self::OpaqueVoid => "opaque-void".to_string(),
            Self::Object(ty) => ty.dump(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CDataPointerStorage {
    Direct,
    Nullable(NullableDataPointerEnumRef),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CCodePointerStorage {
    Direct,
    Nullable(NullableCodePointerEnumRef),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CFunctionType {
    pub params: Vec<CType>,
    pub return_type: CReturnType,
}

impl CFunctionType {
    pub fn storage_params(&self) -> Vec<LirType> {
        self.params.iter().map(CType::storage_type).collect()
    }

    pub fn storage_return_type(&self) -> LirType {
        self.return_type.storage_type()
    }

    pub fn dump(&self) -> String {
        let params = self
            .params
            .iter()
            .map(CType::dump)
            .collect::<Vec<_>>()
            .join(",");
        format!("({params})->{}", self.return_type.dump())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CReturnType {
    Void,
    Value(Box<CType>),
}

impl CReturnType {
    pub fn storage_type(&self) -> LirType {
        match self {
            Self::Void => LirType::Void,
            Self::Value(ty) => ty.storage_type(),
        }
    }

    pub const fn is_void(&self) -> bool {
        matches!(self, Self::Void)
    }

    pub fn dump(&self) -> String {
        match self {
            Self::Void => "void".to_string(),
            Self::Value(ty) => ty.dump(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CType {
    Integer(IntegerKind),
    Boolean,
    DataPointer {
        pointee: CDataPointee,
        storage: CDataPointerStorage,
    },
    CodePointer {
        signature: Box<CFunctionType>,
        storage: CCodePointerStorage,
    },
    Struct(CStructRef),
}

impl CType {
    /// The single physical LIR storage type implied by this exact C type.
    /// C declarations and their physical signatures must derive from this
    /// method rather than storing a second, potentially contradictory type.
    pub fn storage_type(&self) -> LirType {
        match self {
            Self::Integer(kind) => kind.scalar_type(),
            Self::Boolean => LirType::I1,
            Self::DataPointer {
                storage: CDataPointerStorage::Direct,
                ..
            } => LirType::Ptr(PointerKind::Raw),
            Self::DataPointer {
                storage: CDataPointerStorage::Nullable(reference),
                ..
            } => LirType::Enum(reference.definition()),
            Self::CodePointer {
                storage: CCodePointerStorage::Direct,
                ..
            } => LirType::Ptr(PointerKind::Code),
            Self::CodePointer {
                storage: CCodePointerStorage::Nullable(reference),
                ..
            } => LirType::Enum(reference.definition()),
            Self::Struct(reference) => LirType::Struct(reference.definition()),
        }
    }

    /// Stable structural spelling used by LIR dumps.  Nullable pointer
    /// storage includes the exact type captured by its refined enum reference
    /// so a malformed outer/bound pair is visible instead of collapsing to
    /// the same physical pointer or enum storage.
    pub fn dump(&self) -> String {
        match self {
            Self::Integer(kind) => kind.canonical_name().to_string(),
            Self::Boolean => "Boolean".to_string(),
            Self::DataPointer { pointee, storage } => match storage {
                CDataPointerStorage::Direct => {
                    format!("data-ptr<{},direct>", pointee.dump())
                }
                CDataPointerStorage::Nullable(reference) => format!(
                    "data-ptr<{},nullable=enum{}<{}>>",
                    pointee.dump(),
                    reference.definition().into_raw().into_u32(),
                    reference.pointee().dump(),
                ),
            },
            Self::CodePointer { signature, storage } => match storage {
                CCodePointerStorage::Direct => {
                    format!("code-ptr<{},direct>", signature.dump())
                }
                CCodePointerStorage::Nullable(reference) => format!(
                    "code-ptr<{},nullable=enum{}<{}>>",
                    signature.dump(),
                    reference.definition().into_raw().into_u32(),
                    reference.signature().dump(),
                ),
            },
            Self::Struct(reference) => {
                format!("c-struct{}", reference.definition().into_raw().into_u32())
            }
        }
    }
}
