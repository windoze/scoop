use super::*;
use scoop_hir::{IntegerKind, IntegerSignedness, IntegerWidth, IntrinsicTypeKind};
use scoop_identity::{IntegerBitWidth, Signedness};

impl NativeBoundaryNormalizer<'_> {
    pub(super) fn intrinsic(
        &mut self,
        exact: PersistentExactTypeId,
    ) -> Result<Option<IntrinsicTypeKind>, NativeBoundaryCompileError> {
        match self.exact(exact)? {
            ExactTypeKey::Nominal(_) | ExactTypeKey::NominalApplication { .. } => {
                let (definition, _) = self.definition(exact)?;
                Ok(match definition.shape() {
                    NativeBoundaryNominalShape::Intrinsic(representation) => {
                        Some(representation.family())
                    }
                    NativeBoundaryNominalShape::Reference
                    | NativeBoundaryNominalShape::Struct { .. }
                    | NativeBoundaryNominalShape::Enum { .. } => None,
                })
            }
            ExactTypeKey::RawPointer(_)
            | ExactTypeKey::NativeFunctionPointer { .. }
            | ExactTypeKey::Tuple(_)
            | ExactTypeKey::Function { .. } => Ok(None),
        }
    }
}

pub(super) fn integer_representation(kind: IntegerKind) -> (Signedness, IntegerBitWidth) {
    let signedness = match kind.signedness() {
        IntegerSignedness::Signed => Signedness::Signed,
        IntegerSignedness::Unsigned => Signedness::Unsigned,
    };
    let width = match kind.width() {
        IntegerWidth::W8 => IntegerBitWidth::Bits8,
        IntegerWidth::W16 => IntegerBitWidth::Bits16,
        IntegerWidth::W32 => IntegerBitWidth::Bits32,
        IntegerWidth::W64 => IntegerBitWidth::Bits64,
    };
    (signedness, width)
}

pub(super) fn intrinsic_layout(
    target: scoop_lir::LirTargetProfile,
    family: IntrinsicTypeKind,
) -> Result<PhysicalType, NativeBoundaryCompileError> {
    match family {
        IntrinsicTypeKind::Unit => Ok(PhysicalType {
            size: 0,
            alignment: 1,
            shape: ScoopAbiValueShape::Aggregate,
            gc_free: true,
        }),
        IntrinsicTypeKind::Integer(kind) => {
            let (_, width) = integer_representation(kind);
            Ok(scalar(
                target.scalar_layout(integer_scalar_kind(width)),
                true,
            ))
        }
        IntrinsicTypeKind::Float(kind) => Ok(scalar(target.float_layout(kind), true)),
        IntrinsicTypeKind::Char => Ok(scalar(
            target.scalar_layout(scoop_lir::BackendScalarKind::I32),
            true,
        )),
        IntrinsicTypeKind::Boolean => Ok(scalar(
            target.scalar_layout(scoop_lir::BackendScalarKind::I1),
            true,
        )),
        IntrinsicTypeKind::String
        | IntrinsicTypeKind::Atomic(_)
        | IntrinsicTypeKind::Array
        | IntrinsicTypeKind::MutableArray
        | IntrinsicTypeKind::Any
        | IntrinsicTypeKind::Nothing => Ok(pointer(target, scoop_lir::PointerKind::Managed, false)),
        IntrinsicTypeKind::Ptr | IntrinsicTypeKind::FunPtr | IntrinsicTypeKind::MaybeUninit => {
            Err(NativeBoundaryTargetError::InvalidSignatureShape.into())
        }
    }
}

pub(super) fn is_reference(shape: &NativeBoundaryNominalShape) -> bool {
    match shape {
        NativeBoundaryNominalShape::Reference => true,
        NativeBoundaryNominalShape::Intrinsic(representation) => {
            representation.family().target() == scoop_hir::IntrinsicTypeTarget::Class
        }
        NativeBoundaryNominalShape::Struct { .. } | NativeBoundaryNominalShape::Enum { .. } => {
            false
        }
    }
}
