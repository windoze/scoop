use super::*;

#[derive(Clone, Copy)]
pub(super) enum LoweredExternFunctionRef {
    C(lir::CExternFunctionRef),
    Scoop(lir::ScoopExternFunctionRef),
}

pub(super) fn lower_extern_functions(
    context: &LoweringContext,
    module: &mir::Module,
    structs: &lir::StructDefs,
    enums: &lir::EnumDefs,
    native_externals: &lir::NativeExternalMetadata,
) -> StorageResult<(
    lir::ExternFunctions,
    HashMap<mir::ExternFunctionId, LoweredExternFunctionRef>,
)> {
    let mut functions = lir::ExternFunctions::default();
    let mut references = HashMap::new();
    for (id, extern_) in module.extern_functions.iter() {
        let identity = || lir::ExternFunctionIdentity {
            source_name: extern_.source_name.clone(),
            native_symbol: extern_.native_symbol.clone(),
            library: extern_.library.clone(),
            calling_convention: match extern_.calling_convention {
                mir::CallingConvention::Cdecl => lir::CallingConvention::Cdecl,
            },
        };
        let reference = match extern_.abi {
            mir::ExternAbi::C => LoweredExternFunctionRef::C(
                functions.alloc_c(lir::CExternFunction {
                    identity: identity(),
                    bridge: lir::GeneratedBridgeEntryIdentity::new(
                        module.cone,
                        scoop_identity::GeneratedBridgeUnitKey::OutboundFunction(
                            native_externals
                                .contract(extern_.source_contract.id())
                                .expect("every C extern has one normalized target contract")
                                .fingerprint(),
                        ),
                    )
                    .expect("validated C extern bridge identities are encodable"),
                    signature: lir::CFunctionType {
                        params: extern_
                            .params
                            .iter()
                            .map(|ty| c_ffi_type(module, structs, enums, ty))
                            .collect(),
                        return_type: c_return_type(module, structs, enums, &extern_.return_type),
                    },
                }),
            ),
            mir::ExternAbi::Scoop => {
                LoweredExternFunctionRef::Scoop(functions.alloc_scoop(lir::ScoopExternFunction {
                    identity: identity(),
                    gc_effect: match extern_.gc_effect {
                        mir::GcEffect::Managed => lir::GcEffect::Managed,
                        mir::GcEffect::NoGc => lir::GcEffect::NoGc,
                    },
                    signature: abi::classify_mir_signature(
                        context,
                        module,
                        extern_.params.iter(),
                        &extern_.return_type,
                        structs,
                        enums,
                    )?,
                }))
            }
        };
        references.insert(id, reference);
    }
    Ok((functions, references))
}

fn c_data_pointee(
    module: &mir::Module,
    structs: &lir::StructDefs,
    enums: &lir::EnumDefs,
    pointee: &mir::Type,
) -> lir::CDataPointee {
    if pointee == &mir::Type::Unit {
        lir::CDataPointee::OpaqueVoid
    } else {
        lir::CDataPointee::Object(Box::new(c_ffi_type(module, structs, enums, pointee)))
    }
}

fn c_function_type(
    module: &mir::Module,
    structs: &lir::StructDefs,
    enums: &lir::EnumDefs,
    signature: mir::FunctionTypeId,
) -> lir::CFunctionType {
    let signature = &module.function_types[signature];
    lir::CFunctionType {
        params: signature
            .parameter_types
            .iter()
            .map(|ty| c_ffi_type(module, structs, enums, ty))
            .collect(),
        return_type: c_return_type(module, structs, enums, &signature.return_type),
    }
}

pub(super) fn c_return_type(
    module: &mir::Module,
    structs: &lir::StructDefs,
    enums: &lir::EnumDefs,
    ty: &mir::Type,
) -> lir::CReturnType {
    if ty == &mir::Type::Unit {
        lir::CReturnType::Void
    } else {
        lir::CReturnType::Value(Box::new(c_ffi_type(module, structs, enums, ty)))
    }
}

pub(super) fn exact_option_payload<'a>(
    module: &'a mir::Module,
    id: mir::EnumId,
    arguments: &'a [mir::Type],
) -> &'a mir::Type {
    let option = module
        .option_core(id)
        .expect("the caller established exact core Option identity");
    assert_eq!(
        option.enum_id(),
        id,
        "Option refinement belongs to this exact enum specialization",
    );
    let some = option
        .some()
        .definition(&module.enums)
        .expect("core Option Some variant exists");
    let none = option
        .none()
        .definition(&module.enums)
        .expect("core Option None variant exists");
    let [argument] = arguments else {
        panic!("core Option specialization has exactly one type argument")
    };
    let field = option
        .some_payload()
        .definition(&module.enums)
        .expect("core Option Some payload exists");
    assert_eq!(some.fields.len(), 1, "core Option Some has one field");
    assert!(none.fields.is_empty(), "core Option None has no fields");
    assert_eq!(
        &field.ty, argument,
        "core Option Some payload exactly matches its type argument",
    );
    argument
}

pub(super) fn c_ffi_type(
    module: &mir::Module,
    structs: &lir::StructDefs,
    enums: &lir::EnumDefs,
    ty: &mir::Type,
) -> lir::CType {
    match ty {
        mir::Type::Unit => unreachable!("Unit has no C object representation"),
        mir::Type::Integer(kind) => lir::CType::Integer(integer_kind(*kind)),
        mir::Type::MachineScalar(kind) => {
            unreachable!("internal machine scalar {kind:?} cannot cross source C FFI")
        }
        mir::Type::Boolean => lir::CType::Boolean,
        mir::Type::Ptr(pointee) => lir::CType::DataPointer {
            pointee: c_data_pointee(module, structs, enums, pointee),
            storage: lir::CDataPointerStorage::Direct,
        },
        mir::Type::FunPtr(signature) => lir::CType::CodePointer {
            signature: Box::new(c_function_type(module, structs, enums, *signature)),
            storage: lir::CCodePointerStorage::Direct,
        },
        mir::Type::Struct(id) => {
            if let mir::StructRepresentation::Intrinsic(mir::IntrinsicTypeRepresentation::Float(
                kind,
            )) = module.structs[*id].representation
            {
                return lir::CType::Float(kind);
            }
            if matches!(
                module.structs[*id].representation,
                mir::StructRepresentation::Intrinsic(mir::IntrinsicTypeRepresentation::Char)
            ) {
                return lir::CType::Integer(lir::IntegerKind::UNSIGNED_32);
            }
            if matches!(
                module.structs[*id].representation,
                mir::StructRepresentation::Declared {
                    c_abi: mir::StructCAbi::UInt64Field { .. },
                    ..
                }
            ) {
                lir::CType::Integer(lir::IntegerKind::UNSIGNED_64)
            } else {
                lir::CType::Struct(
                    structs
                        .c_ref(struct_def_id(*id))
                        .expect("C struct storage requires an explicit C layout"),
                )
            }
        }
        mir::Type::Enum(id, args) if module.option_core(*id).is_some() => {
            match exact_option_payload(module, *id, args) {
                mir::Type::Ptr(pointee) => {
                    let pointee = c_data_pointee(module, structs, enums, pointee);
                    let reference = enums
                        .nullable_data_pointer_ref(enum_def_id(*id), pointee.clone())
                        .expect("nullable C data pointers require a raw-pointer niche enum");
                    lir::CType::DataPointer {
                        pointee,
                        storage: lir::CDataPointerStorage::Nullable(reference),
                    }
                }
                mir::Type::FunPtr(signature) => {
                    let signature = c_function_type(module, structs, enums, *signature);
                    let reference = enums
                        .nullable_code_pointer_ref(enum_def_id(*id), signature.clone())
                        .expect("nullable C code pointers require a code-pointer niche enum");
                    lir::CType::CodePointer {
                        signature: Box::new(signature),
                        storage: lir::CCodePointerStorage::Nullable(reference),
                    }
                }
                other => unreachable!(
                    "core Option payload {} is not C-nullable",
                    mir::type_name(module, other)
                ),
            }
        }
        other => unreachable!(
            "HIR C-FFI classification rejects {} before MIR",
            mir::type_name(module, other)
        ),
    }
}
