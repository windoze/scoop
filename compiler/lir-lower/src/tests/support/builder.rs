//! MIR module fixture builder shared by LIR lowering tests.

use super::*;

/// MIR module shell as mir-lower produces it.
pub(in crate::tests) struct Builder {
    pub(in crate::tests) functions: Arena<mir::Function>,
    pub(in crate::tests) extern_functions: Arena<mir::ExternFunction>,
    pub(in crate::tests) strings: Arena<mir::StringConst>,
    pub(in crate::tests) structs: Arena<mir::StructDef>,
    pub(in crate::tests) enums: Arena<mir::EnumDef>,
    pub(in crate::tests) classes: Arena<mir::ClassDef>,
    pub(in crate::tests) interfaces: Arena<mir::InterfaceDef>,
    pub(in crate::tests) function_types: Arena<mir::FunctionType>,
    pub(in crate::tests) option_core: Vec<mir::OptionCore>,
    pub(in crate::tests) top_level: Vec<mir::FunctionId>,
}

impl Builder {
    pub(in crate::tests) fn new() -> Self {
        Builder {
            functions: Arena::new(),
            extern_functions: Arena::new(),
            strings: Arena::new(),
            structs: Arena::new(),
            enums: Arena::new(),
            classes: Arena::new(),
            interfaces: Arena::new(),
            function_types: Arena::new(),
            option_core: Vec::new(),
            top_level: Vec::new(),
        }
    }

    /// `enum Option<T> { Some(T), None }` instantiated at
    /// `payload`, named as mir-lower names its instances.
    pub(in crate::tests) fn option_enum(&mut self, name: &str, payload: mir::Type) -> mir::EnumId {
        let payload_gc_free = self.type_gc_free(&payload);
        let mut variants = Vec::new();
        let some_index = u32::try_from(variants.len()).expect("test enum arity fits u32");
        let mut some_fields = Vec::new();
        let some_payload_index =
            u32::try_from(some_fields.len()).expect("test field arity fits u32");
        some_fields.push(mir::Field {
            name: "_1".to_string(),
            ty: payload.clone(),
        });
        variants.push(mir::VariantDef {
            name: "Some".to_string(),
            gc_free: payload_gc_free,
            fields: some_fields,
        });
        let none_index = u32::try_from(variants.len()).expect("test enum arity fits u32");
        variants.push(mir::VariantDef {
            name: "None".to_string(),
            gc_free: true,
            fields: Vec::new(),
        });
        let id = self.enums.alloc(mir::EnumDef {
            name: name.to_string(),
            type_arguments: vec![payload],
            gc_free: payload_gc_free,
            variants,
        });
        let some = mir::MirVariantRef::new(&self.enums, id, some_index).expect("Some variant");
        let some_payload = mir::MirVariantFieldRef::new(&self.enums, some, some_payload_index)
            .expect("Some payload");
        let none = mir::MirVariantRef::new(&self.enums, id, none_index).expect("None variant");
        self.option_core.push(
            mir::OptionCore::checked(&self.enums, some_payload, none)
                .expect("test Option metadata matches its enum"),
        );
        id
    }

    pub(in crate::tests) fn type_gc_free(&self, ty: &mir::Type) -> bool {
        match ty {
            mir::Type::Unit
            | mir::Type::Integer(_)
            | mir::Type::MachineScalar(_)
            | mir::Type::Boolean
            | mir::Type::Ptr(_)
            | mir::Type::FunPtr(_) => true,
            mir::Type::String
            | mir::Type::Class(_)
            | mir::Type::Interface(_)
            | mir::Type::Any
            | mir::Type::Function(_) => false,
            mir::Type::Struct(id) => self.structs[*id].gc_free,
            mir::Type::Enum(id, _) => self.enums[*id].gc_free,
            mir::Type::Tuple(elements) => elements.iter().all(|element| self.type_gc_free(element)),
        }
    }

    pub(in crate::tests) fn string(&mut self, value: &str) -> mir::StringConstId {
        let ordinal = u32::try_from(self.strings.len()).expect("test string count fits u32");
        let identity = mir::ImmortalObjectKey::string_constant(
            mir::ImmortalObjectOwner::Property(property_owner("testStringConstants")),
            mir::StructuralDefinitionPath::from_first(
                mir::StructuralPathSegment::new(
                    mir::StructuralDefinitionSiteRole::StringConstant,
                    ordinal,
                ),
                [],
            ),
        );
        self.strings.alloc(mir::StringConst {
            identity,
            value: value.to_string(),
        })
    }

    pub(in crate::tests) fn managed_scoop_extern(
        &mut self,
        source_name: &str,
        native_symbol: &str,
        params: Vec<mir::Type>,
        return_type: mir::Type,
    ) -> mir::ExternFunctionId {
        self.extern_functions.alloc(mir::ExternFunction {
            source_contract: test_source_native_contract(
                source_name,
                native_symbol,
                mir::ExternAbi::Scoop,
            ),
            source_name: source_name.to_string(),
            native_symbol: native_symbol.to_string(),
            library: String::new(),
            abi: mir::ExternAbi::Scoop,
            calling_convention: mir::CallingConvention::Cdecl,
            gc_effect: mir::GcEffect::Managed,
            params,
            return_type,
        })
    }

    pub(in crate::tests) fn c_extern(
        &mut self,
        source_name: &str,
        native_symbol: &str,
        params: Vec<mir::Type>,
        return_type: mir::Type,
    ) -> mir::ExternFunctionId {
        self.extern_functions.alloc(mir::ExternFunction {
            source_contract: test_source_native_contract(
                source_name,
                native_symbol,
                mir::ExternAbi::C,
            ),
            source_name: source_name.to_string(),
            native_symbol: native_symbol.to_string(),
            library: String::new(),
            abi: mir::ExternAbi::C,
            calling_convention: mir::CallingConvention::Cdecl,
            gc_effect: mir::GcEffect::Managed,
            params,
            return_type,
        })
    }

    pub(in crate::tests) fn strukt(
        &mut self,
        name: &str,
        fields: &[(&str, mir::Type)],
    ) -> mir::StructId {
        let gc_free = fields.iter().all(|(_, ty)| self.type_gc_free(ty));
        self.structs.alloc(mir::StructDef {
            name: name.to_string(),
            type_arguments: Vec::new(),
            gc_free,
            representation: mir::StructRepresentation::Declared {
                c_layout: None,
                interior_mutable: false,
                fields: fields
                    .iter()
                    .map(|(field_name, ty)| mir::DeclaredStructField {
                        identity: test_field_identity(name, field_name),
                        name: field_name.to_string(),
                        ty: ty.clone(),
                    })
                    .collect(),
            },
        })
    }

    pub(in crate::tests) fn c_strukt(
        &mut self,
        name: &str,
        aligned: mir::MirCLayoutValue,
        packed: mir::MirCLayoutValue,
        interior_mutable: bool,
        fields: &[(&str, mir::Type)],
    ) -> mir::StructId {
        let gc_free = fields.iter().all(|(_, ty)| self.type_gc_free(ty));
        self.structs.alloc(mir::StructDef {
            name: name.to_string(),
            type_arguments: Vec::new(),
            gc_free,
            representation: mir::StructRepresentation::Declared {
                c_layout: Some(mir::MirCLayoutContract { aligned, packed }),
                interior_mutable,
                fields: fields
                    .iter()
                    .map(|(field_name, ty)| mir::DeclaredStructField {
                        identity: test_field_identity(name, field_name),
                        name: field_name.to_string(),
                        ty: ty.clone(),
                    })
                    .collect(),
            },
        })
    }

    pub(in crate::tests) fn interface(&mut self, name: &str, methods: &[&str]) -> mir::InterfaceId {
        let methods = methods
            .iter()
            .map(|method| {
                self.functions.alloc(mir::Function {
                    gc_effect: mir::GcEffect::Managed,
                    name: format!("{name}.{method}"),
                    params: Vec::new(),
                    return_ty: mir::Type::Unit,
                    body: mir::Body::unreachable(Arena::new()),
                })
            })
            .collect();
        self.interfaces.alloc(mir::InterfaceDef {
            name: name.to_string(),
            type_arguments: Vec::new(),
            methods,
        })
    }

    pub(in crate::tests) fn class(
        &mut self,
        name: &str,
        base: Option<mir::ClassId>,
        fields: &[(&str, mir::Type)],
        vtable: Vec<mir::TableSlot>,
        itables: Vec<mir::ItableRecord>,
    ) -> mir::ClassId {
        self.classes.alloc(mir::ClassDef {
            modifier: mir::ClassModifier::Final,
            name: name.to_string(),
            type_arguments: Vec::new(),
            representation: mir::ClassRepresentation::Declared {
                fields: fields
                    .iter()
                    .map(|(name, ty)| mir::Field {
                        name: name.to_string(),
                        ty: ty.clone(),
                    })
                    .collect(),
                base_class: base,
            },
            interfaces: Vec::new(),
            vtable,
            itables,
        })
    }

    pub(in crate::tests) fn array_class(
        &mut self,
        name: &str,
        kind: mir::ArrayKind,
        element: mir::Type,
    ) -> mir::ClassId {
        self.classes.alloc(mir::ClassDef {
            modifier: mir::ClassModifier::Final,
            name: name.to_string(),
            type_arguments: Vec::new(),
            representation: mir::ClassRepresentation::Intrinsic(match kind {
                mir::ArrayKind::Immutable => mir::IntrinsicTypeRepresentation::Array { element },
                mir::ArrayKind::Mutable => {
                    mir::IntrinsicTypeRepresentation::MutableArray { element }
                }
            }),
            interfaces: Vec::new(),
            vtable: Vec::new(),
            itables: Vec::new(),
        })
    }

    pub(in crate::tests) fn array(&mut self, name: &str, element: mir::Type) -> mir::Type {
        mir::Type::Class(self.array_class(name, mir::ArrayKind::Immutable, element))
    }

    pub(in crate::tests) fn mutable_array(&mut self, name: &str, element: mir::Type) -> mir::Type {
        mir::Type::Class(self.array_class(name, mir::ArrayKind::Mutable, element))
    }

    /// A function that exists only as a signature (e.g. an
    /// interface method shell): not pushed to `top_level`, so it
    /// is never emitted.
    pub(in crate::tests) fn decl_fn(
        &mut self,
        name: &str,
        params: Vec<mir::Param>,
        return_ty: mir::Type,
    ) -> mir::FunctionId {
        self.functions.alloc(mir::Function {
            gc_effect: mir::GcEffect::Managed,
            name: name.to_string(),
            params,
            return_ty,
            body: mir::Body::unreachable(Arena::new()),
        })
    }

    pub(in crate::tests) fn user_fn(
        &mut self,
        name: &str,
        locals: Arena<mir::Local>,
        statements: Vec<mir::Statement>,
    ) -> mir::FunctionId {
        self.user_fn_full(name, Vec::new(), mir::Type::Unit, locals, statements)
    }

    pub(in crate::tests) fn user_fn_full(
        &mut self,
        name: &str,
        params: Vec<mir::Param>,
        return_ty: mir::Type,
        locals: Arena<mir::Local>,
        statements: Vec<mir::Statement>,
    ) -> mir::FunctionId {
        let mut blocks = Arena::new();
        let terminator = if return_ty == mir::Type::Unit {
            mir::Terminator::Return { value: None }
        } else {
            mir::Terminator::Unreachable
        };
        let entry = blocks.alloc(mir::BasicBlock {
            name: "entry".to_string(),
            statements,
            terminator,
            unwind: None,
        });
        let id = self.functions.alloc(mir::Function {
            gc_effect: mir::GcEffect::Managed,
            name: name.to_string(),
            params,
            return_ty,
            body: mir::Body {
                locals,
                blocks,
                entry,
                loop_header_polls: Vec::new(),
            },
        });
        self.top_level.push(id);
        id
    }

    pub(in crate::tests) fn user_fn_body(
        &mut self,
        name: &str,
        params: Vec<mir::Param>,
        return_ty: mir::Type,
        body: mir::Body,
    ) -> mir::FunctionId {
        let id = self.functions.alloc(mir::Function {
            gc_effect: mir::GcEffect::Managed,
            name: name.to_string(),
            params,
            return_ty,
            body,
        });
        self.top_level.push(id);
        id
    }

    pub(in crate::tests) fn main(
        &mut self,
        locals: Arena<mir::Local>,
        statements: Vec<mir::Statement>,
    ) -> mir::FunctionId {
        self.user_fn("main", locals, statements)
    }

    pub(in crate::tests) fn finish(mut self, entry: mir::FunctionId) -> mir::Module {
        for (name, representation) in mir::IntegerKind::ALL
            .map(|kind| {
                (
                    kind.canonical_name(),
                    mir::IntrinsicTypeRepresentation::Integer(kind),
                )
            })
            .into_iter()
            .chain([("Boolean", mir::IntrinsicTypeRepresentation::Boolean)])
        {
            self.structs.alloc(mir::StructDef {
                name: name.to_string(),
                type_arguments: Vec::new(),
                gc_free: true,
                representation: mir::StructRepresentation::Intrinsic(representation),
            });
        }
        self.classes.alloc(mir::ClassDef {
            modifier: mir::ClassModifier::Final,
            name: "String".to_string(),
            type_arguments: Vec::new(),
            representation: mir::ClassRepresentation::Intrinsic(
                mir::IntrinsicTypeRepresentation::String,
            ),
            interfaces: Vec::new(),
            vtable: Vec::new(),
            itables: Vec::new(),
        });
        let mut exact_types = Vec::new();
        test_exact_type(&self.function_types, &mir::Type::String, &mut exact_types);
        for (id, structure) in self.structs.iter() {
            let ty = match &structure.representation {
                mir::StructRepresentation::Declared { .. } => mir::Type::Struct(id),
                mir::StructRepresentation::Intrinsic(representation) => match representation {
                    mir::IntrinsicTypeRepresentation::Integer(kind) => mir::Type::Integer(*kind),
                    mir::IntrinsicTypeRepresentation::Boolean => mir::Type::Boolean,
                    mir::IntrinsicTypeRepresentation::Ptr { pointee } => {
                        mir::Type::Ptr(Box::new(pointee.clone()))
                    }
                    mir::IntrinsicTypeRepresentation::FunPtr { signature } => {
                        mir::Type::FunPtr(*signature)
                    }
                    mir::IntrinsicTypeRepresentation::String
                    | mir::IntrinsicTypeRepresentation::Array { .. }
                    | mir::IntrinsicTypeRepresentation::MutableArray { .. } => {
                        unreachable!("the test registry fixes intrinsic declaration targets")
                    }
                },
            };
            test_exact_type(&self.function_types, &ty, &mut exact_types);
            if let mir::StructRepresentation::Declared { fields, .. } = &structure.representation {
                for field in fields {
                    test_tuple_layout_type(&self.function_types, &field.ty, &mut exact_types);
                }
            }
        }
        for (id, enumeration) in self.enums.iter() {
            test_exact_type(
                &self.function_types,
                &mir::Type::Enum(id, enumeration.type_arguments.clone()),
                &mut exact_types,
            );
            for variant in &enumeration.variants {
                for field in &variant.fields {
                    test_tuple_layout_type(&self.function_types, &field.ty, &mut exact_types);
                }
            }
        }
        for (id, _) in self.interfaces.iter() {
            test_exact_type(
                &self.function_types,
                &mir::Type::Interface(id),
                &mut exact_types,
            );
        }
        for (id, class) in self.classes.iter() {
            if !matches!(
                class.representation,
                mir::ClassRepresentation::Intrinsic(mir::IntrinsicTypeRepresentation::String)
            ) {
                test_exact_type(
                    &self.function_types,
                    &mir::Type::Class(id),
                    &mut exact_types,
                );
            }
            if let mir::ClassRepresentation::Declared { fields, .. } = &class.representation {
                for field in fields {
                    test_tuple_layout_type(&self.function_types, &field.ty, &mut exact_types);
                }
            }
        }
        let mut source_callables = Vec::new();
        for (declaration_index, &function_id) in self.top_level.iter().enumerate() {
            let function = &self.functions[function_id];
            for (_, local) in function.body.locals.iter() {
                test_tuple_layout_type(&self.function_types, &local.ty, &mut exact_types);
            }
            let parameters = function
                .params
                .iter()
                .map(|parameter| {
                    test_exact_type(&self.function_types, &parameter.ty, &mut exact_types)
                })
                .collect();
            let result =
                test_exact_type(&self.function_types, &function.return_ty, &mut exact_types);
            let declaration_name = if function_id == entry {
                "main".to_string()
            } else {
                format!("testFunction{declaration_index}")
            };
            let declaration = SourceDeclarationKey::function(
                test_declaration_site(),
                CanonicalIdentifier::new(&declaration_name).unwrap(),
                0,
                None,
                Vec::new(),
            );
            let owner = PersistentFunctionId::from_source_declaration(&declaration).unwrap();
            let materialization = CallableMaterialization::new(
                CallableTemplateOwner::Function(owner),
                CallableMaterializationContext::NoSubstitution,
            );
            source_callables.push(
                mir::SourceCallableMaterialization::new(
                    function_id,
                    materialization,
                    ExactCallableSignature::new(Effect::Ordinary, None, parameters, result),
                    None,
                )
                .unwrap(),
            );
        }
        let source_exact_types = mir::SourceExactTypeIdentities::checked(exact_types).unwrap();
        let source_callable_materializations =
            mir::SourceCallableMaterializations::checked(source_callables).unwrap();
        let callable_signatures = mir::MirCallableSignatures::checked(
            source_callable_materializations
                .iter()
                .map(|source| source.signature_record().clone())
                .collect(),
        )
        .unwrap();
        mir::Module {
            cone: ConeIdentity::SINGLE_FILE,
            functions: self.functions,
            extern_functions: self.extern_functions,
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
            option_core: self.option_core,
            function_types: self.function_types,
            closure_classes: Arena::new(),
            closure_invoke_functions: Arena::new(),
            top_level: self.top_level,
            strings: self.strings,
            structs: self.structs,
            enums: self.enums,
            classes: self.classes,
            interfaces: self.interfaces,
            output: mir::MirOutput::Executable { entry },
            meta: mir::MirMeta {
                source_exact_types,
                source_callable_materializations,
                callable_signatures,
                ..mir::MirMeta::default()
            },
        }
    }
}

fn test_tuple_layout_type(
    function_types: &Arena<mir::FunctionType>,
    ty: &mir::Type,
    entries: &mut Vec<mir::SourceExactTypeIdentity>,
) {
    if matches!(ty, mir::Type::Tuple(_)) {
        test_exact_type(function_types, ty, entries);
    }
}

fn test_declaration_site() -> SourceDeclarationSite {
    SourceDeclarationSite::new(
        ConeIdentity::SINGLE_FILE,
        PackagePath::root(),
        DefinitionOwnerChain::top_level(),
        DeclarationScope::ConeWide,
    )
    .unwrap()
}

pub(in crate::tests) fn test_exact_type(
    function_types: &Arena<mir::FunctionType>,
    ty: &mir::Type,
    entries: &mut Vec<mir::SourceExactTypeIdentity>,
) -> PersistentExactTypeId {
    if let Some(existing) = entries.iter().find(|entry| entry.ty() == ty) {
        return existing.identity_record().id();
    }
    let key = match ty {
        mir::Type::Unit => ExactTypeKey::Nominal(CoreBuiltinNominal::Unit.identity_record().id()),
        mir::Type::Any => ExactTypeKey::Nominal(CoreBuiltinNominal::Any.identity_record().id()),
        mir::Type::Integer(kind) => test_nominal_exact(
            &format!("Test{}", kind.canonical_name()),
            SourceNominalKind::Struct,
        ),
        mir::Type::MachineScalar(kind) => test_nominal_exact(
            &format!(
                "TestMachine{}",
                kind.name()
                    .bytes()
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>()
            ),
            SourceNominalKind::Struct,
        ),
        mir::Type::Boolean => test_nominal_exact("TestBoolean", SourceNominalKind::Struct),
        mir::Type::String => test_nominal_exact("TestString", SourceNominalKind::Class),
        mir::Type::Struct(id) => test_nominal_exact(
            &format!("TestStruct{}", id.into_raw().into_u32()),
            SourceNominalKind::Struct,
        ),
        mir::Type::Class(id) => test_nominal_exact(
            &format!("TestClass{}", id.into_raw().into_u32()),
            SourceNominalKind::Class,
        ),
        mir::Type::Interface(id) => test_nominal_exact(
            &format!("TestInterface{}", id.into_raw().into_u32()),
            SourceNominalKind::Interface,
        ),
        mir::Type::Enum(id, _) => test_nominal_exact(
            &format!("TestEnum{}", id.into_raw().into_u32()),
            SourceNominalKind::Enum,
        ),
        mir::Type::Tuple(elements) => ExactTypeKey::Tuple(
            scoop_identity::NonEmptyVec::new(
                elements
                    .iter()
                    .map(|element| test_exact_type(function_types, element, entries))
                    .collect(),
            )
            .expect("test tuple types are non-empty"),
        ),
        mir::Type::Function(id) => {
            let signature = function_types[*id].clone();
            ExactTypeKey::Function {
                effect: if signature.is_suspend {
                    Effect::Suspend
                } else {
                    Effect::Ordinary
                },
                parameters: signature
                    .parameter_types
                    .iter()
                    .map(|parameter| test_exact_type(function_types, parameter, entries))
                    .collect(),
                result: test_exact_type(function_types, &signature.return_type, entries),
            }
        }
        mir::Type::Ptr(pointee) => {
            ExactTypeKey::RawPointer(test_exact_type(function_types, pointee, entries))
        }
        mir::Type::FunPtr(id) => {
            let signature = function_types[*id].clone();
            ExactTypeKey::NativeFunctionPointer {
                calling_convention: scoop_identity::CallingConvention::C,
                parameters: signature
                    .parameter_types
                    .iter()
                    .map(|parameter| test_exact_type(function_types, parameter, entries))
                    .collect(),
                result: test_exact_type(function_types, &signature.return_type, entries),
            }
        }
    };
    let record = CborIdentityRecord::from_key(key).unwrap();
    let id = record.id();
    entries.push(mir::SourceExactTypeIdentity::checked(ty.clone(), record, None).unwrap());
    id
}

fn test_nominal_exact(name: &str, kind: SourceNominalKind) -> ExactTypeKey {
    let declaration = SourceDeclarationKey::nominal(
        test_declaration_site(),
        CanonicalIdentifier::new(name).unwrap(),
        kind,
        0,
    );
    ExactTypeKey::Nominal(PersistentTypeId::from_source_declaration(&declaration).unwrap())
}
