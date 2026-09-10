use super::*;

pub type FunctionId = Idx<Function>;
pub type ExternFunctionId = Idx<ExternFunction>;
pub type GlobalId = Idx<Global>;
pub type InitializationUnitId = Idx<InitializationUnit>;
pub type InitializationFailureRootId = Idx<InitializationFailureRoot>;
pub type ObjectId = Idx<ObjectDef>;
pub type ObjectTypeId = Idx<ObjectType>;
pub type SingletonValueId = Idx<SingletonValue>;
pub type SingletonPublishedRootId = Idx<SingletonPublishedRoot>;
pub type CallbackBridgeId = Idx<CallbackBridge>;
pub type ForeignCallbackAdapterId = Idx<ForeignCallbackAdapter>;
pub type ForeignCallbackFamilyId = Idx<ForeignCallbackFamily>;
pub type ForeignCallbackBridgeId = Idx<ForeignCallbackBridge>;
pub type FunctionTypeId = Idx<FunctionType>;
pub type ClosureClassId = Idx<ClosureClass>;
pub type ClosureInvokeFunctionId = Idx<ClosureInvokeFunction>;
pub type ClosureAdapterId = Idx<ClosureAdapter>;
pub type DynamicClosureAdapterId = Idx<DynamicClosureAdapter>;
pub type MonomorphizedFunctionId = Idx<MonomorphizedFunction>;
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
pub type CoroutineSavedValueId = Idx<CoroutineSavedValue>;
pub type CoroutineFailureValueId = Idx<CoroutineFailureValue>;

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

pub fn mangle_singleton_root(link_name: &str) -> String {
    format!("scoop.singleton.{link_name}")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceManglingTypeError {
    machine_kind: MachineScalarKind,
}

impl SourceManglingTypeError {
    pub const fn machine_kind(self) -> MachineScalarKind {
        self.machine_kind
    }
}

impl std::fmt::Display for SourceManglingTypeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "compiler-owned {} scalar has no compact source type code",
            self.machine_kind.name()
        )
    }
}

impl std::error::Error for SourceManglingTypeError {}

/// Mangle a monomorphized instance: `scoop.<name>$<encoded type args>`.
pub fn mangle_instance(
    module: &Module,
    name: &str,
    type_args: &[Type],
) -> Result<String, SourceManglingTypeError> {
    let args = type_args
        .iter()
        .map(|ty| encode_type(module, ty))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(format!("scoop.{name}${}", args.join("_")))
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
) -> Result<String, SourceManglingTypeError> {
    Ok(format!(
        "{}.g{generic_discriminator}",
        mangle_instance(module, name, type_args)?
    ))
}

/// Mangle one overload of a name shared by several functions (M7):
/// `scoop.<name>.<encoded params>` — `scoop.show.I32`,
/// `scoop.println.S`. Under compact-v2 an `Int` parameter is `I32`, not the
/// retired bare `I`; a zero-parameter overload gets an empty encoding
/// (`scoop.f.`). `.` introduces the overload encoding while `$` stays
/// reserved for monomorphized instances, so the two never collide.
pub fn mangle_overload(
    module: &Module,
    name: &str,
    params: &[Type],
) -> Result<String, SourceManglingTypeError> {
    Ok(format!("scoop.{name}.{}", encode_params(module, params)?))
}

/// Add the hidden coroutine-ABI discriminator to an already mangled source
/// callable. `$suspend` cannot collide with a source identifier or the `$`
/// type-argument encoding of a monomorphized function.
pub fn mangle_suspend(symbol: &str) -> String {
    format!("{symbol}$suspend")
}

/// The `_`-joined parameter encoding shared by overload mangling and
/// dispatch signature keys.
pub fn encode_params(module: &Module, params: &[Type]) -> Result<String, SourceManglingTypeError> {
    params
        .iter()
        .map(|t| encode_type(module, t))
        .collect::<Result<Vec<_>, _>>()
        .map(|parameters| parameters.join("_"))
}

fn encode_nominal_application(
    module: &Module,
    kind: char,
    stem: &NominalLinkStem,
    arguments: &[Type],
) -> Result<String, SourceManglingTypeError> {
    let stem = stem.as_str();
    let mut encoded = format!("{kind}{}_{}", stem.len(), stem);
    if !arguments.is_empty() {
        let arguments = arguments
            .iter()
            .map(|argument| encode_type(module, argument))
            .collect::<Result<Vec<_>, _>>()?;
        encoded.push('A');
        encoded.push_str(&arguments.join("_"));
    }
    encoded.push('X');
    Ok(encoded)
}

/// Compact-v2 type encoding for source signatures.
///
/// Compiler-owned machine scalars are rejected because they have generated
/// role identities rather than source type codes.
pub fn encode_type(module: &Module, ty: &Type) -> Result<String, SourceManglingTypeError> {
    Ok(match ty {
        Type::Unit => "U".to_string(),
        Type::Integer(kind) => kind.compact_v2_code().to_string(),
        Type::MachineScalar(kind) => {
            return Err(SourceManglingTypeError {
                machine_kind: *kind,
            });
        }
        Type::Boolean => "B".to_string(),
        Type::String => "S".to_string(),
        Type::Struct(id) => {
            let definition = &module.structs[*id];
            encode_nominal_application(
                module,
                'D',
                &definition.link_stem,
                &definition.type_arguments,
            )?
        }
        Type::Class(id) => match &module.classes[*id].representation {
            ClassRepresentation::Intrinsic(IntrinsicTypeRepresentation::Array { element }) => {
                format!("A{}X", encode_type(module, element)?)
            }
            ClassRepresentation::Intrinsic(IntrinsicTypeRepresentation::MutableArray {
                element,
            }) => format!("M{}X", encode_type(module, element)?),
            ClassRepresentation::Declared { .. }
            | ClassRepresentation::Intrinsic(IntrinsicTypeRepresentation::String) => {
                let definition = &module.classes[*id];
                encode_nominal_application(
                    module,
                    'C',
                    &definition.link_stem,
                    &definition.type_arguments,
                )?
            }
            ClassRepresentation::Intrinsic(_) => {
                unreachable!("the intrinsic registry fixes declaration targets")
            }
        },
        Type::Interface(id) => {
            let definition = &module.interfaces[*id];
            encode_nominal_application(
                module,
                'J',
                &definition.link_stem,
                &definition.type_arguments,
            )?
        }
        Type::Any => "Any".to_string(),
        Type::Tuple(elements) => {
            let inner = elements
                .iter()
                .map(|ty| encode_type(module, ty))
                .collect::<Result<Vec<_>, _>>()?;
            format!("T{}X", inner.join("_"))
        }
        Type::Function(id) => {
            let function = &module.function_types[*id];
            let kind = if function.is_suspend { "S" } else { "F" };
            let parameters = function
                .parameter_types
                .iter()
                .map(|ty| encode_type(module, ty))
                .collect::<Result<Vec<_>, _>>()?
                .join("_");
            format!(
                "{kind}{parameters}R{}X",
                encode_type(module, &function.return_type)?
            )
        }
        Type::Ptr(inner) => format!("P{}X", encode_type(module, inner)?),
        Type::FunPtr(id) => {
            let function = &module.function_types[*id];
            let parameters = function
                .parameter_types
                .iter()
                .map(|ty| encode_type(module, ty))
                .collect::<Result<Vec<_>, _>>()?
                .join("_");
            format!(
                "N{parameters}R{}X",
                encode_type(module, &function.return_type)?
            )
        }
        Type::Enum(id, args) => {
            let definition = &module.enums[*id];
            debug_assert_eq!(&definition.type_arguments, args);
            encode_nominal_application(module, 'E', &definition.link_stem, args)?
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nominal_link_stem(value: &str) -> NominalLinkStem {
        NominalLinkStem::from_session_local_encoding(value.to_string())
    }

    fn class(module: &mut Module, display: &str, stem: &str, arguments: Vec<Type>) -> ClassId {
        module.classes.alloc(ClassDef {
            modifier: ClassModifier::Final,
            link_stem: nominal_link_stem(stem),
            name: display.to_string(),
            type_arguments: arguments,
            representation: ClassRepresentation::Declared {
                fields: Vec::new(),
                base_class: None,
            },
            interfaces: Vec::new(),
            vtable: Vec::new(),
            itables: Vec::new(),
        })
    }

    fn structure(module: &mut Module, display: &str, stem: &str) -> StructId {
        module.structs.alloc(StructDef {
            link_stem: nominal_link_stem(stem),
            name: display.to_string(),
            type_arguments: Vec::new(),
            gc_free: true,
            representation: StructRepresentation::Declared {
                c_layout: None,
                interior_mutable: false,
                fields: Vec::new(),
            },
        })
    }

    fn enumeration(module: &mut Module, display: &str, stem: &str) -> EnumId {
        module.enums.alloc(EnumDef {
            link_stem: nominal_link_stem(stem),
            name: display.to_string(),
            type_arguments: Vec::new(),
            gc_free: true,
            variants: Vec::new(),
        })
    }

    fn interface(module: &mut Module, display: &str, stem: &str) -> InterfaceId {
        module.interfaces.alloc(InterfaceDef {
            link_stem: nominal_link_stem(stem),
            name: display.to_string(),
            type_arguments: Vec::new(),
            methods: Vec::new(),
        })
    }

    fn module() -> Module {
        let mut functions = Arena::new();
        let entry = functions.alloc(Function {
            gc_effect: GcEffect::Managed,
            name: "main".to_string(),
            symbol: ENTRY_SYMBOL.to_string(),
            params: Vec::new(),
            return_ty: Type::Unit,
            body: Body::unreachable(Arena::new()),
        });
        Module {
            functions,
            extern_functions: Arena::new(),
            globals: Arena::new(),
            initialization_units: Arena::new(),
            initialization_failure_roots: Arena::new(),
            objects: Arena::new(),
            object_types: Arena::new(),
            singleton_values: Arena::new(),
            singleton_published_roots: Arena::new(),
            callback_bridges: Arena::new(),
            foreign_callback_adapters: Arena::new(),
            foreign_callback_families: Arena::new(),
            foreign_callback_bridges: Arena::new(),
            function_types: Arena::new(),
            closure_classes: Arena::new(),
            closure_invoke_functions: Arena::new(),
            top_level: Vec::new(),
            strings: Arena::new(),
            structs: Arena::new(),
            enums: Arena::new(),
            classes: Arena::new(),
            interfaces: Arena::new(),
            option_core: Vec::new(),
            entry,
            meta: MirMeta::default(),
        }
    }

    #[test]
    fn compact_v2_integer_codes_are_used_by_source_mangling() {
        let module = module();
        for kind in IntegerKind::ALL {
            assert_eq!(
                encode_type(&module, &Type::Integer(kind)).as_deref(),
                Ok(kind.compact_v2_code())
            );
        }
        assert_eq!(
            mangle_overload(
                &module,
                "mix",
                &[
                    Type::Integer(IntegerKind::SIGNED_8),
                    Type::Integer(IntegerKind::UNSIGNED_64),
                ],
            )
            .as_deref(),
            Ok("scoop.mix.I8_V64")
        );
        assert!(crate::dump(&module).starts_with("Module\n"));
    }

    #[test]
    fn nominal_encodings_use_typed_stems_and_recursive_application_arguments() {
        let mut module = module();
        let package_a = class(&mut module, "Same", "$pkg$a$class$Same", Vec::new());
        let package_b = class(&mut module, "Same", "$pkg$b$class$Same", Vec::new());
        let a = Type::Class(package_a);
        let b = Type::Class(package_b);
        assert_ne!(
            encode_type(&module, &a).unwrap(),
            encode_type(&module, &b).unwrap()
        );
        assert_ne!(
            mangle_overload(&module, "pick", std::slice::from_ref(&a)).unwrap(),
            mangle_overload(&module, "pick", std::slice::from_ref(&b)).unwrap()
        );

        let generic_a = class(
            &mut module,
            "Box$Same",
            "$pkg$containers$class$Box",
            vec![a],
        );
        let generic_b = class(
            &mut module,
            "Box$Same",
            "$pkg$containers$class$Box",
            vec![b],
        );
        let generic_a = Type::Class(generic_a);
        let generic_b = Type::Class(generic_b);
        assert_ne!(
            encode_type(&module, &generic_a).unwrap(),
            encode_type(&module, &generic_b).unwrap()
        );
        assert_ne!(
            mangle_instance(&module, "identity", &[generic_a]).unwrap(),
            mangle_instance(&module, "identity", &[generic_b]).unwrap()
        );

        let struct_a = structure(&mut module, "Value", "$pkg$a$struct$Value");
        let struct_b = structure(&mut module, "Value", "$pkg$b$struct$Value");
        let enum_a = enumeration(&mut module, "Choice", "$pkg$a$enum$Choice");
        let enum_b = enumeration(&mut module, "Choice", "$pkg$b$enum$Choice");
        let interface_a = interface(&mut module, "View", "$pkg$a$interface$View");
        let interface_b = interface(&mut module, "View", "$pkg$b$interface$View");
        for (left, right) in [
            (Type::Struct(struct_a), Type::Struct(struct_b)),
            (
                Type::Enum(enum_a, Vec::new()),
                Type::Enum(enum_b, Vec::new()),
            ),
            (Type::Interface(interface_a), Type::Interface(interface_b)),
        ] {
            assert_ne!(
                encode_type(&module, &left).unwrap(),
                encode_type(&module, &right).unwrap()
            );
        }
    }

    #[test]
    fn machine_scalars_have_no_source_mangling_code() {
        let module = module();
        let error = encode_type(&module, &Type::MachineScalar(MachineScalarKind::EnumTag))
            .expect_err("enum tags are not source integers");
        assert_eq!(error.machine_kind(), MachineScalarKind::EnumTag);
        assert_eq!(
            error.to_string(),
            "compiler-owned enum-tag scalar has no compact source type code"
        );
    }

    #[test]
    fn dump_exposes_exact_integer_global() {
        let mut module = module();
        module.globals.alloc(Global {
            name: "maximum".to_string(),
            symbol: "scoop.global.maximum".to_string(),
            ty: Type::Integer(IntegerKind::UNSIGNED_64),
            mutable: false,
            storage: GlobalStorage::Local {
                thread_local: false,
                initial_state: MirStaticInitialState::EncodedStaticValue {
                    payload: MirConstantImage::Integer(MirIntegerConstant::Unsigned64(u64::MAX)),
                },
            },
        });

        let dump = crate::dump(&module);
        assert!(dump.starts_with("Module\n"), "{dump}");
        assert!(
            dump.contains(
                "@scoop.global.maximum maximum: ULong global initial=encoded(ULong:0xffffffffffffffff)"
            ),
            "{dump}"
        );
    }
}
