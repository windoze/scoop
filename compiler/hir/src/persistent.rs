//! Persistent identity computation over the Export HIR (DESIGN 3.1).
//!
//! Arena ids never cross a Cone boundary; linker-relevant declarations
//! derive persistent ids from their origin Cone, package, typed owner
//! chain, name and normalized signature key instead. Signatures
//! reference types through *persistent type refs*: builtins by fixed
//! primitive tags, nominal declarations by their persistent ids,
//! applications structurally, and generic parameters by binder position
//! — never by arena ids or FQN.

use std::collections::BTreeMap;

pub use scoop_identity::persistent::{
    ExactTypeKey, ManagedFunctionEffect, NativeCallingConvention, PersistentExactTypeId,
};
use scoop_identity::persistent::{
    PersistentFunctionId, PersistentGenericFunctionId, PersistentGenericTypeId, PersistentTypeId,
};
use scoop_identity::{CborWriter, ConeIdentity, DefinitionKey, OwnerKind, OwnerStep};

use crate::{
    ClassDecl, ClassId, DeclarationOrigin, EnumDecl, EnumId, Function, FunctionGenericity,
    FunctionId, FunctionType, FunctionTypeId, IntegerKind, IntegerSignedness, InterfaceDecl,
    InterfaceId, Module, StructDecl, StructId, Type, TypeId,
};

/// Maps the compilation's source providers to Cone identities. In the
/// single-unit model provider 0 is the core Cone and provider 1 the
/// current Cone; the mapping stays explicit so later stages widen it
/// without touching the encoders.
#[derive(Debug, Clone)]
pub struct PersistentWorld {
    core: ConeIdentity,
    user: ConeIdentity,
}

impl PersistentWorld {
    /// Single-unit mapping: provider raw 0 is the core Cone, every other
    /// provider (user files plus allowlisted test providers) belongs to
    /// the current Cone.
    pub fn single_unit(core: ConeIdentity, user: ConeIdentity) -> Self {
        PersistentWorld { core, user }
    }

    pub fn cone_of(&self, origin: DeclarationOrigin) -> Option<ConeIdentity> {
        if origin.provider.into_raw() == 0 {
            Some(self.core)
        } else {
            Some(self.user)
        }
    }

    pub fn core_cone(&self) -> ConeIdentity {
        self.core
    }

    pub fn user_cone(&self) -> ConeIdentity {
        self.user
    }
}

/// Computes persistent ids and signature keys for one module. Pure:
/// nothing is written back into the module.
pub struct PersistentIds<'a> {
    module: &'a Module,
    world: &'a PersistentWorld,
    struct_ids: BTreeMap<u32, PersistentTypeId>,
    enum_ids: BTreeMap<u32, PersistentTypeId>,
    class_ids: BTreeMap<u32, PersistentTypeId>,
    interface_ids: BTreeMap<u32, PersistentTypeId>,
    generic_type_ids: BTreeMap<u32, PersistentGenericTypeId>,
}

fn nominal_key(name: &str, kind: &'static str) -> Vec<u8> {
    DefinitionKey::Source {
        // Packages arrive with the import milestone; all current
        // declarations live in the root package.
        package: String::new(),
        owner: Vec::new(),
        name: name.to_owned(),
        signature: kind.as_bytes().to_vec(),
    }
    .canonical_cbor()
}

impl<'a> PersistentIds<'a> {
    pub fn new(module: &'a Module, world: &'a PersistentWorld) -> Self {
        PersistentIds {
            module,
            world,
            struct_ids: BTreeMap::new(),
            enum_ids: BTreeMap::new(),
            class_ids: BTreeMap::new(),
            interface_ids: BTreeMap::new(),
            generic_type_ids: BTreeMap::new(),
        }
    }

    pub fn struct_id(&mut self, id: StructId) -> Option<PersistentTypeId> {
        let raw: u32 = u32::from(id.into_raw());
        if let Some(cached) = self.struct_ids.get(&raw) {
            return Some(*cached);
        }
        let decl = &self.module.structs[id];
        let cone = self.world.cone_of(decl.origin)?;
        let persistent =
            PersistentTypeId::from_definition_key(cone, &nominal_key(&decl.name, "struct"));
        self.struct_ids.insert(raw, persistent);
        Some(persistent)
    }

    pub fn enum_id(&mut self, id: EnumId) -> Option<PersistentTypeId> {
        let raw: u32 = u32::from(id.into_raw());
        if let Some(cached) = self.enum_ids.get(&raw) {
            return Some(*cached);
        }
        let decl: &EnumDecl = &self.module.enums[id];
        let cone = self.world.cone_of(decl.origin)?;
        let persistent =
            PersistentTypeId::from_definition_key(cone, &nominal_key(&decl.name, "enum"));
        self.enum_ids.insert(raw, persistent);
        Some(persistent)
    }

    pub fn class_id(&mut self, id: ClassId) -> Option<PersistentTypeId> {
        let raw: u32 = u32::from(id.into_raw());
        if let Some(cached) = self.class_ids.get(&raw) {
            return Some(*cached);
        }
        let decl: &ClassDecl = &self.module.classes[id];
        let cone = self.world.cone_of(decl.origin)?;
        let persistent =
            PersistentTypeId::from_definition_key(cone, &nominal_key(&decl.name, "class"));
        self.class_ids.insert(raw, persistent);
        Some(persistent)
    }

    pub fn interface_id(&mut self, id: InterfaceId) -> Option<PersistentTypeId> {
        let raw: u32 = u32::from(id.into_raw());
        if let Some(cached) = self.interface_ids.get(&raw) {
            return Some(*cached);
        }
        let decl: &InterfaceDecl = &self.module.interfaces[id];
        let cone = self.world.cone_of(decl.origin)?;
        let persistent =
            PersistentTypeId::from_definition_key(cone, &nominal_key(&decl.name, "interface"));
        self.interface_ids.insert(raw, persistent);
        Some(persistent)
    }

    /// The generic template identity of a nominal declaration that has
    /// type parameters; non-generic declarations use their plain type id.
    fn generic_template_id(
        &mut self,
        origin: DeclarationOrigin,
        name: &str,
        kind: &'static str,
    ) -> Option<PersistentGenericTypeId> {
        let cone = self.world.cone_of(origin)?;
        let key = DefinitionKey::Source {
            package: String::new(),
            owner: Vec::new(),
            name: name.to_owned(),
            signature: format!("{kind}-template").into_bytes(),
        }
        .canonical_cbor();
        Some(PersistentGenericTypeId::from_definition_key(cone, &key))
    }

    pub fn generic_struct_id(&mut self, id: StructId) -> Option<PersistentGenericTypeId> {
        let raw: u32 = u32::from(id.into_raw());
        if let Some(cached) = self.generic_type_ids.get(&raw) {
            return Some(*cached);
        }
        let decl: &StructDecl = &self.module.structs[id];
        let persistent = self.generic_template_id(decl.origin, &decl.name, "struct")?;
        self.generic_type_ids.insert(raw, persistent);
        Some(persistent)
    }

    pub fn generic_enum_id(&mut self, id: EnumId) -> Option<PersistentGenericTypeId> {
        let raw: u32 = u32::from(id.into_raw());
        if let Some(cached) = self.generic_type_ids.get(&raw) {
            return Some(*cached);
        }
        let decl: &EnumDecl = &self.module.enums[id];
        let persistent = self.generic_template_id(decl.origin, &decl.name, "enum")?;
        self.generic_type_ids.insert(raw, persistent);
        Some(persistent)
    }

    pub fn generic_class_id(&mut self, id: ClassId) -> Option<PersistentGenericTypeId> {
        let raw: u32 = u32::from(id.into_raw());
        if let Some(cached) = self.generic_type_ids.get(&raw) {
            return Some(*cached);
        }
        let decl: &ClassDecl = &self.module.classes[id];
        let persistent = self.generic_template_id(decl.origin, &decl.name, "class")?;
        self.generic_type_ids.insert(raw, persistent);
        Some(persistent)
    }

    pub fn generic_interface_id(&mut self, id: InterfaceId) -> Option<PersistentGenericTypeId> {
        let raw: u32 = u32::from(id.into_raw());
        if let Some(cached) = self.generic_type_ids.get(&raw) {
            return Some(*cached);
        }
        let decl: &InterfaceDecl = &self.module.interfaces[id];
        let persistent = self.generic_template_id(decl.origin, &decl.name, "interface")?;
        self.generic_type_ids.insert(raw, persistent);
        Some(persistent)
    }

    /// Encodes one type as a canonical persistent signature ref.
    pub fn signature_ref(&mut self, ty: TypeId, out: &mut CborWriter) -> Option<()> {
        match self.module.types[ty] {
            Type::Unit => {
                out.array(1).unsigned(1);
            }
            Type::Integer(kind) => {
                out.array(2).unsigned(2).unsigned(integer_tag(kind));
            }
            Type::Boolean => {
                out.array(1).unsigned(3);
            }
            Type::String => {
                out.array(1).unsigned(4);
            }
            Type::Any => {
                out.array(1).unsigned(5);
            }
            Type::Struct(application) => {
                let app = &self.module.struct_applications[application];
                let template = app.template;
                let decl: &StructDecl = &self.module.structs[template];
                if decl.type_params.is_empty() {
                    let id = self.struct_id(template)?;
                    out.array(2).unsigned(6).bytes(id.as_bytes());
                } else {
                    let origin = self.generic_struct_id(template)?;
                    out.array(3).unsigned(7).bytes(origin.as_bytes());
                    out.array(app.arguments.len() as u64);
                    for argument in app.arguments.clone() {
                        self.signature_ref(argument, out)?;
                    }
                }
            }
            Type::Enum(application) => {
                let app = &self.module.enum_applications[application];
                let template = app.template;
                let decl: &EnumDecl = &self.module.enums[template];
                if decl.type_params.is_empty() {
                    let id = self.enum_id(template)?;
                    out.array(2).unsigned(6).bytes(id.as_bytes());
                } else {
                    let origin = self.generic_template_id(decl.origin, &decl.name, "enum")?;
                    out.array(3).unsigned(7).bytes(origin.as_bytes());
                    out.array(app.arguments.len() as u64);
                    for argument in app.arguments.clone() {
                        self.signature_ref(argument, out)?;
                    }
                }
            }
            Type::Class(application) => {
                let app = &self.module.class_applications[application];
                let template = app.template;
                let decl: &ClassDecl = &self.module.classes[template];
                if decl.type_params.is_empty() {
                    let id = self.class_id(template)?;
                    out.array(2).unsigned(6).bytes(id.as_bytes());
                } else {
                    let origin = self.generic_template_id(decl.origin, &decl.name, "class")?;
                    out.array(3).unsigned(7).bytes(origin.as_bytes());
                    out.array(app.arguments.len() as u64);
                    for argument in app.arguments.clone() {
                        self.signature_ref(argument, out)?;
                    }
                }
            }
            Type::Interface(application) => {
                let app = &self.module.interface_applications[application];
                let template = app.template;
                let decl: &InterfaceDecl = &self.module.interfaces[template];
                if decl.type_params.is_empty() {
                    let id = self.interface_id(template)?;
                    out.array(2).unsigned(6).bytes(id.as_bytes());
                } else {
                    let origin = self.generic_template_id(decl.origin, &decl.name, "interface")?;
                    out.array(3).unsigned(7).bytes(origin.as_bytes());
                    out.array(app.arguments.len() as u64);
                    for argument in app.arguments.clone() {
                        self.signature_ref(argument, out)?;
                    }
                }
            }
            Type::Tuple(ref elements) => {
                out.array(elements.len() as u64 + 1).unsigned(8);
                for element in elements.clone() {
                    self.signature_ref(element, out)?;
                }
            }
            Type::Function(function_type) => {
                self.function_ref(function_type, false, out)?;
            }
            Type::FunPtr(function_type) => {
                self.function_ref(function_type, true, out)?;
            }
            Type::Ptr(pointee) => {
                out.array(2).unsigned(11);
                // Nest the pointee ref under an inner array head.
                let mut inner = CborWriter::new();
                self.signature_ref(pointee, &mut inner)?;
                out.bytes(&inner.into_bytes());
            }
            Type::Param(param) => {
                out.array(3)
                    .unsigned(12)
                    .unsigned(param.identity_raw() as u64)
                    .unsigned(param.into_raw() as u64);
            }
        }
        Some(())
    }

    fn function_ref(
        &mut self,
        function_type: FunctionTypeId,
        native: bool,
        out: &mut CborWriter,
    ) -> Option<()> {
        let signature: &FunctionType = &self.module.function_types[function_type];
        let head = if native { 10u64 } else { 9u64 };
        out.array(3).unsigned(head);
        let mut inner = CborWriter::new();
        inner.array(signature.parameter_types.len() as u64);
        for parameter in signature.parameter_types.clone() {
            self.signature_ref(parameter, &mut inner)?;
        }
        let mut result = CborWriter::new();
        self.signature_ref(signature.return_type, &mut result)?;
        out.array(2);
        out.bytes(&inner.into_bytes());
        out.bytes(&result.into_bytes());
        if !native {
            // Suspend flag follows the parameter/result pair.
            out.unsigned(signature.is_suspend as u64);
        }
        Some(())
    }

    /// Canonical signature key of a function declaration: extension
    /// marker, suspend flag and the parameter/result refs in source
    /// order. Generic parameters appear as binder refs.
    pub fn function_signature_key(&mut self, function: &Function) -> Option<Vec<u8>> {
        let is_extension = function.method.is_none()
            && function
                .params
                .first()
                .is_some_and(|parameter| parameter.name == "this");
        let mut writer = CborWriter::new();
        writer.map(3);
        writer.field(1).unsigned(is_extension as u64);
        writer.field(2).unsigned(function.is_suspend as u64);
        writer.field(3);
        writer.array(function.params.len() as u64);
        for parameter in &function.params {
            self.signature_ref(parameter.ty, &mut writer)?;
        }
        let mut result = CborWriter::new();
        self.signature_ref(function.return_ty, &mut result)?;
        writer.bytes(&result.into_bytes());
        Some(writer.into_bytes())
    }

    /// Owner chain of a function: methods carry their owner's nominal
    /// declaration as one typed step (the template for generic owners).
    fn function_owner(&mut self, function: &Function) -> Option<Vec<OwnerStep>> {
        let method: Option<&crate::Method> = function.method.as_ref();
        let Some(method) = method else {
            return Some(Vec::new());
        };
        let owner = method.owner;
        let (step, kind) = match self.module.types[owner] {
            Type::Struct(application) => {
                let template = self.module.struct_applications[application].template;
                let decl: &StructDecl = &self.module.structs[template];
                (
                    OwnerStep {
                        kind: if decl.type_params.is_empty() {
                            OwnerKind::Type
                        } else {
                            OwnerKind::GenericType
                        },
                        id: if decl.type_params.is_empty() {
                            *self.struct_id(template)?.as_bytes()
                        } else {
                            *self.generic_struct_id(template)?.as_bytes()
                        },
                    },
                    "struct",
                )
            }
            Type::Enum(application) => {
                let template = self.module.enum_applications[application].template;
                let decl: &EnumDecl = &self.module.enums[template];
                (
                    OwnerStep {
                        kind: if decl.type_params.is_empty() {
                            OwnerKind::Type
                        } else {
                            OwnerKind::GenericType
                        },
                        id: if decl.type_params.is_empty() {
                            *self.enum_id(template)?.as_bytes()
                        } else {
                            *self
                                .generic_template_id(decl.origin, &decl.name, "enum")?
                                .as_bytes()
                        },
                    },
                    "enum",
                )
            }
            Type::Class(application) => {
                let template = self.module.class_applications[application].template;
                let decl: &ClassDecl = &self.module.classes[template];
                (
                    OwnerStep {
                        kind: if decl.type_params.is_empty() {
                            OwnerKind::Type
                        } else {
                            OwnerKind::GenericType
                        },
                        id: if decl.type_params.is_empty() {
                            *self.class_id(template)?.as_bytes()
                        } else {
                            *self
                                .generic_template_id(decl.origin, &decl.name, "class")?
                                .as_bytes()
                        },
                    },
                    "class",
                )
            }
            Type::Interface(application) => {
                let template = self.module.interface_applications[application].template;
                let decl: &InterfaceDecl = &self.module.interfaces[template];
                (
                    OwnerStep {
                        kind: if decl.type_params.is_empty() {
                            OwnerKind::Type
                        } else {
                            OwnerKind::GenericType
                        },
                        id: if decl.type_params.is_empty() {
                            *self.interface_id(template)?.as_bytes()
                        } else {
                            *self
                                .generic_template_id(decl.origin, &decl.name, "interface")?
                                .as_bytes()
                        },
                    },
                    "interface",
                )
            }
            // Methods of tuple/pointer/function types do not exist in the
            // current language subset.
            _ => return None,
        };
        let _ = kind;
        Some(vec![step])
    }

    /// Persistent id of a non-generic function declaration.
    pub fn function_id(&mut self, id: FunctionId) -> Option<PersistentFunctionId> {
        let function: &Function = &self.module.functions[id];
        if !matches!(function.genericity, FunctionGenericity::Plain) {
            // Generic templates use their own id kind.
            return None;
        }
        let cone = self.world.cone_of(function.origin)?;
        let key = DefinitionKey::Source {
            package: String::new(),
            owner: self.function_owner(function)?,
            name: function.name.clone(),
            signature: self.function_signature_key(function)?,
        }
        .canonical_cbor();
        Some(PersistentFunctionId::from_definition_key(cone, &key))
    }

    /// Persistent id of a well-known builtin nominal (`Unit`, `Any`).
    /// These have no source declaration; their identity is the reserved
    /// core Cone plus a fixed name/kind pair (see the open spec question
    /// recorded for the diagnostic printer).
    pub fn builtin_type_id(&mut self, name: &str, kind: &'static str) -> Option<PersistentTypeId> {
        let cone = self.core_cone()?;
        let key = DefinitionKey::Source {
            package: String::new(),
            owner: Vec::new(),
            name: name.to_owned(),
            signature: kind.as_bytes().to_vec(),
        }
        .canonical_cbor();
        Some(PersistentTypeId::from_definition_key(cone, &key))
    }

    fn core_cone(&self) -> Option<ConeIdentity> {
        Some(self.world.core_cone())
    }

    /// Persistent template id of an owner-parameterized or generic
    /// method template, derived from the same definition key as its
    /// function id (the signature's binder refs make it template-shaped).
    pub fn generic_callable_id(
        &mut self,
        id: FunctionId,
    ) -> Option<scoop_identity::persistent::PersistentGenericCallableId> {
        let function = &self.module.functions[id];
        let cone = self.world.cone_of(function.origin)?;
        let key = DefinitionKey::Source {
            package: String::new(),
            owner: self.function_owner(function)?,
            name: function.name.clone(),
            signature: self.function_signature_key(function)?,
        }
        .canonical_cbor();
        Some(
            scoop_identity::persistent::PersistentGenericCallableId::from_definition_key(
                cone, &key,
            ),
        )
    }

    /// Persistent template id of a generic function declaration.
    pub fn generic_function_id(&mut self, id: FunctionId) -> Option<PersistentGenericFunctionId> {
        let function: &Function = &self.module.functions[id];
        let FunctionGenericity::Generic { .. } = function.genericity else {
            return None;
        };
        let cone = self.world.cone_of(function.origin)?;
        let key = DefinitionKey::Source {
            package: String::new(),
            owner: self.function_owner(function)?,
            name: function.name.clone(),
            signature: self.function_signature_key(function)?,
        }
        .canonical_cbor();
        Some(PersistentGenericFunctionId::from_definition_key(cone, &key))
    }
}

fn integer_tag(kind: IntegerKind) -> u64 {
    // Sign bit in the low position, width (bytes) in the high bits: a
    // compact, order-free encoding of the eight canonical kinds.
    let sign = match kind.signedness() {
        IntegerSignedness::Signed => 1u64,
        IntegerSignedness::Unsigned => 0u64,
    };
    let width = kind.width().bits() as u64 / 8;
    sign | (width << 1)
}
