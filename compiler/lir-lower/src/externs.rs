use super::*;

#[derive(Clone, Copy)]
pub(super) enum LoweredExternFunctionRef {
    C(lir::CExternFunctionRef),
    Scoop(lir::ScoopExternFunctionRef),
}

pub(super) fn lower_extern_functions(
    module: &mir::Module,
) -> (
    lir::ExternFunctions,
    HashMap<mir::ExternFunctionId, LoweredExternFunctionRef>,
) {
    let mut functions = lir::ExternFunctions::default();
    let mut references = HashMap::new();
    for (id, extern_) in module.extern_functions.iter() {
        let declaration = lir::ExternFunctionDeclaration {
            source_name: extern_.source_name.clone(),
            native_symbol: extern_.native_symbol.clone(),
            library: extern_.library.clone(),
            calling_convention: match extern_.calling_convention {
                mir::CallingConvention::Cdecl => lir::CallingConvention::Cdecl,
            },
            params: extern_.params.iter().map(lir_type).collect(),
            return_type: lir_type(&extern_.return_type),
        };
        let reference = match extern_.abi {
            mir::ExternAbi::C => LoweredExternFunctionRef::C(
                functions.alloc_c(lir::CExternFunction {
                    declaration,
                    bridge_symbol: format!("scoop_c_bridge_{}", id.into_raw().into_u32()),
                    params: extern_
                        .params
                        .iter()
                        .map(|ty| c_ffi_type(module, ty))
                        .collect(),
                    return_type: c_ffi_type(module, &extern_.return_type),
                }),
            ),
            mir::ExternAbi::Scoop => {
                LoweredExternFunctionRef::Scoop(functions.alloc_scoop(lir::ScoopExternFunction {
                    declaration,
                    gc_effect: match extern_.gc_effect {
                        mir::GcEffect::Managed => lir::GcEffect::Managed,
                        mir::GcEffect::NoGc => lir::GcEffect::NoGc,
                    },
                }))
            }
        };
        references.insert(id, reference);
    }
    (functions, references)
}

pub(super) fn c_ffi_type(module: &mir::Module, ty: &mir::Type) -> lir::CType {
    match ty {
        mir::Type::Unit => lir::CType::Unit,
        mir::Type::Int => lir::CType::Int,
        mir::Type::UInt => lir::CType::UInt,
        mir::Type::MachineScalar(kind) => {
            unreachable!("internal machine scalar {kind:?} cannot cross source C FFI")
        }
        mir::Type::Boolean => lir::CType::Boolean,
        mir::Type::Ptr(_) => lir::CType::Pointer,
        mir::Type::FunPtr(signature) => {
            let signature = &module.function_types[*signature];
            lir::CType::FunctionPointer {
                params: signature
                    .parameter_types
                    .iter()
                    .map(|ty| c_ffi_type(module, ty))
                    .collect(),
                return_type: Box::new(c_ffi_type(module, &signature.return_type)),
            }
        }
        mir::Type::Struct(id) => lir::CType::Struct(struct_def_id(*id)),
        mir::Type::Enum(_, args)
            if matches!(args.as_slice(), [mir::Type::Ptr(_) | mir::Type::FunPtr(_)]) =>
        {
            c_ffi_type(module, &args[0])
        }
        other => unreachable!(
            "HIR C-FFI classification rejects {} before MIR",
            mir::type_name(module, other)
        ),
    }
}
