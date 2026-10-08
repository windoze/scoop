//! Target capabilities consumed while turning MIR values into physical LIR.
//!
//! Source integer identity remains target-independent. These scalar entries
//! describe backend storage only, including widths that source integers will
//! select once their semantic kind has already been fixed by an earlier
//! stage. The closed profile also keeps data- and code-pointer qualification
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

/// Coarse physical shape consumed by the closed Scoop ABI classifier.
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
/// Fields are private because only a closed target profile may manufacture a
/// layout. Consumers can inspect it but cannot assemble a contradictory
/// profile from arbitrary integers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ScalarLayout {
    size_bytes: u8,
    alignment_bytes: u8,
}

impl ScalarLayout {
    const fn new(size_bytes: u8, alignment_bytes: u8) -> Self {
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
    const fn new(
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
/// The driver obtains this value from the opaque target registry and passes
/// the same value through LIR lowering and codegen. Private fields and closed
/// constants ensure that a partial set of target capabilities cannot enter
/// the pipeline.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LirTargetProfile {
    id: TargetProfileId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NativeObjectFormat {
    MachO64,
    Elf64,
}

impl LirTargetProfile {
    pub const DARWIN_AARCH64: Self = Self::from_id(TargetProfileId::DarwinAarch64);
    pub const LINUX_X86_64_GNU: Self = Self::from_id(TargetProfileId::LinuxX86_64Gnu);
    pub const LINUX_X86_64_MUSL: Self = Self::from_id(TargetProfileId::LinuxX86_64Musl);

    pub const fn from_id(id: TargetProfileId) -> Self {
        Self { id }
    }

    pub const fn id(self) -> TargetProfileId {
        self.id
    }

    pub const fn native_object_format(self) -> NativeObjectFormat {
        match self.id {
            TargetProfileId::DarwinAarch64 => NativeObjectFormat::MachO64,
            TargetProfileId::LinuxX86_64Gnu | TargetProfileId::LinuxX86_64Musl => {
                NativeObjectFormat::Elf64
            }
        }
    }

    pub fn wire_id(self) -> scoop_identity::TargetProfileWireId {
        scoop_identity::TargetProfileWireId::new(self.id)
    }

    pub const fn contract(self) -> TargetProfileContract {
        TargetProfileContract::new(self)
    }

    pub fn fingerprint(self) -> Result<TargetProfileFingerprint, scoop_wire::HashError> {
        TargetProfileFingerprint::from_profile(self)
    }

    pub const fn canonical_llvm_data_layout(self) -> &'static str {
        match self.id {
            TargetProfileId::DarwinAarch64 => {
                "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32"
            }
            TargetProfileId::LinuxX86_64Gnu | TargetProfileId::LinuxX86_64Musl => {
                "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128"
            }
        }
    }

    /// Total lookup of the physical layout for every backend integer scalar
    /// admitted into LIR.
    pub const fn scalar_layout(self, kind: BackendScalarKind) -> ScalarLayout {
        match kind {
            BackendScalarKind::I1 => ScalarLayout::new(1, 1),
            BackendScalarKind::I8 => ScalarLayout::new(1, 1),
            BackendScalarKind::I16 => ScalarLayout::new(2, 2),
            BackendScalarKind::I32 | BackendScalarKind::F32 => ScalarLayout::new(4, 4),
            BackendScalarKind::I64 | BackendScalarKind::F64 => ScalarLayout::new(8, 8),
        }
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
        ScalarLayout::new(8, 8)
    }

    pub const fn data_pointer(self) -> PointerRepresentation {
        PointerRepresentation::new(
            ScalarLayout::new(8, 8),
            PointerNullEncoding::AllZeroBits,
            InternalPointerCarrier::BitPreservingU64,
        )
    }

    pub const fn code_pointer(self) -> PointerRepresentation {
        PointerRepresentation::new(
            ScalarLayout::new(8, 8),
            PointerNullEncoding::AllZeroBits,
            InternalPointerCarrier::BitPreservingU64,
        )
    }

    pub const fn metadata_pointer_layout(self) -> ScalarLayout {
        ScalarLayout::new(8, 8)
    }

    /// Total physical-layout lookup for the provenance-preserving LIR pointer
    /// sum. Metadata pointers share the target's data-pointer representation
    /// width without acquiring its source-level construction capability.
    pub const fn pointer_layout(self, kind: PointerKind) -> ScalarLayout {
        match kind {
            PointerKind::Managed => self.managed_pointer_layout(),
            PointerKind::Raw => self.data_pointer().layout,
            PointerKind::Code => self.code_pointer().layout,
            PointerKind::Metadata => self.metadata_pointer_layout(),
        }
    }

    /// Closed classifier for non-zero-sized Scoop values.
    ///
    /// This is intentionally not the target C ABI. The same result is stored
    /// in typed LIR and consumed by definitions, every caller, dispatch, and
    /// Scoop extern declarations.
    pub const fn classify_scoop_abi_value(self, shape: ScoopAbiValueShape) -> ScoopAbiPassing {
        match shape {
            ScoopAbiValueShape::Scalar => ScoopAbiPassing::Direct,
            ScoopAbiValueShape::Aggregate => ScoopAbiPassing::Indirect,
            ScoopAbiValueShape::Interface => ScoopAbiPassing::DirectParts,
        }
    }
}
