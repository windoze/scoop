//! Validate String representation operations at the source boundary.
use super::*;

impl Lowerer {
    pub(crate) fn validate_string_intrinsics(&mut self, files: &[ast::Span]) {
        let Ok(character) = self.core_character_type() else {
            return;
        };
        let string = self.intern_type(hir::Type::String);
        let unit = self.intern_type(hir::Type::Unit);
        let long = self.integer_type(hir::IntegerKind::SIGNED_64);
        let byte = self.integer_type(hir::IntegerKind::SIGNED_8);
        let pointer = self.intern_type(hir::Type::Ptr(byte));
        let bytes = self.array_type(crate::types::ArrayKind::Immutable, byte);
        let mutable_bytes = self.array_type(crate::types::ArrayKind::Mutable, byte);
        let ubyte = self.integer_type(hir::IntegerKind::UNSIGNED_8);
        let buffer = self.array_type(crate::types::ArrayKind::Mutable, ubyte);
        for kind in hir::StringIntrinsic::ALL {
            let Some(function) =
                self.require_intrinsic(hir::IntrinsicFunctionKind::String(kind), files)
            else {
                continue;
            };
            let (parameters, result) = match kind {
                hir::StringIntrinsic::ByteLength => (vec![string], long),
                hir::StringIntrinsic::ByteAt | hir::StringIntrinsic::ByteAtUnchecked => {
                    (vec![string, long], byte)
                }
                hir::StringIntrinsic::AllocateZeroed => (vec![long], string),
                hir::StringIntrinsic::SetByteUnchecked => (vec![string, long, byte], unit),
                hir::StringIntrinsic::CharacterAtByte => (vec![string, long], character),
                hir::StringIntrinsic::CopyUtf8Unchecked => (vec![pointer, long], string),
                hir::StringIntrinsic::SliceBytes => (vec![string, long, long], string),
                hir::StringIntrinsic::FromBytes => (vec![bytes], string),
                hir::StringIntrinsic::ToBytes => (vec![string], bytes),
                hir::StringIntrinsic::ToMutableBytes => (vec![string], mutable_bytes),
                hir::StringIntrinsic::FromBytePrefixUnchecked => (vec![buffer, long], string),
                hir::StringIntrinsic::CopyToBytesUnchecked => (vec![string, buffer, long], unit),
                hir::StringIntrinsic::CopyBytePrefixUnchecked => (vec![buffer, buffer, long], unit),
                hir::StringIntrinsic::CopyRawBytesUnchecked => {
                    (vec![pointer, long, buffer, long], unit)
                }
            };
            let signature = &self.signatures[&function];
            let valid = !signature.is_suspend
                && signature.type_params.is_empty()
                && signature.owner_type_param_count == 0
                && signature.params.iter().map(|p| p.ty).eq(parameters)
                && signature.return_ty == result
                && !self.function_owner.contains_key(&function)
                && !self.extension_receivers.contains_key(&function);
            if !valid {
                self.current_file = self.function_files[&function];
                self.error(
                    self.functions[function].span,
                    format!("malformed core String intrinsic `{}`", kind.name()),
                );
            }
        }
    }
}
