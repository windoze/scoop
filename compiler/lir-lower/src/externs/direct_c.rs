use scoop_lir as lir;

pub(super) fn classify(
    target: lir::LirTargetProfile,
    signature: &lir::CFunctionType,
) -> Option<lir::DirectCSignature> {
    match target.id() {
        lir::TargetProfileId::DarwinAarch64
        | lir::TargetProfileId::LinuxX86_64Gnu
        | lir::TargetProfileId::LinuxX86_64Musl => {
            let params = signature.params.iter().map(scalar).collect::<Option<_>>()?;
            let result = match &signature.return_type {
                lir::CReturnType::Void => lir::DirectCReturn::Void,
                lir::CReturnType::Value(ty) => lir::DirectCReturn::Value(scalar(ty)?),
            };
            Some(lir::DirectCSignature { params, result })
        }
    }
}

fn scalar(ty: &lir::CType) -> Option<lir::DirectCValue> {
    use lir::{CIntegerExtension as Extension, DirectCType as Scalar};
    let (ty, extension) = match ty {
        lir::CType::Integer(kind) => {
            let extension = if kind.width().bits() < 32 {
                match kind.signedness() {
                    lir::IntegerSignedness::Signed => Extension::Sign,
                    lir::IntegerSignedness::Unsigned => Extension::Zero,
                }
            } else {
                Extension::None
            };
            (Scalar::Integer(*kind), extension)
        }
        lir::CType::Boolean => (Scalar::Boolean, Extension::Zero),
        lir::CType::Float(kind) => (Scalar::Float(*kind), Extension::None),
        lir::CType::DataPointer { storage, .. } => {
            (Scalar::DataPointer(storage.clone()), Extension::None)
        }
        lir::CType::CodePointer { storage, .. } => {
            (Scalar::CodePointer(storage.clone()), Extension::None)
        }
        lir::CType::Struct(_) => return None,
    };
    Some(lir::DirectCValue { ty, extension })
}
