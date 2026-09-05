//! Target capabilities consumed while turning MIR values into physical LIR.
//!
//! Source integer identity remains target-independent. These scalar entries
//! describe backend storage only, including widths that source integers will
//! select once their semantic kind has already been fixed by an earlier
//! stage. The closed profile also keeps data- and code-pointer qualification
//! separate even though the current target gives them the same layout.

use crate::PointerKind;

/// Stable identity of one complete executable target profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TargetProfileId {
    DarwinAarch64,
}

impl TargetProfileId {
    /// Canonical spelling used by diagnostics and target metadata.
    pub const fn canonical_name(self) -> &'static str {
        match self {
            Self::DarwinAarch64 => "darwin-aarch64",
        }
    }
}

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
    canonical_llvm_data_layout: &'static str,
    i1: ScalarLayout,
    i8: ScalarLayout,
    i16: ScalarLayout,
    i32: ScalarLayout,
    i64: ScalarLayout,
    managed_pointer_layout: ScalarLayout,
    data_pointer: PointerRepresentation,
    code_pointer: PointerRepresentation,
    metadata_pointer_layout: ScalarLayout,
}

impl LirTargetProfile {
    /// The sole executable profile currently qualified by Scoop.
    pub const DARWIN_AARCH64: Self = {
        const BYTE: ScalarLayout = ScalarLayout::new(1, 1);
        const WORD16: ScalarLayout = ScalarLayout::new(2, 2);
        const WORD32: ScalarLayout = ScalarLayout::new(4, 4);
        const WORD64: ScalarLayout = ScalarLayout::new(8, 8);
        const QUALIFIED_POINTER: PointerRepresentation = PointerRepresentation::new(
            WORD64,
            PointerNullEncoding::AllZeroBits,
            InternalPointerCarrier::BitPreservingU64,
        );

        Self {
            id: TargetProfileId::DarwinAarch64,
            canonical_llvm_data_layout: "e-m:o-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-n32:64-S128-Fn32",
            i1: BYTE,
            i8: BYTE,
            i16: WORD16,
            i32: WORD32,
            i64: WORD64,
            managed_pointer_layout: WORD64,
            data_pointer: QUALIFIED_POINTER,
            code_pointer: QUALIFIED_POINTER,
            metadata_pointer_layout: WORD64,
        }
    };

    pub const fn id(self) -> TargetProfileId {
        self.id
    }

    pub const fn canonical_llvm_data_layout(self) -> &'static str {
        self.canonical_llvm_data_layout
    }

    /// Total lookup of the physical layout for every backend integer scalar
    /// admitted into LIR.
    pub const fn scalar_layout(self, kind: BackendScalarKind) -> ScalarLayout {
        match kind {
            BackendScalarKind::I1 => self.i1,
            BackendScalarKind::I8 => self.i8,
            BackendScalarKind::I16 => self.i16,
            BackendScalarKind::I32 => self.i32,
            BackendScalarKind::I64 => self.i64,
        }
    }

    pub const fn managed_pointer_layout(self) -> ScalarLayout {
        self.managed_pointer_layout
    }

    pub const fn data_pointer(self) -> PointerRepresentation {
        self.data_pointer
    }

    pub const fn code_pointer(self) -> PointerRepresentation {
        self.code_pointer
    }

    pub const fn metadata_pointer_layout(self) -> ScalarLayout {
        self.metadata_pointer_layout
    }

    /// Total physical-layout lookup for the provenance-preserving LIR pointer
    /// sum. Metadata pointers share the target's data-pointer representation
    /// width without acquiring its source-level construction capability.
    pub const fn pointer_layout(self, kind: PointerKind) -> ScalarLayout {
        match kind {
            PointerKind::Managed => self.managed_pointer_layout,
            PointerKind::Raw => self.data_pointer.layout,
            PointerKind::Code => self.code_pointer.layout,
            PointerKind::Metadata => self.metadata_pointer_layout,
        }
    }
}
