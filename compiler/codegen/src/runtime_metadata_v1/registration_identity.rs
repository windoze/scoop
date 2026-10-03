use inkwell::context::Context;
use inkwell::values::StructValue;
use scoop_lir::RegistrationDefinitionOwner;

use super::RuntimeMetadataV1Types;

pub(super) fn definition_linkage(
    request: scoop_lir::PersistentSymbolRequest,
) -> Result<inkwell::module::Linkage, crate::CodegenError> {
    match request.linkage() {
        scoop_lir::LinkageClass::ConeStrong => Ok(inkwell::module::Linkage::External),
        scoop_lir::LinkageClass::OdrWeak => Ok(inkwell::module::Linkage::WeakODR),
        _ => Err(crate::CodegenError(format!(
            "`{}` is not a local Strong or ODR definition",
            request.symbol()
        ))),
    }
}

pub(super) fn registration_identity_value<'ctx>(
    context: &'ctx Context,
    types: &RuntimeMetadataV1Types<'ctx>,
    semantic_id: &[u8; 32],
    owner: RegistrationDefinitionOwner,
) -> StructValue<'ctx> {
    let i32 = context.i32_type();
    let digest = |bytes: &[u8; 32]| digest_value(context, types.digest, bytes);
    let zero = types.digest.const_zero();
    let (linkage, group, member) = match owner {
        RegistrationDefinitionOwner::Strong => (1, zero, zero),
        RegistrationDefinitionOwner::Odr { group, member } => {
            (2, digest(group.as_array()), digest(member.as_array()))
        }
    };
    types.registration_identity.const_named_struct(&[
        i32.const_int(linkage, false).into(),
        i32.const_zero().into(),
        digest(semantic_id).into(),
        group.into(),
        member.into(),
        zero.into(),
    ])
}

pub(super) fn digest_value<'ctx>(
    context: &'ctx Context,
    digest_type: inkwell::types::StructType<'ctx>,
    bytes: &[u8; 32],
) -> StructValue<'ctx> {
    let i8 = context.i8_type();
    let values = bytes
        .iter()
        .map(|byte| i8.const_int(u64::from(*byte), false))
        .collect::<Vec<_>>();
    digest_type.const_named_struct(&[i8.const_array(&values).into()])
}
