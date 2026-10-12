use super::*;

pub(crate) fn scalar_carrier(ty: &lir::LirType, enums: &lir::EnumDefs) -> Option<lir::AbiCarrier> {
    use lir::{AbiCarrier as C, FloatKind, LirType as T};
    Some(match ty {
        T::I1 | T::I8 => C::Integer(8),
        T::I16 => C::Integer(16),
        T::I32 => C::Integer(32),
        T::I64 | T::MachineScalar(_) => C::Integer(64),
        T::F32 => C::Float(FloatKind::F32),
        T::F64 => C::Float(FloatKind::F64),
        T::Ptr(kind) => C::Pointer(*kind),
        T::Enum(id) => match enums[*id].repr {
            lir::EnumRepr::Integral { kind, .. } => C::Integer(kind.width().bits() as u8),
            lir::EnumRepr::Niche { kind, .. } => C::Pointer(match kind {
                lir::NullNicheKind::Managed => lir::PointerKind::Managed,
                lir::NullNicheKind::Raw => lir::PointerKind::Raw,
                lir::NullNicheKind::Code => lir::PointerKind::Code,
                lir::NullNicheKind::Interface => return None,
            }),
            _ => return None,
        },
        _ => return None,
    })
}
