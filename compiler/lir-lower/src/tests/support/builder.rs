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
        variants.push(test_variant(
            "Some".to_string(),
            payload_gc_free,
            some_fields,
        ));
        let none_index = u32::try_from(variants.len()).expect("test enum arity fits u32");
        variants.push(test_variant("None".to_string(), true, Vec::new()));
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
                c_abi: mir::StructCAbi::SourceRepresentation,
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
                c_abi: mir::StructCAbi::SourceRepresentation,
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
            .map(|method| mir::InterfaceMethod {
                gc_effect: mir::GcEffect::Managed,
                name: format!("{name}.{method}"),
                parameters: Vec::new(),
                return_type: mir::Type::Unit,
            })
            .collect();
        self.interfaces.alloc(mir::InterfaceDef {
            name: name.to_string(),
            type_arguments: Vec::new(),
            parents: Vec::new(),
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
            release_policy: Default::default(),
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
            release_policy: Default::default(),
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
}

mod finish;
mod identities;
pub(in crate::tests) use identities::test_exact_type;
use identities::{test_declaration_site, test_exact_type_at, test_tuple_layout_type};
