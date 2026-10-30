//! Complete target layout data consumed while turning MIR values into physical LIR.
//!
//! Source integer identity remains target-independent. These scalar entries
//! describe backend storage only, including widths that source integers will
//! select once their semantic kind has already been fixed by an earlier
//! stage. The explicit profile also keeps data- and code-pointer qualification
//! separate even though the current target gives them the same layout.

use crate::{IntegerKind, PointerKind};

mod contract;
pub use contract::{
    ByteOrder, CAbiLoweringProfile, NativeSymbolNormalization, ScoopAbiClassifier,
    TargetProfileContract, TargetProfileFingerprint,
};

pub use scoop_identity::TargetProfileId;

/// LLVM scalar storage classes needed by LIR layout.
///
/// Signedness is deliberately absent: it affects operations and native ABI
/// classification, not the physical size or natural alignment of an LLVM
/// integer scalar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BackendScalarKind {
    I1,
    I8,
    I16,
    I32,
    I64,
    F32,
    F64,
}

/// Coarse physical shape consumed by Scoop ABI planning.
///
/// Zero-sized values are classified before this point. Interface values carry
/// an object and immutable metadata; ordinary aggregates retain their layouts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ScoopAbiValueShape {
    Scalar,
    Aggregate,
    Interface,
}

/// Passing convention selected for one non-zero-sized Scoop ABI value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ScoopAbiPassing {
    Direct,
    DirectParts,
    Indirect,
}

/// Exact byte size and natural byte alignment of one backend scalar.
///
/// Construction checks the local nonzero size and alignment requirements.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ScalarLayout {
    size_bytes: u8,
    alignment_bytes: u8,
}

impl ScalarLayout {
    pub const fn new(size_bytes: u8, alignment_bytes: u8) -> Self {
        assert!(
            size_bytes > 0 && alignment_bytes.is_power_of_two(),
            "scalar layout requires nonzero size and power-of-two alignment"
        );
        Self {
            size_bytes,
            alignment_bytes,
        }
    }

    pub const fn size_bytes(self) -> u64 {
        self.size_bytes as u64
    }

    pub const fn alignment_bytes(self) -> u64 {
        self.alignment_bytes as u64
    }
}

/// Object representation used by the null value of a qualified pointer
/// family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PointerNullEncoding {
    AllZeroBits,
}

/// Compiler/runtime-internal raw carrier admitted by a pointer profile.
///
/// This is not the Scoop source type `ULong` and does not grant a source-level
/// integer conversion. `BitPreservingU64` promises that every legal non-null
/// address can make a lossless round trip through exactly 64 raw bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum InternalPointerCarrier {
    BitPreservingU64,
}

/// Complete representation qualification for one pointer family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PointerRepresentation {
    layout: ScalarLayout,
    null_encoding: PointerNullEncoding,
    carrier: InternalPointerCarrier,
}

impl PointerRepresentation {
    pub const fn new(
        layout: ScalarLayout,
        null_encoding: PointerNullEncoding,
        carrier: InternalPointerCarrier,
    ) -> Self {
        Self {
            layout,
            null_encoding,
            carrier,
        }
    }

    pub const fn layout(self) -> ScalarLayout {
        self.layout
    }

    pub const fn null_encoding(self) -> PointerNullEncoding {
        self.null_encoding
    }

    pub const fn carrier(self) -> InternalPointerCarrier {
        self.carrier
    }
}

/// Immutable LIR-facing projection of a complete target/backend profile.
///
/// The driver selects a complete description supplied by codegen target data.
/// This crate does not register or select concrete target implementations.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LirTargetProfile {
    id: &'static TargetProfileId,
    description: &'static TargetLayoutDescription,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NativeObjectFormat {
    MachO64,
    Elf64,
    Coff64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TargetArchitecture {
    Aarch64,
    X86_64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ProcedureAbi {
    DarwinAarch64,
    SysvAmd64,
    Win64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TargetExceptionHandling {
    Dwarf,
    WindowsSeh,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TargetLayoutDescription {
    pub canonical_triple: &'static str,
    pub llvm_data_layout: &'static str,
    pub object_format: NativeObjectFormat,
    pub architecture: TargetArchitecture,
    pub procedure_abi: ProcedureAbi,
    pub scoop_abi_classifier: ScoopAbiClassifier,
    pub exception_handling: TargetExceptionHandling,
    pub native_symbol_normalization: NativeSymbolNormalization,
    pub byte_order: ByteOrder,
    pub scalar_layouts: [ScalarLayout; 7],
    pub managed_pointer: ScalarLayout,
    pub data_pointer: PointerRepresentation,
    pub code_pointer: PointerRepresentation,
    pub metadata_pointer: ScalarLayout,
    pub stack_alignment: u64,
    pub maximum_managed_alignment: u64,
    pub maximum_managed_object_size: u64,
}

impl NativeObjectFormat {
    pub fn wire_id(self) -> scoop_identity::ObjectFormatId {
        match self {
            Self::MachO64 => scoop_identity::ObjectFormatId::macho_relocatable(),
            Self::Elf64 => scoop_identity::ObjectFormatId::elf_relocatable(),
            Self::Coff64 => scoop_identity::ObjectFormatId::coff_relocatable(),
        }
    }
}

impl LirTargetProfile {
    pub const fn new(
        id: &'static TargetProfileId,
        description: &'static TargetLayoutDescription,
    ) -> Self {
        Self { id, description }
    }
    pub const fn id(self) -> TargetProfileId {
        *self.id
    }
    pub const fn description(self) -> &'static TargetLayoutDescription {
        self.description
    }
    pub const fn native_object_format(self) -> NativeObjectFormat {
        self.description.object_format
    }
    pub const fn architecture(self) -> TargetArchitecture {
        self.description.architecture
    }
    pub const fn procedure_abi(self) -> ProcedureAbi {
        self.description.procedure_abi
    }
    pub const fn exception_handling(self) -> TargetExceptionHandling {
        self.description.exception_handling
    }
    pub fn wire_id(self) -> scoop_identity::TargetProfileWireId {
        scoop_identity::TargetProfileWireId::new(*self.id)
    }
    pub const fn contract(self) -> TargetProfileContract {
        TargetProfileContract::new(self)
    }
    pub fn fingerprint(self) -> Result<TargetProfileFingerprint, scoop_wire::HashError> {
        TargetProfileFingerprint::from_profile(self)
    }
    pub const fn canonical_llvm_data_layout(self) -> &'static str {
        self.description.llvm_data_layout
    }
    pub const fn scalar_layout(self, kind: BackendScalarKind) -> ScalarLayout {
        self.description.scalar_layouts[match kind {
            BackendScalarKind::I1 => 0,
            BackendScalarKind::I8 => 1,
            BackendScalarKind::I16 => 2,
            BackendScalarKind::I32 => 3,
            BackendScalarKind::I64 => 4,
            BackendScalarKind::F32 => 5,
            BackendScalarKind::F64 => 6,
        }]
    }
    pub const fn float_layout(self, kind: crate::FloatKind) -> ScalarLayout {
        self.scalar_layout(match kind {
            crate::FloatKind::F32 => BackendScalarKind::F32,
            crate::FloatKind::F64 => BackendScalarKind::F64,
        })
    }
    pub const fn integer_layout(self, kind: IntegerKind) -> ScalarLayout {
        self.scalar_layout(kind.width().backend_scalar_kind())
    }
    pub const fn managed_pointer_layout(self) -> ScalarLayout {
        self.description.managed_pointer
    }
    pub const fn data_pointer(self) -> PointerRepresentation {
        self.description.data_pointer
    }
    pub const fn code_pointer(self) -> PointerRepresentation {
        self.description.code_pointer
    }
    pub const fn metadata_pointer_layout(self) -> ScalarLayout {
        self.description.metadata_pointer
    }
    pub const fn pointer_layout(self, kind: PointerKind) -> ScalarLayout {
        match kind {
            PointerKind::Managed => self.managed_pointer_layout(),
            PointerKind::Raw => self.data_pointer().layout,
            PointerKind::Code => self.code_pointer().layout,
            PointerKind::Metadata => self.metadata_pointer_layout(),
        }
    }
}
