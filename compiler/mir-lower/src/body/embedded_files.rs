use super::*;

impl BodyLowerer<'_> {
    pub(super) fn lower_embedded_files(
        &mut self,
        ty: &mir::Type,
        files: &[scoop_hir::CompileTimeFile],
    ) -> smir::ExprKind {
        let mir::Type::Class(array_type) = *ty else {
            unreachable!("an embedded file table has an array type")
        };
        let mir::ClassRepresentation::Intrinsic(mir::IntrinsicClassRepresentation::Array {
            element: mir::Type::Struct(file_type),
        }) = self.classes[array_type].representation
        else {
            unreachable!("an embedded file table has ordinary struct elements")
        };
        let mir::StructRepresentation::Declared(file) =
            &self.structs.defs[file_type].representation
        else {
            unreachable!("an embedded file is an ordinary struct")
        };
        let mir::Type::Class(bytes_type) = file.fields[1].ty else {
            unreachable!("an embedded file's second field is Array<Byte>")
        };
        let files = files
            .iter()
            .map(|file| {
                let path = self.intern_current_string(file.path.clone());
                let bytes = self.intern_current_immortal(mir::ImmortalConstValue::ByteArray {
                    array_type: bytes_type,
                    bytes: file.bytes.to_vec(),
                });
                mir::EmbeddedFileConst { path, bytes }
            })
            .collect();
        smir::ExprKind::ArrayConst(self.intern_current_immortal(
            mir::ImmortalConstValue::EmbeddedFileArray { array_type, files },
        ))
    }
}
