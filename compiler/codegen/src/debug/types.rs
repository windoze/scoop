use super::*;
use scoop_lir::{DebugEncoding, DebugType, DebugTypeKind, DebugTypeRef};

impl<'ctx> DebugContext<'_, 'ctx> {
    pub(super) fn source_type(&self, id: scoop_lir::PersistentExactTypeId) -> DIType<'ctx> {
        if let Some(ty) = self.types.borrow().get(&id) {
            return *ty;
        }
        // LLVM temporary metadata closes recursive reference types. It is
        // replaced exactly once before DIBuilder is finalized.
        let temporary = unsafe { self.builder.create_placeholder_derived_type(self.context) };
        self.types.borrow_mut().insert(id, temporary.as_type());
        let ty = self.describe_type(&self.source_types[&id], &format!("scoop.{id}"));
        unsafe {
            inkwell::llvm_sys::debuginfo::LLVMMetadataReplaceAllUsesWith(
                temporary.as_mut_ptr(),
                ty.as_mut_ptr(),
            );
        }
        self.types.borrow_mut().insert(id, ty);
        ty
    }

    fn type_ref(&self, ty: &DebugTypeRef, unique: &str) -> DIType<'ctx> {
        match ty {
            DebugTypeRef::Source(id) => self.source_type(*id),
            DebugTypeRef::Inline(ty) => self.describe_type(ty, unique),
        }
    }

    fn shape<'t>(&'t self, ty: &'t DebugTypeRef) -> &'t DebugType {
        match ty {
            DebugTypeRef::Source(id) => &self.source_types[id],
            DebugTypeRef::Inline(ty) => ty,
        }
    }

    fn describe_type(&self, ty: &DebugType, unique: &str) -> DIType<'ctx> {
        let file = self.unit.get_file();
        let scope = self.unit.as_debug_info_scope();
        let bits = ty.size * 8;
        let alignment = (ty.align * 8) as u32;
        match &ty.kind {
            DebugTypeKind::Basic(encoding) => {
                let (base, encoding) = match encoding {
                    DebugEncoding::Boolean => ("bool".into(), 0x02),
                    DebugEncoding::Signed => (format!("int{bits}_t"), 0x05),
                    DebugEncoding::Unsigned => (format!("uint{bits}_t"), 0x07),
                    DebugEncoding::Float => {
                        ((if bits == 32 { "float" } else { "double" }).into(), 0x04)
                    }
                    DebugEncoding::Unicode => ("char32_t".into(), 0x10),
                    DebugEncoding::Address => ("uintptr_t".into(), 0x01),
                };
                let value = self
                    .builder
                    .create_basic_type(&base, bits, encoding, DIFlags::PUBLIC)
                    .expect("physical scalar names are nonempty")
                    .as_type();
                self.builder
                    .create_typedef(value, &ty.name, file, 0, scope, alignment)
                    .as_type()
            }
            DebugTypeKind::Pointer(pointee) => {
                let pointee = self.type_ref(pointee, &format!("{unique}.object"));
                let pointer = self
                    .builder
                    .create_pointer_type(
                        "",
                        pointee,
                        bits,
                        alignment,
                        inkwell::AddressSpace::default(),
                    )
                    .as_type();
                if ty.name.is_empty() {
                    pointer
                } else {
                    self.builder
                        .create_typedef(pointer, &ty.name, file, 0, scope, alignment)
                        .as_type()
                }
            }
            DebugTypeKind::Record(fields) => {
                let members = fields
                    .iter()
                    .enumerate()
                    .map(|(index, field)| {
                        let value = self.type_ref(&field.ty, &format!("{unique}.{index}"));
                        let shape = self.shape(&field.ty);
                        self.builder
                            .create_member_type(
                                scope,
                                &field.name,
                                file,
                                0,
                                shape.size * 8,
                                0,
                                field.offset * 8,
                                if field.artificial {
                                    DIFlags::ARTIFICIAL
                                } else {
                                    DIFlags::PUBLIC
                                },
                                value,
                            )
                            .as_type()
                    })
                    .collect::<Vec<_>>();
                self.builder
                    .create_struct_type(
                        scope,
                        &ty.name,
                        file,
                        0,
                        bits,
                        alignment,
                        DIFlags::PUBLIC,
                        None,
                        &members,
                        0,
                        None,
                        unique,
                    )
                    .as_type()
            }
            DebugTypeKind::Array { element, count } => {
                let element = self.type_ref(element, &format!("{unique}.element"));
                let dimension = 0..*count as i64;
                self.builder
                    .create_array_type(element, bits, alignment, std::slice::from_ref(&dimension))
                    .as_type()
            }
            DebugTypeKind::Enumeration { signed, variants } => {
                let underlying = self
                    .builder
                    .create_basic_type(
                        &format!("{}{bits}_t", if *signed { "int" } else { "uint" }),
                        bits,
                        if *signed { 0x05 } else { 0x07 },
                        DIFlags::PUBLIC,
                    )
                    .expect("enum scalar names are nonempty")
                    .as_type();
                let variants = variants
                    .iter()
                    .map(|(name, value)| {
                        let value = if *signed {
                            ((*value << (64 - bits)) as i64) >> (64 - bits)
                        } else {
                            *value as i64
                        };
                        self.builder.create_enumerator(name, value, !signed)
                    })
                    .collect::<Vec<_>>();
                self.builder
                    .create_enumeration_type(
                        scope, &ty.name, file, 0, bits, alignment, &variants, underlying,
                    )
                    .as_type()
            }
            DebugTypeKind::Function { parameters, result } => {
                let parameters = parameters
                    .iter()
                    .enumerate()
                    .map(|(i, ty)| self.type_ref(ty, &format!("{unique}.arg{i}")))
                    .collect::<Vec<_>>();
                let result = result
                    .as_ref()
                    .map(|ty| self.type_ref(ty, &format!("{unique}.result")));
                let signature =
                    self.builder
                        .create_subroutine_type(file, result, &parameters, DIFlags::PUBLIC);
                // DISubroutineType is a DIType in LLVM. Inkwell 0.10 omits
                // as_type() here; both wrappers are repr(transparent) over
                // the same non-null metadata pointer and context lifetime.
                unsafe {
                    std::mem::transmute::<inkwell::debug_info::DISubroutineType<'ctx>, DIType<'ctx>>(
                        signature,
                    )
                }
            }
        }
    }
}
