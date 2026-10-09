use super::*;

pub(crate) fn canonical_scoop_signature(
    module: &mir::Module,
    enums: &lir::EnumDefs,
    exact_signature: identity::ExactCallableSignature,
    parameter_types: &[mir::Type],
    return_type: &mir::Type,
    gc_effect: mir::GcEffect,
    physical: &lir::ScoopAbiSignature,
) -> identity::CanonicalScoopAbiFunctionSignature {
    assert_eq!(
        parameter_types.len(),
        physical.arguments().len(),
        "one physical Scoop ABI convention exists per logical parameter"
    );
    let parameters = parameter_types
        .iter()
        .map(|ty| exact_type_record(module, ty).id())
        .collect::<Vec<_>>();
    let result_exact = exact_type_record(module, return_type).id();
    let arguments = parameters
        .into_iter()
        .zip(physical.arguments())
        .map(|(exact_type, argument)| match argument {
            lir::AbiArgument::ElidedZst(value) => {
                identity::ScoopAbiArgument::elided_zst(scoop_storage(
                    enums,
                    exact_type,
                    value.storage_type(),
                    value.layout().size(),
                    value.layout().alignment(),
                ))
            }
            lir::AbiArgument::Direct(value) => {
                let storage = scoop_storage(
                    enums,
                    exact_type,
                    value.storage_type(),
                    value.layout().size().get(),
                    value.layout().alignment(),
                );
                match value {
                    lir::AbiDirectValue::Scalar(_) => identity::ScoopAbiArgument::direct(storage),
                    lir::AbiDirectValue::DirectParts(parts) => {
                        identity::ScoopAbiArgument::direct_parts(storage, parts.coercion())
                    }
                }
            }
            lir::AbiArgument::Indirect(value) => {
                identity::ScoopAbiArgument::indirect(scoop_storage(
                    enums,
                    exact_type,
                    value.storage_type(),
                    value.layout().size().get(),
                    value.layout().alignment(),
                ))
            }
        })
        .map(|argument| {
            argument.expect("LIR Scoop ABI passing agrees with canonical storage shape")
        })
        .collect();
    let result = match physical.result() {
        lir::AbiReturn::UnitVoid => identity::ScoopAbiReturn::unit_void(),
        lir::AbiReturn::ElidedZst(value) => identity::ScoopAbiReturn::elided_zst(scoop_storage(
            enums,
            result_exact,
            value.storage_type(),
            value.layout().size(),
            value.layout().alignment(),
        ))
        .expect("LIR Scoop ABI passing agrees with canonical storage shape"),
        lir::AbiReturn::Direct(value) => {
            let storage = scoop_storage(
                enums,
                result_exact,
                value.storage_type(),
                value.layout().size().get(),
                value.layout().alignment(),
            );
            match value {
                lir::AbiDirectValue::Scalar(_) => identity::ScoopAbiReturn::direct(storage),
                lir::AbiDirectValue::DirectParts(parts) => {
                    identity::ScoopAbiReturn::direct_parts(storage, parts.coercion())
                }
            }
            .expect("complete physical return plan matches canonical storage")
        }
        lir::AbiReturn::Indirect(value) => identity::ScoopAbiReturn::indirect(scoop_storage(
            enums,
            result_exact,
            value.storage_type(),
            value.layout().size().get(),
            value.layout().alignment(),
        ))
        .expect("LIR Scoop ABI passing agrees with canonical storage shape"),
    };
    identity::CanonicalScoopAbiFunctionSignature::new(
        exact_signature,
        arguments,
        result,
        match gc_effect {
            mir::GcEffect::Managed => identity::GcEffect::Managed,
            mir::GcEffect::NoGc => identity::GcEffect::NoGc,
        },
    )
    .expect("the canonical Scoop ABI preserves the authoritative exact logical signature")
}

fn scoop_storage(
    enums: &lir::EnumDefs,
    exact_type: identity::PersistentExactTypeId,
    ty: &lir::LirType,
    byte_size: u64,
    alignment: NonZeroU64,
) -> identity::CanonicalScoopStorage {
    let shape = lir::scoop_abi_value_shape(enums, ty)
        .expect("validated Scoop ABI storage has a non-void value shape");
    identity::CanonicalScoopStorage::new(
        exact_type,
        byte_size,
        alignment,
        match shape {
            lir::ScoopAbiValueShape::Scalar => identity::ScoopAbiValueShape::Scalar,
            lir::ScoopAbiValueShape::Aggregate => identity::ScoopAbiValueShape::Aggregate,
            lir::ScoopAbiValueShape::Interface => identity::ScoopAbiValueShape::Interface,
        },
    )
}
