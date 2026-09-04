use super::*;

pub type FunctionId = Idx<Function>;
pub type ExternFunctionId = Idx<ExternFunction>;
pub type GlobalId = Idx<Global>;
pub type InitializationUnitId = Idx<InitializationUnit>;
pub type InitializationFailureRootId = Idx<InitializationFailureRoot>;
pub type CallbackBridgeId = Idx<CallbackBridge>;
pub type ForeignCallbackAdapterId = Idx<ForeignCallbackAdapter>;
pub type ForeignCallbackBridgeId = Idx<ForeignCallbackBridge>;
pub type FunctionTypeId = Idx<FunctionType>;
pub type ClosureClassId = Idx<ClosureClass>;
pub type ClosureInvokeFunctionId = Idx<ClosureInvokeFunction>;
pub type ClosureAdapterId = Idx<ClosureAdapter>;
pub type DynamicClosureAdapterId = Idx<DynamicClosureAdapter>;
pub type MonomorphizedFunctionId = Idx<MonomorphizedFunction>;
pub type GenericFunctionSourceId = Idx<GenericFunctionSource>;
pub type ParameterizedMethodSourceId = Idx<ParameterizedMethodSource>;
pub type GenericMethodSourceId = Idx<GenericMethodSource>;
pub type StringConstId = Idx<StringConst>;
pub type StructId = Idx<StructDef>;
pub type EnumId = Idx<EnumDef>;
pub type ClassId = Idx<ClassDef>;
pub type InterfaceId = Idx<InterfaceDef>;
pub type LocalId = Idx<Local>;
pub type BlockId = Idx<BasicBlock>;
pub type CoroutineFunctionId = Idx<CoroutineFunction>;
pub type CoroutineStepId = Idx<CoroutineStep>;
pub type CoroutineSlotId = Idx<CoroutineSlot>;
pub type CoroutineFrameId = Idx<CoroutineFrame>;
pub type CoroutineResumePointId = Idx<CoroutineResumePoint>;

/// Mangled symbol of the program entry point (called by the C runtime).
pub const ENTRY_SYMBOL: &str = "scoop_main";

/// Mangle a user function name (entry point maps to `ENTRY_SYMBOL`).
pub fn mangle_function(name: &str, is_entry: bool) -> String {
    if is_entry {
        ENTRY_SYMBOL.to_string()
    } else {
        format!("scoop.{name}")
    }
}

pub fn mangle_global(name: &str) -> String {
    format!("scoop.global.{name}")
}

/// Mangle a monomorphized instance: `scoop.<name>$<encoded type args>`.
pub fn mangle_instance(module: &Module, name: &str, type_args: &[Type]) -> String {
    let args: Vec<String> = type_args.iter().map(|t| encode_type(module, t)).collect();
    format!("scoop.{name}${}", args.join("_"))
}

/// Mangle a monomorphized instance when several generic definitions
/// share the same qualified name. The generic-definition discriminator
/// is local to the Cone and only appears for such overload groups, so
/// the ordinary compact instance symbol remains unchanged.
pub fn mangle_generic_overload(
    module: &Module,
    name: &str,
    type_args: &[Type],
    generic_discriminator: u32,
) -> String {
    format!(
        "{}.g{generic_discriminator}",
        mangle_instance(module, name, type_args)
    )
}

/// Mangle one overload of a name shared by several functions (M7):
/// `scoop.<name>.<encoded params>` — `scoop.show.I`,
/// `scoop.println.S`; a zero-parameter overload gets an empty encoding
/// (`scoop.f.`). `.` introduces the overload encoding while `$` stays
/// reserved for monomorphized instances, so the two never collide.
pub fn mangle_overload(module: &Module, name: &str, params: &[Type]) -> String {
    format!("scoop.{name}.{}", encode_params(module, params))
}

/// Add the hidden coroutine-ABI discriminator to an already mangled source
/// callable. `$suspend` cannot collide with a source identifier or the `$`
/// type-argument encoding of a monomorphized function.
pub fn mangle_suspend(symbol: &str) -> String {
    format!("{symbol}$suspend")
}

/// The `_`-joined parameter encoding shared by overload mangling and
/// dispatch signature keys.
pub fn encode_params(module: &Module, params: &[Type]) -> String {
    params
        .iter()
        .map(|t| encode_type(module, t))
        .collect::<Vec<_>>()
        .join("_")
}

/// Compact type encoding for mangling (e.g. `scoop.identity$I`).
pub fn encode_type(module: &Module, ty: &Type) -> String {
    match ty {
        Type::Unit => "U".to_string(),
        Type::Int => "I".to_string(),
        Type::UInt => "V".to_string(),
        Type::Boolean => "B".to_string(),
        Type::String => "S".to_string(),
        Type::Struct(id) => {
            let name = &module.structs[*id].name;
            format!("D{}_{}X", name.len(), name)
        }
        Type::Class(id) => match &module.classes[*id].representation {
            ClassRepresentation::Intrinsic(IntrinsicTypeRepresentation::Array { element }) => {
                format!("A{}X", encode_type(module, element))
            }
            ClassRepresentation::Intrinsic(IntrinsicTypeRepresentation::MutableArray {
                element,
            }) => format!("M{}X", encode_type(module, element)),
            ClassRepresentation::Declared { .. }
            | ClassRepresentation::Intrinsic(IntrinsicTypeRepresentation::String) => {
                let name = &module.classes[*id].name;
                format!("C{}_{}X", name.len(), name)
            }
            ClassRepresentation::Intrinsic(_) => {
                unreachable!("the intrinsic registry fixes declaration targets")
            }
        },
        Type::Interface(id) => {
            let name = &module.interfaces[*id].name;
            format!("J{}_{}X", name.len(), name)
        }
        Type::Any => "Any".to_string(),
        Type::Tuple(elements) => {
            let inner: Vec<String> = elements.iter().map(|t| encode_type(module, t)).collect();
            format!("T{}X", inner.join("_"))
        }
        Type::Function(id) => {
            let function = &module.function_types[*id];
            let kind = if function.is_suspend { "S" } else { "F" };
            let parameters = function
                .parameter_types
                .iter()
                .map(|ty| encode_type(module, ty))
                .collect::<Vec<_>>()
                .join("_");
            format!(
                "{kind}{parameters}R{}X",
                encode_type(module, &function.return_type)
            )
        }
        Type::Ptr(inner) => format!("P{}X", encode_type(module, inner)),
        Type::FunPtr(id) => {
            let function = &module.function_types[*id];
            let parameters = function
                .parameter_types
                .iter()
                .map(|ty| encode_type(module, ty))
                .collect::<Vec<_>>()
                .join("_");
            format!(
                "N{parameters}R{}X",
                encode_type(module, &function.return_type)
            )
        }
        Type::Enum(id, args) => {
            let name = &module.enums[*id].name;
            if args.is_empty() {
                format!("E{}_{}X", name.len(), name)
            } else {
                let inner: Vec<String> = args.iter().map(|t| encode_type(module, t)).collect();
                format!("E{}_{}A{}X", name.len(), name, inner.join("_"))
            }
        }
    }
}
