//! Immutable objects use ordinary array and struct layouts, with static references.

use super::*;

pub(super) struct ImmortalEmitter<'a, 'ctx> {
    pub context: &'ctx Context,
    pub llvm: &'a LlvmModule<'ctx>,
    pub module: &'a Module,
    pub target_data: &'a inkwell::targets::TargetData,
    pub descriptors: TypeDescriptorGlobals<'a, 'ctx>,
    pub managed_address_space: ManagedAddressSpace,
    pub globals: &'a BTreeMap<scoop_lir::GlobalId, GlobalValue<'ctx>>,
    pub define: bool,
}

impl<'ctx> ImmortalEmitter<'_, 'ctx> {
    pub(super) fn emit(&self, global: &Global) -> Result<GlobalValue<'ctx>, CodegenError> {
        let context = self.context;
        let (identity, descriptor, count, payload_type, payload) = match &global.init {
            GlobalInit::StringConst { identity, .. }
            | GlobalInit::ByteArrayConst { identity, .. } => {
                let (bytes, descriptor) = match &global.init {
                    GlobalInit::StringConst { value, .. } => (
                        value.as_bytes(),
                        self.module.meta.well_known_type_descriptors.string,
                    ),
                    GlobalInit::ByteArrayConst {
                        array_type, bytes, ..
                    } => (
                        bytes.as_slice(),
                        self.module.meta.arrays[*array_type].type_descriptor,
                    ),
                    _ => unreachable!("only immutable byte payloads enter this branch"),
                };
                let length = llvm_length(bytes.len())?;
                (
                    identity,
                    descriptor,
                    bytes.len(),
                    context.i8_type().array_type(length),
                    self.define.then(|| context.const_string(bytes, false)),
                )
            }
            GlobalInit::EmbeddedFileArrayConst {
                identity,
                array_type,
                files,
            } => {
                let array = &self.module.meta.arrays[*array_type];
                let LirType::Struct(struct_id) = array.element else {
                    return Err(CodegenError(
                        "embedded file arrays require ordinary struct elements".into(),
                    ));
                };
                let element_type = basic_ty(
                    context,
                    &self.module.structs,
                    &self.module.enums,
                    self.managed_address_space,
                    &array.element,
                )?
                .into_struct_type();
                let length = llvm_length(files.len())?;
                let payload = if self.define {
                    let elements = files
                        .iter()
                        .map(|file| {
                            let fields = [file.path, file.bytes].map(|global| {
                                LirConstantImage::GlobalPointer {
                                    global,
                                    kind: PointerKind::Managed,
                                }
                            });
                            llvm_constant(
                                context,
                                &self.module.structs,
                                &self.module.enums,
                                self.globals,
                                self.managed_address_space,
                                self.target_data,
                                &array.element,
                                &LirConstantImage::Struct {
                                    struct_id,
                                    fields: fields.into(),
                                },
                            )
                            .map(BasicValueEnum::into_struct_value)
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    Some(element_type.const_array(&elements))
                } else {
                    None
                };
                (
                    identity,
                    array.type_descriptor,
                    files.len(),
                    element_type.array_type(length),
                    payload,
                )
            }
            _ => unreachable!("immortal emission only receives immutable object definitions"),
        };
        let pointer = context.ptr_type(AddressSpace::default());
        let word = context.i64_type();
        let ty = context.struct_type(
            &[
                pointer.into(),
                word.into(),
                word.into(),
                payload_type.into(),
            ],
            false,
        );
        let value = self.llvm.add_global(
            ty,
            Some(self.managed_address_space.inkwell()),
            global.symbol(),
        );
        value.set_constant(true);
        apply_persistent_linkage(&value, identity.symbol_request(), self.define)?;
        if let Some(payload) = payload {
            let descriptor = type_descriptor_global(descriptor, self.descriptors)?;
            value.set_initializer(&ty.const_named_struct(&[
                descriptor.as_pointer_value().into(),
                word.const_zero().into(),
                word.const_int(count as u64, false).into(),
                payload.into(),
            ]));
        }
        Ok(value)
    }
}

fn llvm_length(length: usize) -> Result<u32, CodegenError> {
    u32::try_from(length)
        .map_err(|_| CodegenError("static payload exceeds LLVM constant array length".into()))
}
