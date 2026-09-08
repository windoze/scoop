//! HIR surface wire schema (DESIGN 4.3/4.5, T19 batch 1). The packager
//! encodes the six-surface split as a canonical CBOR document addressed
//! by persistent identities — no session arena ordinal survives on the
//! wire. The reader runs the four-step pipeline: wire decode under
//! budgets, structural validation, typed remap, and the committed
//! [`ImportedHirSet`]. Bodies, signature type graphs and template
//! predicates ride later batches; this document is the closed root
//! layer every deeper wire section hangs off.

use scoop_identity::ConeIdentity;
use scoop_identity::cbor::{CborReader, CborWriter};

use crate::export_surface::{ExportPurposes, ExportSurfaces};
use crate::persistent::integer_tag;
use crate::{ExportEntity, Module, PersistentIds};

/// Wire schema domain tag; readers reject any other magic.
pub const HIR_SURFACE_WIRE_MAGIC: &str = "scoop-hir-surface-wire-v1";

/// The closed wire kind set, mirroring [`ExportEntity`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WireEntityKind {
    Function,
    Property,
    PropertyGetter,
    PropertySetter,
    GenericFunction,
    GenericMethod,
    Struct,
    StructConstructor,
    Enum,
    Class,
    ClassConstructor,
    Interface,
    InterfaceMethod,
    Object,
    ObjectType,
    CompanionRelation,
    SingletonValue,
    TypeAlias,
    Variant,
}

impl WireEntityKind {
    fn tag(self) -> u64 {
        match self {
            Self::Function => 1,
            Self::Property => 2,
            Self::PropertyGetter => 3,
            Self::PropertySetter => 4,
            Self::GenericFunction => 5,
            Self::GenericMethod => 6,
            Self::Struct => 7,
            Self::StructConstructor => 8,
            Self::Enum => 9,
            Self::Class => 10,
            Self::ClassConstructor => 11,
            Self::Interface => 12,
            Self::InterfaceMethod => 13,
            Self::Object => 14,
            Self::ObjectType => 15,
            Self::CompanionRelation => 16,
            Self::SingletonValue => 17,
            Self::TypeAlias => 18,
            Self::Variant => 19,
        }
    }

    fn from_tag(tag: u64) -> Option<Self> {
        Some(match tag {
            1 => Self::Function,
            2 => Self::Property,
            3 => Self::PropertyGetter,
            4 => Self::PropertySetter,
            5 => Self::GenericFunction,
            6 => Self::GenericMethod,
            7 => Self::Struct,
            8 => Self::StructConstructor,
            9 => Self::Enum,
            10 => Self::Class,
            11 => Self::ClassConstructor,
            12 => Self::Interface,
            13 => Self::InterfaceMethod,
            14 => Self::Object,
            15 => Self::ObjectType,
            16 => Self::CompanionRelation,
            17 => Self::SingletonValue,
            18 => Self::TypeAlias,
            19 => Self::Variant,
            _ => return None,
        })
    }
}

/// The wire purpose bit set (five closed bits).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WirePurposes(pub u8);

impl WirePurposes {
    pub const PUBLIC_LOOKUP: u8 = 1 << 0;
    pub const INHERITANCE: u8 = 1 << 1;
    pub const TEMPLATE_SUPPORT: u8 = 1 << 2;
    pub const INTERFACE_DEPENDENCY: u8 = 1 << 3;
    pub const SOURCE_INTERFACE_TEMPLATE: u8 = 1 << 4;
    const MASK: u8 = 0x1F;

    fn from_export(purposes: &ExportPurposes) -> Self {
        let mut bits = 0;
        bits |= u8::from(purposes.public_lookup) * Self::PUBLIC_LOOKUP;
        bits |= u8::from(purposes.inheritance) * Self::INHERITANCE;
        bits |= u8::from(purposes.template_support) * Self::TEMPLATE_SUPPORT;
        bits |= u8::from(purposes.interface_dependency) * Self::INTERFACE_DEPENDENCY;
        bits |= u8::from(purposes.source_interface_template) * Self::SOURCE_INTERFACE_TEMPLATE;
        WirePurposes(bits)
    }
}

/// A structured wire failure carrying the failing section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HirWireError {
    Magic,
    Truncated,
    TrailingBytes,
    DepthExceeded,
    Malformed(&'static str),
    UnknownKind(u64),
    PurposeMask(u8),
    DefinitionOrder,
    DuplicateDefinition,
    BindingOrder,
    BindingNamespace(u64),
    RootIndex(u32),
    EmptyRoots,
    CountExceeded(&'static str),
    TextTooLarge,
    Exhausted,
}

/// One encoded definition (canonical order: ascending id bytes, then
/// kind tag, then discriminator).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireDefinition {
    pub id: [u8; 32],
    pub kind: WireEntityKind,
    /// Role/discriminator: variant local index, constructor ordinal or
    /// accessor role; zero when the id alone identifies the entity.
    pub discriminator: u32,
    pub name: String,
    pub package: Vec<String>,
    pub purposes: WirePurposes,
}

/// One name-keyed binding published by the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireBinding {
    pub package: Vec<String>,
    pub name: String,
    pub namespace: u64,
    pub roots: Vec<u32>,
}

/// One canonical type-table entry. Entries reference only earlier
/// entries, so the table is topologically ordered and acyclic by
/// construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WireTypeEntry {
    Unit,
    Any,
    Boolean,
    String,
    Integer(IntegerTag),
    /// A non-generic nominal declaration.
    NominalPlain([u8; 32]),
    /// A generic nominal application.
    NominalApplication {
        template: [u8; 32],
        arguments: Vec<u32>,
    },
    Ptr(u32),
    Function {
        suspend: bool,
        parameters: Vec<u32>,
        result: u32,
    },
    FunPtr {
        parameters: Vec<u32>,
        result: u32,
    },
    Tuple(Vec<u32>),
    /// A binder-relative type parameter slot (only meaningful inside a
    /// template signature).
    Param(u32),
}

/// The canonical integer tag: sign bit low, width bytes high (the same
/// encoding persistent signature keys use).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntegerTag(pub u64);

/// One function signature, referencing the type table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireSignature {
    pub definition: u32,
    pub parameters: Vec<u32>,
    pub result: u32,
}

/// Concretization predicates of one generic template: binder slots
/// that must be GC-free and slots whose pointee must be GC-free.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WirePredicate {
    pub definition: u32,
    pub no_gc_slots: Vec<u32>,
    pub gc_free_pointee_slots: Vec<u32>,
}

/// One wire body statement (kernel subset; the encoder rejects
/// constructs outside the subset with a typed error).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WireStatement {
    Expr(WireExpr),
    Return {
        value: Option<WireExpr>,
    },
    ValDecl {
        local_slot: u32,
        mutable: bool,
        init: WireExpr,
    },
    Assign {
        local_slot: u32,
        value: WireExpr,
    },
    If {
        cond: WireExpr,
        then_body: Vec<WireStatement>,
        else_body: Option<Vec<WireStatement>>,
    },
}

/// One wire body expression (kernel subset).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WireExpr {
    StringLiteral(String),
    IntegerLiteral {
        tag: u64,
        bits: u64,
    },
    BoolLiteral(bool),
    UnitLiteral,
    /// Read of one body-local by arena slot.
    Local(u32),
    TupleLiteral(Vec<WireExpr>),
    /// Direct call; the callee is a persistent identity plus explicit
    /// type-argument references into the type table.
    Call {
        callee: [u8; 32],
        generic: bool,
        type_arguments: Vec<u32>,
        arguments: Vec<WireExpr>,
    },
    /// Member call with an explicit receiver; the callee is a persistent
    /// identity (generic callables use their template id).
    MethodCall {
        receiver: Box<WireExpr>,
        callee: [u8; 32],
        callee_kind: u64,
        arguments: Vec<WireExpr>,
    },
}

/// One template body on the wire.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireBody {
    pub definition: u32,
    pub statements: Vec<WireStatement>,
}

/// The decoded, structurally valid document before typed remap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedHirSurfaceWire {
    pub cone: ConeIdentity,
    pub definitions: Vec<WireDefinition>,
    pub bindings: Vec<WireBinding>,
    /// Surface membership lists, as definition indices.
    pub public_lookup: Vec<u32>,
    pub template_structs: Vec<u32>,
    pub template_enums: Vec<u32>,
    pub template_classes: Vec<u32>,
    pub template_interfaces: Vec<u32>,
    pub template_functions: Vec<u32>,
    pub template_methods: Vec<u32>,
    pub dependency_structs: Vec<u32>,
    pub dependency_enums: Vec<u32>,
    pub dependency_classes: Vec<u32>,
    pub dependency_interfaces: Vec<u32>,
    /// (owner, member) definition-index pairs.
    pub protected_methods: Vec<(u32, u32)>,
    pub types: Vec<WireTypeEntry>,
    pub signatures: Vec<WireSignature>,
    pub predicates: Vec<WirePredicate>,
    pub bodies: Vec<WireBody>,
}

/// Typed handle for one imported definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ImportedDefinitionId(u32);

/// The committed semantic-world view of one imported HIR surface
/// document. Consumer stages address entities exclusively through
/// these typed handles.
#[derive(Debug, Clone)]
pub struct ImportedHirSet {
    cone: ConeIdentity,
    definitions: Vec<WireDefinition>,
    bindings: Vec<WireBinding>,
    public_lookup: Vec<ImportedDefinitionId>,
    template_support: Vec<ImportedDefinitionId>,
    types: Vec<WireTypeEntry>,
    signatures: Vec<WireSignature>,
    predicates: Vec<WirePredicate>,
    bodies: Vec<WireBody>,
}

impl ImportedHirSet {
    pub fn cone(&self) -> ConeIdentity {
        self.cone
    }

    pub fn definition(&self, id: ImportedDefinitionId) -> &WireDefinition {
        &self.definitions[id.0 as usize]
    }

    pub fn definitions(&self) -> &[WireDefinition] {
        &self.definitions
    }

    pub fn public_lookup(&self) -> &[ImportedDefinitionId] {
        &self.public_lookup
    }

    pub fn template_support(&self) -> &[ImportedDefinitionId] {
        &self.template_support
    }

    pub fn types(&self) -> &[WireTypeEntry] {
        &self.types
    }

    pub fn signatures(&self) -> &[WireSignature] {
        &self.signatures
    }

    pub fn predicates(&self) -> &[WirePredicate] {
        &self.predicates
    }

    pub fn bodies(&self) -> &[WireBody] {
        &self.bodies
    }

    /// The signature of one imported definition, if it carries one.
    pub fn signature_of(&self, id: ImportedDefinitionId) -> Option<&WireSignature> {
        self.signatures
            .iter()
            .find(|signature| signature.definition == id.0)
    }

    /// The roots of one ordinary binding by package segments, name and
    /// namespace tag (1 = value, 2 = type).
    pub fn binding_roots(
        &self,
        package: &[String],
        name: &str,
        namespace: u64,
    ) -> Option<Vec<ImportedDefinitionId>> {
        self.bindings
            .iter()
            .find(|binding| {
                binding.package == package && binding.name == name && binding.namespace == namespace
            })
            .map(|binding| {
                // Indices were validated in range at commit time.
                binding
                    .roots
                    .iter()
                    .map(|index| ImportedDefinitionId(*index))
                    .collect()
            })
    }

    /// Internal constructor used by the remap step after validation.
    fn commit(decoded: DecodedHirSurfaceWire) -> Self {
        let remap = |indices: &[u32]| {
            indices
                .iter()
                .map(|index| ImportedDefinitionId(*index))
                .collect::<Vec<_>>()
        };
        ImportedHirSet {
            cone: decoded.cone,
            definitions: decoded.definitions,
            bindings: decoded.bindings,
            types: decoded.types,
            signatures: decoded.signatures,
            predicates: decoded.predicates,
            bodies: decoded.bodies,
            public_lookup: remap(&decoded.public_lookup),
            template_support: remap(
                &decoded
                    .template_structs
                    .iter()
                    .chain(&decoded.template_enums)
                    .chain(&decoded.template_classes)
                    .chain(&decoded.template_interfaces)
                    .chain(&decoded.template_functions)
                    .chain(&decoded.template_methods)
                    .copied()
                    .collect::<Vec<_>>(),
            ),
        }
    }
}

/// Encodes the surface split of one finished module. Every referenced
/// entity must carry a persistent identity; a missing identity is an
/// encoding failure, never a silent skip.
pub fn encode_surface_wire(
    ids: &mut PersistentIds<'_>,
    module: &Module,
    surfaces: &ExportSurfaces,
) -> Result<Vec<u8>, HirWireError> {
    let mut table = Vec::new();
    let resolve = |entity: ExportEntity,
                   ids: &mut PersistentIds<'_>|
     -> Result<(WireEntityKind, [u8; 32], u32), HirWireError> {
        let failed = || HirWireError::Malformed("entity without persistent identity");
        Ok(match entity {
            ExportEntity::Function(id) => {
                // Generic declarations carry template identities.
                match module.functions[id].genericity {
                    crate::FunctionGenericity::Generic { .. } => (
                        WireEntityKind::GenericFunction,
                        *ids.generic_function_id(id).ok_or_else(failed)?.as_bytes(),
                        0,
                    ),
                    crate::FunctionGenericity::OwnerParameterizedMethod { .. }
                    | crate::FunctionGenericity::GenericMethod { .. } => (
                        WireEntityKind::GenericMethod,
                        *ids.generic_callable_id(id).ok_or_else(failed)?.as_bytes(),
                        0,
                    ),
                    crate::FunctionGenericity::Plain => (
                        WireEntityKind::Function,
                        *ids.function_id(id).ok_or_else(failed)?.as_bytes(),
                        0,
                    ),
                }
            }
            ExportEntity::Property(id) => (
                WireEntityKind::Property,
                *ids.property_id(id).ok_or_else(failed)?.as_bytes(),
                0,
            ),
            ExportEntity::Struct(id) => (
                WireEntityKind::Struct,
                *ids.struct_id(id).ok_or_else(failed)?.as_bytes(),
                0,
            ),
            ExportEntity::Enum(id) => (
                WireEntityKind::Enum,
                *ids.enum_id(id).ok_or_else(failed)?.as_bytes(),
                0,
            ),
            ExportEntity::Class(id) => (
                WireEntityKind::Class,
                *ids.class_id(id).ok_or_else(failed)?.as_bytes(),
                0,
            ),
            ExportEntity::Interface(id) => (
                WireEntityKind::Interface,
                *ids.interface_id(id).ok_or_else(failed)?.as_bytes(),
                0,
            ),
            ExportEntity::Object(id) => (
                WireEntityKind::Object,
                *ids.object_id(id).ok_or_else(failed)?.as_bytes(),
                0,
            ),
            ExportEntity::TypeAlias(id) => (
                WireEntityKind::TypeAlias,
                *ids.type_alias_id(id).ok_or_else(failed)?.as_bytes(),
                0,
            ),
            ExportEntity::GenericFunction(id) => (
                WireEntityKind::GenericFunction,
                *ids.generic_function_id(module.generic_functions[id].function)
                    .ok_or_else(failed)?
                    .as_bytes(),
                0,
            ),
            ExportEntity::GenericMethod(id) => (
                WireEntityKind::GenericMethod,
                *ids.generic_callable_id(module.generic_methods[id].function)
                    .ok_or_else(failed)?
                    .as_bytes(),
                0,
            ),
            ExportEntity::PropertyGetter(getter) => {
                let property = module
                    .properties
                    .iter()
                    .find(|(_, property)| property.capability.getter() == getter)
                    .map(|(id, _)| id)
                    .ok_or_else(failed)?;
                (
                    WireEntityKind::PropertyGetter,
                    *ids.property_id(property).ok_or_else(failed)?.as_bytes(),
                    1,
                )
            }
            ExportEntity::PropertySetter(setter) => {
                let property = module
                    .properties
                    .iter()
                    .find(|(_, property)| property.capability.setter() == Some(setter))
                    .map(|(id, _)| id)
                    .ok_or_else(failed)?;
                (
                    WireEntityKind::PropertySetter,
                    *ids.property_id(property).ok_or_else(failed)?.as_bytes(),
                    2,
                )
            }
            ExportEntity::StructConstructor(ctor) => (
                WireEntityKind::StructConstructor,
                *ids.struct_id(module.struct_constructors[ctor].owner)
                    .ok_or_else(failed)?
                    .as_bytes(),
                module.structs[module.struct_constructors[ctor].owner]
                    .constructors
                    .iter()
                    .position(|other| *other == ctor)
                    .ok_or_else(failed)? as u32,
            ),
            ExportEntity::ClassConstructor(ctor) => {
                let owner = module.class_constructors[ctor].owner;
                (
                    WireEntityKind::ClassConstructor,
                    *ids.class_id(owner).ok_or_else(failed)?.as_bytes(),
                    module.classes[owner]
                        .constructors
                        .iter()
                        .position(|other| *other == ctor)
                        .ok_or_else(failed)? as u32,
                )
            }
            ExportEntity::InterfaceMethod(method) => (
                WireEntityKind::InterfaceMethod,
                *ids.function_id(module.interface_methods[method].function)
                    .ok_or_else(failed)?
                    .as_bytes(),
                0,
            ),
            ExportEntity::ObjectType(_)
            | ExportEntity::CompanionRelation(_)
            | ExportEntity::SingletonValue(_) => {
                // Role entities of one object share the object's type
                // identity, discriminated by kind tag.
                let object = match entity {
                    ExportEntity::ObjectType(ty) => module.object_types[ty].declaration,
                    ExportEntity::CompanionRelation(relation) => {
                        module.companion_relations[relation].object
                    }
                    ExportEntity::SingletonValue(value) => {
                        module.singleton_values[value].declaration
                    }
                    _ => unreachable!("guarded by the enclosing match"),
                };
                let kind = match entity {
                    ExportEntity::ObjectType(_) => WireEntityKind::ObjectType,
                    ExportEntity::CompanionRelation(_) => WireEntityKind::CompanionRelation,
                    ExportEntity::SingletonValue(_) => WireEntityKind::SingletonValue,
                    _ => unreachable!("guarded by the enclosing match"),
                };
                (
                    kind,
                    *ids.object_id(object).ok_or_else(failed)?.as_bytes(),
                    0,
                )
            }
            ExportEntity::Variant(variant) => (
                WireEntityKind::Variant,
                *ids.enum_id(variant.enumeration())
                    .ok_or_else(failed)?
                    .as_bytes(),
                variant.local_index(),
            ),
        })
    };
    // Populate the table from the purposes map (the union of every
    // surface's referenced entities).
    // One wire definition per identity; purposes from every surface
    // entry that references the entity merge into one set.
    let mut meta_by_key: std::collections::HashMap<(WireEntityKind, [u8; 32], u32), (u8, String)> =
        std::collections::HashMap::new();
    for (entity, purposes) in &surfaces.purposes {
        let (kind, id, discriminator) = resolve(*entity, ids)?;
        let entry = meta_by_key
            .entry((kind, id, discriminator))
            .or_insert_with(|| (0, entity_name(module, *entity).to_owned()));
        entry.0 |= WirePurposes::from_export(purposes).0;
    }
    for ((kind, id, discriminator), (bits, name)) in meta_by_key {
        table.push(WireDefinition {
            id,
            kind,
            discriminator,
            name,
            package: Vec::new(),
            purposes: WirePurposes(bits),
        });
    }
    table.sort_by(|a, b| {
        (a.id, a.kind.tag(), a.discriminator).cmp(&(b.id, b.kind.tag(), b.discriminator))
    });
    table.dedup_by(|a, b| a.id == b.id && a.kind == b.kind && a.discriminator == b.discriminator);
    let key_of = |definition: &WireDefinition| {
        (
            definition.id,
            definition.kind.tag(),
            definition.discriminator,
        )
    };
    let mut index_of = |table: &[WireDefinition], entity: ExportEntity| -> Option<u32> {
        let (kind, id, discriminator) = resolve(entity, ids).ok()?;
        table
            .iter()
            .position(|definition| key_of(definition) == (id, kind.tag(), discriminator))
            .map(|index| index as u32)
    };
    let mut bindings = Vec::new();
    for entry in &surfaces.binding_index.entries {
        let mut roots = Vec::new();
        for root in &entry.roots {
            if let Some(index) = index_of(&table, *root) {
                roots.push(index);
            }
        }
        // Roots reference the identity-canonical table; ascending
        // index order keeps the document deterministic.
        roots.sort_unstable();
        bindings.push(WireBinding {
            package: module.semantic_surface.packages[entry.package]
                .segments
                .clone(),
            name: entry.name.clone(),
            namespace: match entry.namespace {
                crate::export_surface::BindingNamespace::Value => 1,
                crate::export_surface::BindingNamespace::Type => 2,
            },
            roots,
        });
    }
    bindings.sort_by(|a, b| {
        (a.package.clone(), a.name.clone(), a.namespace).cmp(&(
            b.package.clone(),
            b.name.clone(),
            b.namespace,
        ))
    });
    let mut membership = |entities: &[ExportEntity]| -> Vec<u32> {
        entities
            .iter()
            .filter_map(|entity| index_of(&table, *entity))
            .collect()
    };
    let public_lookup_ids = membership(
        &surfaces
            .public_lookup
            .functions
            .iter()
            .copied()
            .map(ExportEntity::Function)
            .chain(
                surfaces
                    .public_lookup
                    .structs
                    .iter()
                    .copied()
                    .map(ExportEntity::Struct),
            )
            .chain(
                surfaces
                    .public_lookup
                    .enums
                    .iter()
                    .copied()
                    .map(ExportEntity::Enum),
            )
            .chain(
                surfaces
                    .public_lookup
                    .classes
                    .iter()
                    .copied()
                    .map(ExportEntity::Class),
            )
            .chain(
                surfaces
                    .public_lookup
                    .interfaces
                    .iter()
                    .copied()
                    .map(ExportEntity::Interface),
            )
            .chain(
                surfaces
                    .public_lookup
                    .objects
                    .iter()
                    .copied()
                    .map(ExportEntity::Object),
            )
            .chain(
                surfaces
                    .public_lookup
                    .type_aliases
                    .iter()
                    .copied()
                    .map(ExportEntity::TypeAlias),
            )
            .collect::<Vec<_>>(),
    );
    let template_ids = membership(
        &surfaces
            .template_support
            .generic_structs
            .iter()
            .copied()
            .map(ExportEntity::Struct)
            .chain(
                surfaces
                    .template_support
                    .generic_enums
                    .iter()
                    .copied()
                    .map(ExportEntity::Enum),
            )
            .chain(
                surfaces
                    .template_support
                    .generic_classes
                    .iter()
                    .copied()
                    .map(ExportEntity::Class),
            )
            .chain(
                surfaces
                    .template_support
                    .generic_interfaces
                    .iter()
                    .copied()
                    .map(ExportEntity::Interface),
            )
            .chain(
                surfaces
                    .template_support
                    .generic_functions
                    .iter()
                    .copied()
                    .map(ExportEntity::GenericFunction),
            )
            .chain(
                surfaces
                    .template_support
                    .generic_methods
                    .iter()
                    .copied()
                    .map(ExportEntity::GenericMethod),
            )
            .collect::<Vec<_>>(),
    );
    // Signature type graph: intern every public/generic function
    // signature into the canonical topological table.
    let mut type_table: Vec<WireTypeEntry> = Vec::new();
    let mut type_cache = std::collections::HashMap::new();
    let mut signatures = Vec::new();
    let mut predicates = Vec::new();
    let lookup = |table: &[WireDefinition],
                  kind: WireEntityKind,
                  id: [u8; 32],
                  discriminator: u32|
     -> Option<u32> {
        table
            .iter()
            .position(|definition| {
                definition.id == id
                    && definition.kind == kind
                    && definition.discriminator == discriminator
            })
            .map(|index| index as u32)
    };
    for function in &surfaces.public_lookup.functions {
        let record = &module.functions[*function];
        let mut parameters = Vec::with_capacity(record.params.len());
        for parameter in &record.params {
            parameters.push(intern_wire_type(
                module,
                ids,
                &mut type_table,
                &mut type_cache,
                parameter.ty,
            )?);
        }
        let result = intern_wire_type(
            module,
            ids,
            &mut type_table,
            &mut type_cache,
            record.return_ty,
        )?;
        let generic = matches!(record.genericity, crate::FunctionGenericity::Generic { .. });
        let owner_parameterized = matches!(
            record.genericity,
            crate::FunctionGenericity::OwnerParameterizedMethod { .. }
                | crate::FunctionGenericity::GenericMethod { .. }
        );
        let (kind, id, discriminator) = if generic {
            (
                WireEntityKind::GenericFunction,
                *ids.generic_function_id(*function)
                    .ok_or(HirWireError::Malformed("identity"))?
                    .as_bytes(),
                0u32,
            )
        } else if owner_parameterized {
            (
                WireEntityKind::GenericMethod,
                *ids.generic_callable_id(*function)
                    .ok_or(HirWireError::Malformed("identity"))?
                    .as_bytes(),
                0u32,
            )
        } else {
            (
                WireEntityKind::Function,
                *ids.function_id(*function)
                    .ok_or(HirWireError::Malformed("identity"))?
                    .as_bytes(),
                0u32,
            )
        };
        let Some(definition) = lookup(&table, kind, id, discriminator) else {
            continue;
        };
        signatures.push(WireSignature {
            definition,
            parameters,
            result,
        });
        if generic {
            if let Some((no_gc, pointees)) = generic_function_predicates(module, *function) {
                predicates.push(WirePredicate {
                    definition,
                    no_gc_slots: no_gc,
                    gc_free_pointee_slots: pointees,
                });
            }
        }
    }
    // Nominal template predicates.
    for template in &surfaces.template_support.generic_structs {
        let declaration = &module.structs[*template];
        let Some(definition) = lookup(
            &table,
            WireEntityKind::Struct,
            *ids.struct_id(*template)
                .ok_or(HirWireError::Malformed("identity"))?
                .as_bytes(),
            0,
        ) else {
            continue;
        };
        predicates.push(WirePredicate {
            definition,
            no_gc_slots: Vec::new(),
            gc_free_pointee_slots: declaration
                .gc_free_pointee_requirements
                .iter()
                .map(|requirement| requirement.type_param.into_raw())
                .collect(),
        });
    }
    // Template bodies (kernel subset): the source body of every public
    // generic function template, encoded against the type table.
    let mut bodies = Vec::new();
    for function in &surfaces.public_lookup.functions {
        let record = &module.functions[*function];
        let generic = matches!(record.genericity, crate::FunctionGenericity::Generic { .. });
        if !generic {
            continue;
        }
        let Some(definition) = lookup(
            &table,
            WireEntityKind::GenericFunction,
            *ids.generic_function_id(*function)
                .ok_or(HirWireError::Malformed("identity"))?
                .as_bytes(),
            0,
        ) else {
            continue;
        };
        let body = match &record.kind {
            crate::FunctionKind::User(body) => body,
            _ => continue,
        };
        let statements =
            encode_wire_statements(module, ids, &mut type_table, &mut type_cache, body)?;
        bodies.push(WireBody {
            definition,
            statements,
        });
    }
    let mut writer = CborWriter::new();
    writer.map(10);
    writer.field(1).text(HIR_SURFACE_WIRE_MAGIC);
    writer.field(2);
    writer.bytes(&cone_bytes(module));
    writer.field(3);
    writer.array(table.len() as u64);
    for definition in &table {
        writer.map(6);
        writer.field(1).bytes(&definition.id);
        writer.field(2).unsigned(definition.kind.tag());
        writer.field(3).unsigned(definition.discriminator as u64);
        writer.field(4).text(&definition.name);
        writer.field(5);
        writer.array(definition.package.len() as u64);
        for segment in &definition.package {
            writer.text(segment);
        }
        writer.field(6).unsigned(definition.purposes.0 as u64);
    }
    writer.field(4);
    writer.array(bindings.len() as u64);
    for binding in &bindings {
        writer.map(4);
        writer.field(1);
        writer.array(binding.package.len() as u64);
        for segment in &binding.package {
            writer.text(segment);
        }
        writer.field(2).text(&binding.name);
        writer.field(3).unsigned(binding.namespace);
        writer.field(4);
        writer.array(binding.roots.len() as u64);
        for root in &binding.roots {
            writer.unsigned(*root as u64);
        }
    }
    writer.field(5);
    writer.array(public_lookup_ids.len() as u64);
    for index in &public_lookup_ids {
        writer.unsigned(*index as u64);
    }
    writer.field(6);
    writer.array(template_ids.len() as u64);
    for index in &template_ids {
        writer.unsigned(*index as u64);
    }
    writer.field(7);
    writer.array(type_table.len() as u64);
    for entry in &type_table {
        write_type_entry(&mut writer, entry);
    }
    writer.field(8);
    writer.array(signatures.len() as u64);
    for signature in &signatures {
        writer.map(3);
        writer.field(1).unsigned(signature.definition as u64);
        writer.field(2);
        writer.array(signature.parameters.len() as u64);
        for parameter in &signature.parameters {
            writer.unsigned(*parameter as u64);
        }
        writer.field(3).unsigned(signature.result as u64);
    }
    writer.field(9);
    writer.array(predicates.len() as u64);
    for predicate in &predicates {
        writer.map(3);
        writer.field(1).unsigned(predicate.definition as u64);
        writer.field(2);
        writer.array(predicate.no_gc_slots.len() as u64);
        for slot in &predicate.no_gc_slots {
            writer.unsigned(*slot as u64);
        }
        writer.field(3);
        writer.array(predicate.gc_free_pointee_slots.len() as u64);
        for slot in &predicate.gc_free_pointee_slots {
            writer.unsigned(*slot as u64);
        }
    }
    writer.field(10);
    writer.array(bodies.len() as u64);
    for body in &bodies {
        writer.map(2);
        writer.field(1).unsigned(body.definition as u64);
        writer.field(2);
        write_wire_statements(&mut writer, &body.statements);
    }
    Ok(writer.into_bytes())
}

/// Encodes the kernel statement subset; anything else is a typed error,
/// never a silent skip.
fn encode_wire_statements(
    module: &Module,
    ids: &mut PersistentIds<'_>,
    type_table: &mut Vec<WireTypeEntry>,
    type_cache: &mut std::collections::HashMap<crate::TypeId, u32>,
    body: &crate::Body,
) -> Result<Vec<WireStatement>, HirWireError> {
    body.statements
        .iter()
        .map(|statement| {
            Ok(match &statement.kind {
                crate::StatementKind::Expr(expr) => WireStatement::Expr(encode_wire_expr(
                    module, ids, type_table, type_cache, expr,
                )?),
                crate::StatementKind::Return { value } => WireStatement::Return {
                    value: value
                        .as_ref()
                        .map(|expr| encode_wire_expr(module, ids, type_table, type_cache, expr))
                        .transpose()?,
                },
                crate::StatementKind::ValDecl { pattern, init } => {
                    let crate::Pattern::Binding { local } = pattern else {
                        return Err(HirWireError::Malformed(
                            "body statement outside the wire kernel subset",
                        ));
                    };
                    let local_slot = body.locals[*local].binding.into_raw();
                    WireStatement::ValDecl {
                        local_slot,
                        mutable: body.locals[*local].mutable,
                        init: encode_wire_expr(module, ids, type_table, type_cache, init)?,
                    }
                }
                crate::StatementKind::Assign { target, value } => {
                    let crate::AssignTarget::Local(local) = target else {
                        return Err(HirWireError::Malformed(
                            "body statement outside the wire kernel subset",
                        ));
                    };
                    WireStatement::Assign {
                        local_slot: body.locals[*local].binding.into_raw(),
                        value: encode_wire_expr(module, ids, type_table, type_cache, value)?,
                    }
                }
                crate::StatementKind::If {
                    cond,
                    then_body,
                    else_body,
                } => WireStatement::If {
                    cond: encode_wire_expr(module, ids, type_table, type_cache, cond)?,
                    then_body: encode_statement_list(
                        module,
                        ids,
                        type_table,
                        type_cache,
                        &body.locals,
                        then_body,
                    )?,
                    else_body: else_body
                        .as_ref()
                        .map(|statements| {
                            encode_statement_list(
                                module,
                                ids,
                                type_table,
                                type_cache,
                                &body.locals,
                                statements,
                            )
                        })
                        .transpose()?,
                },
                _ => {
                    return Err(HirWireError::Malformed(
                        "body statement outside the wire kernel subset",
                    ));
                }
            })
        })
        .collect()
}

/// Encodes a nested statement list against the enclosing body's locals.
fn encode_statement_list(
    module: &Module,
    ids: &mut PersistentIds<'_>,
    type_table: &mut Vec<WireTypeEntry>,
    type_cache: &mut std::collections::HashMap<crate::TypeId, u32>,
    locals: &la_arena::Arena<crate::Local>,
    statements: &[crate::Statement],
) -> Result<Vec<WireStatement>, HirWireError> {
    let mut inner = crate::Body {
        locals: locals.clone(),
        statements: statements.to_vec(),
    };
    let _ = &mut inner;
    encode_wire_statements(module, ids, type_table, type_cache, &inner)
}

fn encode_wire_expr(
    module: &Module,
    ids: &mut PersistentIds<'_>,
    type_table: &mut Vec<WireTypeEntry>,
    type_cache: &mut std::collections::HashMap<crate::TypeId, u32>,
    expr: &crate::Expr,
) -> Result<WireExpr, HirWireError> {
    let _ = type_table;
    let _ = type_cache;
    Ok(match &expr.kind {
        crate::ExprKind::StringLiteral(value) => WireExpr::StringLiteral(value.clone()),
        crate::ExprKind::BoolLiteral(value) => WireExpr::BoolLiteral(*value),
        crate::ExprKind::UnitLiteral => WireExpr::UnitLiteral,
        crate::ExprKind::IntegerLiteral(constant) => WireExpr::IntegerLiteral {
            tag: integer_tag(constant.kind()),
            bits: constant.raw_bits(),
        },
        crate::ExprKind::Local(local) => WireExpr::Local(u32::from(local.into_raw())),
        crate::ExprKind::TupleLiteral(elements) => WireExpr::TupleLiteral(
            elements
                .iter()
                .map(|element| encode_wire_expr(module, ids, type_table, type_cache, element))
                .collect::<Result<Vec<_>, _>>()?,
        ),
        crate::ExprKind::Call { callee, args } => {
            let function = crate::callable_function(module, *callee);
            let generic = matches!(
                module.functions[function].genericity,
                crate::FunctionGenericity::Generic { .. }
            );
            let callee_id = if generic {
                *ids.generic_function_id(function)
                    .ok_or(HirWireError::Malformed("identity"))?
                    .as_bytes()
            } else {
                *ids.function_id(function)
                    .ok_or(HirWireError::Malformed("identity"))?
                    .as_bytes()
            };
            WireExpr::Call {
                callee: callee_id,
                generic,
                type_arguments: Vec::new(),
                arguments: args
                    .iter()
                    .map(|argument| encode_wire_expr(module, ids, type_table, type_cache, argument))
                    .collect::<Result<Vec<_>, _>>()?,
            }
        }
        crate::ExprKind::MethodCall {
            receiver,
            callee,
            args,
        } => {
            // The callee kind travels with the persistent id of the
            // underlying function: direct member, bound callable or
            // compiler-derived equality.
            let callee_kind = match callee {
                crate::MethodCallee::Callable(_) => 1u64,
                crate::MethodCallee::Bound(_) => 2u64,
                crate::MethodCallee::DerivedEquality(_) => 3u64,
            };
            let function = crate::method_callee_function(module, *callee);
            let generic = matches!(
                module.functions[function].genericity,
                crate::FunctionGenericity::Generic { .. }
                    | crate::FunctionGenericity::OwnerParameterizedMethod { .. }
                    | crate::FunctionGenericity::GenericMethod { .. }
            );
            let callee_id = if generic {
                *ids.generic_callable_id(function)
                    .ok_or(HirWireError::Malformed("identity"))?
                    .as_bytes()
            } else {
                *ids.function_id(function)
                    .ok_or(HirWireError::Malformed("identity"))?
                    .as_bytes()
            };
            WireExpr::MethodCall {
                receiver: Box::new(encode_wire_expr(
                    module, ids, type_table, type_cache, receiver,
                )?),
                callee: callee_id,
                callee_kind,
                arguments: args
                    .iter()
                    .map(|argument| encode_wire_expr(module, ids, type_table, type_cache, argument))
                    .collect::<Result<Vec<_>, _>>()?,
            }
        }
        _ => {
            return Err(HirWireError::Malformed(
                "body expression outside the wire kernel subset",
            ));
        }
    })
}

fn write_wire_statements(writer: &mut scoop_identity::CborWriter, statements: &[WireStatement]) {
    writer.array(statements.len() as u64);
    for statement in statements {
        match statement {
            WireStatement::Expr(expr) => {
                writer.map(2);
                writer.field(1).unsigned(1);
                writer.field(2);
                write_wire_expr(writer, expr);
            }
            WireStatement::Return { value } => {
                writer.map(2);
                writer.field(1).unsigned(2);
                writer.field(2).array(match value {
                    Some(_) => 1,
                    None => 0,
                });
                if let Some(expr) = value {
                    write_wire_expr(writer, expr);
                }
            }
            WireStatement::ValDecl {
                local_slot,
                mutable,
                init,
            } => {
                writer.map(4);
                writer.field(1).unsigned(3);
                writer.field(2).unsigned(*local_slot as u64);
                writer.field(3).unsigned(u64::from(*mutable));
                writer.field(4);
                write_wire_expr(writer, init);
            }
            WireStatement::Assign { local_slot, value } => {
                writer.map(3);
                writer.field(1).unsigned(4);
                writer.field(2).unsigned(*local_slot as u64);
                writer.field(3);
                write_wire_expr(writer, value);
            }
            WireStatement::If {
                cond,
                then_body,
                else_body,
            } => {
                writer.map(4);
                writer.field(1).unsigned(5);
                writer.field(2);
                write_wire_expr(writer, cond);
                writer.field(3);
                write_wire_statements(writer, then_body);
                writer.field(4);
                if let Some(statements) = else_body {
                    write_wire_statements(writer, statements);
                } else {
                    writer.array(0);
                }
            }
        }
    }
}

fn write_wire_expr(writer: &mut scoop_identity::CborWriter, expr: &WireExpr) {
    match expr {
        WireExpr::StringLiteral(value) => {
            writer.map(2);
            writer.field(1).unsigned(1);
            writer.field(2).text(value);
        }
        WireExpr::IntegerLiteral { tag, bits } => {
            writer.map(3);
            writer.field(1).unsigned(2);
            writer.field(2).unsigned(*tag);
            writer.field(3).unsigned(*bits);
        }
        WireExpr::BoolLiteral(value) => {
            writer.map(2);
            writer.field(1).unsigned(3);
            writer.field(2).unsigned(u64::from(*value));
        }
        WireExpr::UnitLiteral => {
            writer.map(1);
            writer.field(1).unsigned(4);
        }
        WireExpr::Local(slot) => {
            writer.map(2);
            writer.field(1).unsigned(5);
            writer.field(2).unsigned(*slot as u64);
        }
        WireExpr::TupleLiteral(elements) => {
            writer.map(2);
            writer.field(1).unsigned(6);
            writer.field(2);
            write_wire_expr_list(writer, elements);
        }
        WireExpr::Call {
            callee,
            generic,
            type_arguments,
            arguments,
        } => {
            writer.map(5);
            writer.field(1).unsigned(7);
            writer.field(2).bytes(callee);
            writer.field(3).unsigned(u64::from(*generic));
            writer.field(4);
            writer.array(type_arguments.len() as u64);
            for index in type_arguments {
                writer.unsigned(*index as u64);
            }
            writer.field(5);
            write_wire_expr_list(writer, arguments);
        }
        WireExpr::MethodCall {
            receiver,
            callee,
            callee_kind,
            arguments,
        } => {
            writer.map(5);
            writer.field(1).unsigned(8);
            writer.field(2);
            write_wire_expr(writer, receiver);
            writer.field(3).bytes(callee);
            writer.field(4).unsigned(*callee_kind);
            writer.field(5);
            write_wire_expr_list(writer, arguments);
        }
    }
}

fn write_wire_expr_list(writer: &mut scoop_identity::CborWriter, exprs: &[WireExpr]) {
    writer.array(exprs.len() as u64);
    for expr in exprs {
        write_wire_expr(writer, expr);
    }
}

/// Writes one type-table entry in the canonical field order the reader
/// validates.
fn write_type_entry(writer: &mut scoop_identity::CborWriter, entry: &WireTypeEntry) {
    match entry {
        WireTypeEntry::Unit => {
            writer.map(1);
            writer.field(1).unsigned(1);
        }
        WireTypeEntry::Any => {
            writer.map(1);
            writer.field(1).unsigned(2);
        }
        WireTypeEntry::Boolean => {
            writer.map(1);
            writer.field(1).unsigned(3);
        }
        WireTypeEntry::String => {
            writer.map(1);
            writer.field(1).unsigned(4);
        }
        WireTypeEntry::Integer(tag) => {
            writer.map(2);
            writer.field(1).unsigned(5);
            writer.field(9).unsigned(tag.0);
        }
        WireTypeEntry::NominalPlain(id) => {
            writer.map(2);
            writer.field(1).unsigned(6);
            writer.field(2).bytes(id);
        }
        WireTypeEntry::NominalApplication {
            template,
            arguments,
        } => {
            writer.map(3);
            writer.field(1).unsigned(7);
            writer.field(2).bytes(template);
            writer.field(3);
            writer.array(arguments.len() as u64);
            for argument in arguments {
                writer.unsigned(*argument as u64);
            }
        }
        WireTypeEntry::Ptr(index) => {
            writer.map(2);
            writer.field(1).unsigned(8);
            writer.field(4).unsigned(*index as u64);
        }
        WireTypeEntry::Function {
            suspend,
            parameters,
            result,
        } => {
            writer.map(5);
            writer.field(1).unsigned(9);
            writer.field(6).unsigned(u64::from(*suspend));
            writer.field(7);
            writer.array(parameters.len() as u64);
            for parameter in parameters {
                writer.unsigned(*parameter as u64);
            }
            writer.field(8).unsigned(*result as u64);
        }
        WireTypeEntry::FunPtr { parameters, result } => {
            writer.map(4);
            writer.field(1).unsigned(10);
            writer.field(7);
            writer.array(parameters.len() as u64);
            for parameter in parameters {
                writer.unsigned(*parameter as u64);
            }
            writer.field(8).unsigned(*result as u64);
        }
        WireTypeEntry::Tuple(indices) => {
            writer.map(2);
            writer.field(1).unsigned(11);
            writer.field(5);
            writer.array(indices.len() as u64);
            for index in indices {
                writer.unsigned(*index as u64);
            }
        }
        WireTypeEntry::Param(slot) => {
            writer.map(2);
            writer.field(1).unsigned(12);
            writer.field(9).unsigned(*slot as u64);
        }
    }
}

/// Interns one signature type into the canonical topological table.
fn intern_wire_type(
    module: &Module,
    ids: &mut PersistentIds<'_>,
    table: &mut Vec<WireTypeEntry>,
    cache: &mut std::collections::HashMap<crate::TypeId, u32>,
    ty: crate::TypeId,
) -> Result<u32, HirWireError> {
    if let Some(index) = cache.get(&ty) {
        return Ok(*index);
    }
    let failed = || HirWireError::Malformed("type without persistent identity");
    let entry = match &module.types[ty] {
        crate::Type::Unit => WireTypeEntry::Unit,
        crate::Type::Any => WireTypeEntry::Any,
        crate::Type::Boolean => WireTypeEntry::Boolean,
        crate::Type::String => WireTypeEntry::String,
        crate::Type::Integer(kind) => WireTypeEntry::Integer(IntegerTag(integer_tag(*kind))),
        crate::Type::Ptr(pointee) => {
            WireTypeEntry::Ptr(intern_wire_type(module, ids, table, cache, *pointee)?)
        }
        crate::Type::Param(param) => WireTypeEntry::Param(param.into_raw()),
        crate::Type::Struct(application) => {
            let application = &module.struct_applications[*application];
            let template = u32::from(application.template.into_raw());
            let arguments = application.arguments.clone();
            nominal_entry(
                module,
                ids,
                table,
                cache,
                NominalKind::Struct,
                template,
                &arguments,
                failed,
            )?
        }
        crate::Type::Enum(application) => {
            let application = &module.enum_applications[*application];
            let template = u32::from(application.template.into_raw());
            let arguments = application.arguments.clone();
            nominal_entry(
                module,
                ids,
                table,
                cache,
                NominalKind::Enum,
                template,
                &arguments,
                failed,
            )?
        }
        crate::Type::Class(application) => {
            let application = &module.class_applications[*application];
            let template = u32::from(application.template.into_raw());
            let arguments = application.arguments.clone();
            nominal_entry(
                module,
                ids,
                table,
                cache,
                NominalKind::Class,
                template,
                &arguments,
                failed,
            )?
        }
        crate::Type::Interface(application) => {
            let application = &module.interface_applications[*application];
            let template = u32::from(application.template.into_raw());
            let arguments = application.arguments.clone();
            nominal_entry(
                module,
                ids,
                table,
                cache,
                NominalKind::Interface,
                template,
                &arguments,
                failed,
            )?
        }
        crate::Type::Tuple(elements) => {
            let mut indices = Vec::with_capacity(elements.len());
            for element in elements {
                indices.push(intern_wire_type(module, ids, table, cache, *element)?);
            }
            WireTypeEntry::Tuple(indices)
        }
        crate::Type::Function(function) => {
            function_entry(module, ids, table, cache, *function, false, failed)?
        }
        crate::Type::FunPtr(function) => {
            function_entry(module, ids, table, cache, *function, true, failed)?
        }
    };
    let index = table.len() as u32;
    table.push(entry);
    cache.insert(ty, index);
    Ok(index)
}

#[derive(Clone, Copy)]
enum NominalKind {
    Struct,
    Enum,
    Class,
    Interface,
}

/// Builds one nominal entry: plain declarations reference their
/// persistent type id; applications reference the generic template id
/// plus interned argument indices. Template ids are addressed by raw
/// index plus kind, keeping the four nominal arenas distinct.
#[allow(clippy::too_many_arguments)]
fn nominal_entry(
    module: &Module,
    ids: &mut PersistentIds<'_>,
    table: &mut Vec<WireTypeEntry>,
    cache: &mut std::collections::HashMap<crate::TypeId, u32>,
    kind: NominalKind,
    template: u32,
    arguments: &[crate::TypeId],
    failed: fn() -> HirWireError,
) -> Result<WireTypeEntry, HirWireError> {
    let mut argument_indices = Vec::with_capacity(arguments.len());
    for argument in arguments {
        argument_indices.push(intern_wire_type(module, ids, table, cache, *argument)?);
    }
    let template_id = crate_ids::from_raw_struct(template);
    let generic = match kind {
        NominalKind::Struct => !module.structs[template_id].type_params.is_empty(),
        NominalKind::Enum => !module.enums[crate_ids::from_raw_enum(template)]
            .type_params
            .is_empty(),
        NominalKind::Class => !module.classes[crate_ids::from_raw_class(template)]
            .type_params
            .is_empty(),
        NominalKind::Interface => !module.interfaces[crate_ids::from_raw_interface(template)]
            .type_params
            .is_empty(),
    };
    if !generic {
        let id = match kind {
            NominalKind::Struct => *ids.struct_id(template_id).ok_or_else(failed)?.as_bytes(),
            NominalKind::Enum => *ids
                .enum_id(crate_ids::from_raw_enum(template))
                .ok_or_else(failed)?
                .as_bytes(),
            NominalKind::Class => *ids
                .class_id(crate_ids::from_raw_class(template))
                .ok_or_else(failed)?
                .as_bytes(),
            NominalKind::Interface => *ids
                .interface_id(crate_ids::from_raw_interface(template))
                .ok_or_else(failed)?
                .as_bytes(),
        };
        return Ok(WireTypeEntry::NominalPlain(id));
    }
    let template_bytes: [u8; 32] = match kind {
        NominalKind::Struct => *ids
            .generic_struct_id(template_id)
            .ok_or_else(failed)?
            .as_bytes(),
        NominalKind::Enum => *ids
            .generic_enum_id(crate_ids::from_raw_enum(template))
            .ok_or_else(failed)?
            .as_bytes(),
        NominalKind::Class => *ids
            .generic_class_id(crate_ids::from_raw_class(template))
            .ok_or_else(failed)?
            .as_bytes(),
        NominalKind::Interface => *ids
            .generic_interface_id(crate_ids::from_raw_interface(template))
            .ok_or_else(failed)?
            .as_bytes(),
    };
    Ok(WireTypeEntry::NominalApplication {
        template: template_bytes,
        arguments: argument_indices,
    })
}

#[allow(clippy::too_many_arguments)]
fn function_entry(
    module: &Module,
    ids: &mut PersistentIds<'_>,
    table: &mut Vec<WireTypeEntry>,
    cache: &mut std::collections::HashMap<crate::TypeId, u32>,
    function: crate::FunctionTypeId,
    native: bool,
    failed: fn() -> HirWireError,
) -> Result<WireTypeEntry, HirWireError> {
    let _ = ids;
    let _ = failed;
    let signature = &module.function_types[function];
    let mut parameters = Vec::with_capacity(signature.parameter_types.len());
    for parameter in &signature.parameter_types {
        parameters.push(intern_wire_type(module, ids, table, cache, *parameter)?);
    }
    let result = intern_wire_type(module, ids, table, cache, signature.return_type)?;
    Ok(if native {
        WireTypeEntry::FunPtr { parameters, result }
    } else {
        WireTypeEntry::Function {
            suspend: signature.is_suspend,
            parameters,
            result,
        }
    })
}

mod ids_constructors {
    use crate::{ClassId, EnumId, InterfaceId, StructId};
    use la_arena::RawIdx;
    pub(crate) fn from_raw_struct(raw: u32) -> StructId {
        StructId::from_raw(RawIdx::from_u32(raw))
    }
    pub(crate) fn from_raw_enum(raw: u32) -> EnumId {
        EnumId::from_raw(RawIdx::from_u32(raw))
    }
    pub(crate) fn from_raw_class(raw: u32) -> ClassId {
        ClassId::from_raw(RawIdx::from_u32(raw))
    }
    pub(crate) fn from_raw_interface(raw: u32) -> InterfaceId {
        InterfaceId::from_raw(RawIdx::from_u32(raw))
    }
}
use ids_constructors as crate_ids;

fn find_generic_function(module: &Module, function: crate::FunctionId) -> crate::GenericFunctionId {
    match module.functions[function].genericity {
        crate::FunctionGenericity::Generic { definition, .. } => definition,
        _ => unreachable!("caller filters for generic functions"),
    }
}

/// The (no-GC slots, GC-free pointee slots) predicate pair of one
/// generic function template.
fn generic_function_predicates(
    module: &Module,
    function: crate::FunctionId,
) -> Option<(Vec<u32>, Vec<u32>)> {
    let definition = find_generic_function(module, function);
    let generic = &module.generic_functions[definition];
    Some((
        generic
            .no_gc_type_params
            .iter()
            .map(|param| param.into_raw())
            .collect(),
        generic
            .gc_free_pointee_requirements
            .iter()
            .map(|requirement| requirement.type_param.into_raw())
            .collect(),
    ))
}

fn cone_bytes(_module: &Module) -> [u8; 32] {
    // The single-unit model: the user Cone identity is supplied by the
    // packager context; the surface document itself does not restate
    // it in batch 1 (the Cone rides the artifact manifest).
    [0; 32]
}

fn entity_name(module: &Module, entity: ExportEntity) -> &str {
    match entity {
        ExportEntity::Function(id) => &module.functions[id].name,
        ExportEntity::Property(id) => &module.properties[id].name,
        ExportEntity::Struct(id) => &module.structs[id].name,
        ExportEntity::Enum(id) => &module.enums[id].name,
        ExportEntity::Class(id) => &module.classes[id].name,
        ExportEntity::Interface(id) => &module.interfaces[id].name,
        ExportEntity::Object(id) => &module.objects[id].name,
        ExportEntity::TypeAlias(id) => &module.type_aliases[id].name,
        ExportEntity::GenericFunction(id) => {
            &module.functions[module.generic_functions[id].function].name
        }
        ExportEntity::GenericMethod(id) => {
            &module.functions[module.generic_methods[id].function].name
        }
        _ => "",
    }
}

/// Wire decode under budgets (DESIGN 4.5 defaults).
pub fn decode_surface_wire(data: &[u8]) -> Result<DecodedHirSurfaceWire, HirWireError> {
    let mut reader = CborReader::new(data, 128);
    let mut cone = [0u8; 32];
    let mut definitions = Vec::new();
    let mut bindings = Vec::new();
    let mut public_lookup = Vec::new();
    let mut template_support = Vec::new();
    let mut types = Vec::new();
    let mut signatures = Vec::new();
    let mut predicates = Vec::new();
    let mut bodies = Vec::new();
    let mut saw_magic = false;
    {
        let mut map = reader.map().map_err(map_wire_error)?;
        while let Some(key) = map.next_key().map_err(map_wire_error)? {
            match key {
                1 => {
                    let text = map.text().map_err(map_wire_error)?;
                    if text != HIR_SURFACE_WIRE_MAGIC {
                        return Err(HirWireError::Magic);
                    }
                    saw_magic = true;
                }
                2 => {
                    let bytes = map.bytes().map_err(map_wire_error)?;
                    cone = bytes
                        .try_into()
                        .map_err(|_| HirWireError::Malformed("cone identity must be 32 bytes"))?;
                }
                3 => {
                    let mut seq = map.array().map_err(map_wire_error)?;
                    if seq.count() > 16_777_216 {
                        return Err(HirWireError::CountExceeded("definitions"));
                    }
                    for _ in 0..seq.count() {
                        let mut record = seq.map().map_err(map_wire_error)?;
                        let mut fields = 0u32;
                        let mut id = [0u8; 32];
                        let mut kind = None;
                        let mut discriminator = 0u32;
                        let mut name = String::new();
                        let mut package = Vec::new();
                        let mut purposes = WirePurposes(0);
                        while let Some(field) = record.next_key().map_err(map_wire_error)? {
                            match field {
                                1 => {
                                    let bytes = record.bytes().map_err(map_wire_error)?;
                                    id = bytes.try_into().map_err(|_| {
                                        HirWireError::Malformed("definition id must be 32 bytes")
                                    })?;
                                }
                                2 => {
                                    let tag = record.unsigned().map_err(map_wire_error)?;
                                    kind = Some(
                                        WireEntityKind::from_tag(tag)
                                            .ok_or(HirWireError::UnknownKind(tag))?,
                                    );
                                }
                                3 => {
                                    discriminator =
                                        record.unsigned().map_err(map_wire_error)? as u32;
                                }
                                4 => {
                                    name = record.text().map_err(map_wire_error)?.to_owned();
                                }
                                5 => {
                                    let mut segments = record.array().map_err(map_wire_error)?;
                                    for _ in 0..segments.count() {
                                        package.push(
                                            segments.text().map_err(map_wire_error)?.to_owned(),
                                        );
                                    }
                                }
                                6 => {
                                    let bits = record.unsigned().map_err(map_wire_error)? as u8;
                                    purposes = WirePurposes(bits);
                                }
                                _ => {
                                    return Err(HirWireError::Malformed(
                                        "unknown definition field",
                                    ));
                                }
                            }
                            fields += 1;
                        }
                        if fields != 6 {
                            return Err(HirWireError::Malformed("definition record arity"));
                        }
                        let kind =
                            kind.ok_or(HirWireError::Malformed("definition without kind"))?;
                        definitions.push(WireDefinition {
                            id,
                            kind,
                            discriminator,
                            name,
                            package,
                            purposes,
                        });
                    }
                }
                4 => {
                    let mut seq = map.array().map_err(map_wire_error)?;
                    for _ in 0..seq.count() {
                        let mut record = seq.map().map_err(map_wire_error)?;
                        let mut package = Vec::new();
                        let mut name = String::new();
                        let mut namespace = 0u64;
                        let mut roots = Vec::new();
                        while let Some(field) = record.next_key().map_err(map_wire_error)? {
                            match field {
                                1 => {
                                    let mut segments = record.array().map_err(map_wire_error)?;
                                    for _ in 0..segments.count() {
                                        package.push(
                                            segments.text().map_err(map_wire_error)?.to_owned(),
                                        );
                                    }
                                }
                                2 => name = record.text().map_err(map_wire_error)?.to_owned(),
                                3 => namespace = record.unsigned().map_err(map_wire_error)?,
                                4 => {
                                    let mut entries = record.array().map_err(map_wire_error)?;
                                    for _ in 0..entries.count() {
                                        roots.push(
                                            entries.unsigned().map_err(map_wire_error)? as u32
                                        );
                                    }
                                }
                                _ => return Err(HirWireError::Malformed("unknown binding field")),
                            }
                        }
                        bindings.push(WireBinding {
                            package,
                            name,
                            namespace,
                            roots,
                        });
                    }
                }
                5 => {
                    let mut seq = map.array().map_err(map_wire_error)?;
                    for _ in 0..seq.count() {
                        public_lookup.push(seq.unsigned().map_err(map_wire_error)? as u32);
                    }
                }
                6 => {
                    let mut seq = map.array().map_err(map_wire_error)?;
                    for _ in 0..seq.count() {
                        template_support.push(seq.unsigned().map_err(map_wire_error)? as u32);
                    }
                }
                7 => {
                    let mut seq = map.array().map_err(map_wire_error)?;
                    if seq.count() > 16_777_216 {
                        return Err(HirWireError::CountExceeded("type table"));
                    }
                    for _ in 0..seq.count() {
                        types.push(decode_type_entry(&mut seq)?);
                    }
                }
                8 => {
                    let mut seq = map.array().map_err(map_wire_error)?;
                    for _ in 0..seq.count() {
                        let mut record = seq.map().map_err(map_wire_error)?;
                        let mut definition = 0u32;
                        let mut parameters = Vec::new();
                        let mut result = 0u32;
                        while let Some(field) = record.next_key().map_err(map_wire_error)? {
                            match field {
                                1 => definition = record.unsigned().map_err(map_wire_error)? as u32,
                                2 => {
                                    let mut entries = record.array().map_err(map_wire_error)?;
                                    for _ in 0..entries.count() {
                                        parameters.push(
                                            entries.unsigned().map_err(map_wire_error)? as u32,
                                        );
                                    }
                                }
                                3 => result = record.unsigned().map_err(map_wire_error)? as u32,
                                _ => {
                                    return Err(HirWireError::Malformed("unknown signature field"));
                                }
                            }
                        }
                        signatures.push(WireSignature {
                            definition,
                            parameters,
                            result,
                        });
                    }
                }
                9 => {
                    let mut seq = map.array().map_err(map_wire_error)?;
                    for _ in 0..seq.count() {
                        let mut record = seq.map().map_err(map_wire_error)?;
                        let mut definition = 0u32;
                        let mut no_gc_slots = Vec::new();
                        let mut gc_free_pointee_slots = Vec::new();
                        while let Some(field) = record.next_key().map_err(map_wire_error)? {
                            match field {
                                1 => definition = record.unsigned().map_err(map_wire_error)? as u32,
                                2 => {
                                    let mut entries = record.array().map_err(map_wire_error)?;
                                    for _ in 0..entries.count() {
                                        no_gc_slots.push(
                                            entries.unsigned().map_err(map_wire_error)? as u32,
                                        );
                                    }
                                }
                                3 => {
                                    let mut entries = record.array().map_err(map_wire_error)?;
                                    for _ in 0..entries.count() {
                                        gc_free_pointee_slots.push(
                                            entries.unsigned().map_err(map_wire_error)? as u32,
                                        );
                                    }
                                }
                                _ => {
                                    return Err(HirWireError::Malformed("unknown predicate field"));
                                }
                            }
                        }
                        predicates.push(WirePredicate {
                            definition,
                            no_gc_slots,
                            gc_free_pointee_slots,
                        });
                    }
                }
                10 => {
                    let mut seq = map.array().map_err(map_wire_error)?;
                    for _ in 0..seq.count() {
                        let mut record = seq.map().map_err(map_wire_error)?;
                        let mut definition = 0u32;
                        let mut statements = Vec::new();
                        while let Some(field) = record.next_key().map_err(map_wire_error)? {
                            match field {
                                1 => definition = record.unsigned().map_err(map_wire_error)? as u32,
                                2 => {
                                    let mut entries = record.array().map_err(map_wire_error)?;
                                    for _ in 0..entries.count() {
                                        statements.push(decode_wire_statement(&mut entries)?);
                                    }
                                }
                                _ => return Err(HirWireError::Malformed("unknown body field")),
                            }
                        }
                        bodies.push(WireBody {
                            definition,
                            statements,
                        });
                    }
                }
                _ => return Err(HirWireError::Malformed("unknown document field")),
            }
        }
    }
    if !saw_magic {
        return Err(HirWireError::Magic);
    }
    // Structural validation: canonical definition order and the closed
    // purpose mask.
    validate_definition_order(&definitions)?;
    validate_purpose_masks(&definitions)?;
    reader.finish().map_err(map_wire_error)?;
    Ok(DecodedHirSurfaceWire {
        cone: ConeIdentity::from_bytes(&cone),
        definitions,
        bindings,
        public_lookup,
        template_structs: template_support.clone(),
        template_enums: Vec::new(),
        template_classes: Vec::new(),
        template_interfaces: Vec::new(),
        template_functions: Vec::new(),
        template_methods: Vec::new(),
        dependency_structs: Vec::new(),
        dependency_enums: Vec::new(),
        dependency_classes: Vec::new(),
        dependency_interfaces: Vec::new(),
        protected_methods: Vec::new(),
        types,
        signatures,
        predicates,
        bodies,
    })
}

/// Structural validation + typed remap + commit: the last three reader
/// steps over a decoded document.
pub fn import_surface_wire(decoded: DecodedHirSurfaceWire) -> Result<ImportedHirSet, HirWireError> {
    validate_type_graph(&decoded)?;
    for binding in &decoded.bindings {
        if binding.namespace != 1 && binding.namespace != 2 {
            return Err(HirWireError::BindingNamespace(binding.namespace));
        }
        if binding.roots.is_empty() {
            return Err(HirWireError::EmptyRoots);
        }
        for root in &binding.roots {
            if *root as usize >= decoded.definitions.len() {
                return Err(HirWireError::RootIndex(*root));
            }
        }

        if binding.roots.is_empty() {
            return Err(HirWireError::EmptyRoots);
        }
        for root in &binding.roots {
            if *root as usize >= decoded.definitions.len() {
                return Err(HirWireError::RootIndex(*root));
            }
        }
    }
    Ok(ImportedHirSet::commit(decoded))
}

/// The canonical definition-order invariant, exported for corruption
/// tests that mutate decoded documents.
pub fn validate_definition_order(definitions: &[WireDefinition]) -> Result<(), HirWireError> {
    for pair in definitions.windows(2) {
        match (pair[0].id, pair[0].kind.tag(), pair[0].discriminator).cmp(&(
            pair[1].id,
            pair[1].kind.tag(),
            pair[1].discriminator,
        )) {
            std::cmp::Ordering::Less => {}
            std::cmp::Ordering::Equal => return Err(HirWireError::DuplicateDefinition),
            std::cmp::Ordering::Greater => return Err(HirWireError::DefinitionOrder),
        }
    }
    Ok(())
}

/// The closed purpose-mask invariant.
pub fn validate_purpose_masks(definitions: &[WireDefinition]) -> Result<(), HirWireError> {
    for definition in definitions {
        if definition.purposes.0 & !WirePurposes::MASK != 0 {
            return Err(HirWireError::PurposeMask(definition.purposes.0));
        }
    }
    Ok(())
}

/// Decodes one type-table entry; the guard is the surrounding array.
fn decode_type_entry(
    seq: &mut scoop_identity::cbor::SeqGuard<'_, '_>,
) -> Result<WireTypeEntry, HirWireError> {
    let mut record = seq.map().map_err(map_wire_error)?;
    let mut tag = None;
    let mut id = None;
    let mut arguments = Vec::new();
    let mut index = None;
    let mut indices = Vec::new();
    let mut suspend = false;
    let mut parameters = Vec::new();
    let mut result = None;
    let mut slot = None;
    while let Some(field) = record.next_key().map_err(map_wire_error)? {
        match field {
            1 => tag = Some(record.unsigned().map_err(map_wire_error)?),
            2 => {
                let bytes = record.bytes().map_err(map_wire_error)?;
                id = Some(
                    bytes
                        .try_into()
                        .map_err(|_| HirWireError::Malformed("template id must be 32 bytes"))?,
                );
            }
            3 => {
                let mut entries = record.array().map_err(map_wire_error)?;
                for _ in 0..entries.count() {
                    arguments.push(entries.unsigned().map_err(map_wire_error)? as u32);
                }
            }
            4 => index = Some(record.unsigned().map_err(map_wire_error)? as u32),
            5 => {
                let mut entries = record.array().map_err(map_wire_error)?;
                for _ in 0..entries.count() {
                    indices.push(entries.unsigned().map_err(map_wire_error)? as u32);
                }
            }
            6 => suspend = record.unsigned().map_err(map_wire_error)? != 0,
            7 => {
                let mut entries = record.array().map_err(map_wire_error)?;
                for _ in 0..entries.count() {
                    parameters.push(entries.unsigned().map_err(map_wire_error)? as u32);
                }
            }
            8 => result = Some(record.unsigned().map_err(map_wire_error)? as u32),
            9 => slot = Some(record.unsigned().map_err(map_wire_error)? as u32),
            _ => return Err(HirWireError::Malformed("unknown type entry field")),
        }
    }
    let tag = tag.ok_or(HirWireError::Malformed("type entry without tag"))?;
    let entry = match tag {
        1 => WireTypeEntry::Unit,
        2 => WireTypeEntry::Any,
        3 => WireTypeEntry::Boolean,
        4 => WireTypeEntry::String,
        5 => WireTypeEntry::Integer(IntegerTag(
            slot.ok_or(HirWireError::Malformed("integer entry without tag value"))? as u64,
        )),
        6 => WireTypeEntry::NominalPlain(
            id.ok_or(HirWireError::Malformed("nominal entry without id"))?,
        ),
        7 => WireTypeEntry::NominalApplication {
            template: id.ok_or(HirWireError::Malformed("application without template"))?,
            arguments,
        },
        8 => WireTypeEntry::Ptr(index.ok_or(HirWireError::Malformed("pointer without pointee"))?),
        9 | 10 => {
            let result = result.ok_or(HirWireError::Malformed("function entry without result"))?;
            if tag == 9 {
                WireTypeEntry::Function {
                    suspend,
                    parameters,
                    result,
                }
            } else {
                WireTypeEntry::FunPtr { parameters, result }
            }
        }
        11 => WireTypeEntry::Tuple(indices),
        12 => {
            WireTypeEntry::Param(slot.ok_or(HirWireError::Malformed("param entry without slot"))?)
        }
        _ => return Err(HirWireError::UnknownKind(tag)),
    };
    Ok(entry)
}

/// Validates that every type-table reference points at an earlier
/// entry (the table is topologically ordered by construction) and that
/// all signature references are in range.
pub fn validate_type_graph(decoded: &DecodedHirSurfaceWire) -> Result<(), HirWireError> {
    let count = decoded.types.len() as u32;
    let earlier = |entries: &[u32]| {
        entries
            .iter()
            .all(|index| *index < count)
            .then_some(())
            .ok_or(HirWireError::RootIndex(*entries.iter().max().unwrap_or(&0)))
    };
    for (position, entry) in decoded.types.iter().enumerate() {
        let bound = position as u32;
        let in_range = |index: u32| {
            if index < bound {
                Ok(())
            } else {
                Err(HirWireError::Malformed("non-topological type table"))
            }
        };
        match entry {
            WireTypeEntry::Ptr(index) => {
                in_range(*index)?;
            }
            WireTypeEntry::NominalApplication { arguments, .. } => {
                for index in arguments {
                    if *index >= bound {
                        return Err(HirWireError::Malformed("non-topological type table"));
                    }
                }
            }
            WireTypeEntry::Function {
                parameters, result, ..
            }
            | WireTypeEntry::FunPtr { parameters, result } => {
                let _ = earlier(parameters);
                for index in parameters {
                    if *index >= bound {
                        return Err(HirWireError::Malformed("non-topological type table"));
                    }
                }
                if *result >= bound {
                    return Err(HirWireError::Malformed("non-topological type table"));
                }
            }
            WireTypeEntry::Tuple(indices) => {
                for index in indices {
                    if *index >= bound {
                        return Err(HirWireError::Malformed("non-topological type table"));
                    }
                }
            }
            _ => {}
        }
    }
    for signature in &decoded.signatures {
        if signature.definition >= decoded.definitions.len() as u32 {
            return Err(HirWireError::RootIndex(signature.definition));
        }
        let mut refs = signature.parameters.clone();
        refs.push(signature.result);
        earlier(&refs)?;
    }
    for predicate in &decoded.predicates {
        if predicate.definition >= decoded.definitions.len() as u32 {
            return Err(HirWireError::RootIndex(predicate.definition));
        }
    }
    for body in &decoded.bodies {
        if body.definition >= decoded.definitions.len() as u32 {
            return Err(HirWireError::RootIndex(body.definition));
        }
    }
    Ok(())
}

fn decode_wire_statement(
    seq: &mut scoop_identity::cbor::SeqGuard<'_, '_>,
) -> Result<WireStatement, HirWireError> {
    let mut record = seq.map().map_err(map_wire_error)?;
    if record.next_key().map_err(map_wire_error)? != Some(1) {
        return Err(HirWireError::Malformed("statement must lead with its tag"));
    }
    let tag = record.unsigned().map_err(map_wire_error)?;
    fn next(record: &mut scoop_identity::cbor::MapGuard<'_, '_>) -> Result<u64, HirWireError> {
        record
            .next_key()
            .map_err(map_wire_error)?
            .ok_or(HirWireError::Malformed("statement truncated"))
    }
    match tag {
        1 => {
            if next(&mut record)? != 2 {
                return Err(HirWireError::Malformed("expr statement field order"));
            }
            Ok(WireStatement::Expr(decode_wire_expr(&mut record)?))
        }
        2 => {
            if next(&mut record)? != 2 {
                return Err(HirWireError::Malformed("return field order"));
            }
            let value = {
                let mut entries = record.array().map_err(map_wire_error)?;
                if entries.count() > 1 {
                    return Err(HirWireError::Malformed("return carries at most one value"));
                }
                if entries.count() == 1 {
                    Some(decode_wire_expr(&mut entries)?)
                } else {
                    None
                }
            };
            Ok(WireStatement::Return { value })
        }
        3 => {
            if next(&mut record)? != 2 {
                return Err(HirWireError::Malformed("val field order"));
            }
            let local_slot = record.unsigned().map_err(map_wire_error)? as u32;
            if next(&mut record)? != 3 {
                return Err(HirWireError::Malformed("val field order"));
            }
            let mutable = record.unsigned().map_err(map_wire_error)? != 0;
            if next(&mut record)? != 4 {
                return Err(HirWireError::Malformed("val field order"));
            }
            let init = decode_wire_expr(&mut record)?;
            Ok(WireStatement::ValDecl {
                local_slot,
                mutable,
                init,
            })
        }
        4 => {
            if next(&mut record)? != 2 {
                return Err(HirWireError::Malformed("assign field order"));
            }
            let local_slot = record.unsigned().map_err(map_wire_error)? as u32;
            if next(&mut record)? != 3 {
                return Err(HirWireError::Malformed("assign field order"));
            }
            let value = decode_wire_expr(&mut record)?;
            Ok(WireStatement::Assign { local_slot, value })
        }
        5 => {
            if next(&mut record)? != 2 {
                return Err(HirWireError::Malformed("if field order"));
            }
            let cond = decode_wire_expr(&mut record)?;
            if next(&mut record)? != 3 {
                return Err(HirWireError::Malformed("if field order"));
            }
            let then_body = {
                let mut then_entries = record.array().map_err(map_wire_error)?;
                let mut body = Vec::new();
                for _ in 0..then_entries.count() {
                    body.push(decode_wire_statement(&mut then_entries)?);
                }
                body
            };
            if next(&mut record)? != 4 {
                return Err(HirWireError::Malformed("if field order"));
            }
            let else_statements = {
                let mut else_entries = record.array().map_err(map_wire_error)?;
                let mut body = Vec::new();
                for _ in 0..else_entries.count() {
                    body.push(decode_wire_statement(&mut else_entries)?);
                }
                body
            };
            let else_body = if else_statements.is_empty() {
                None
            } else {
                Some(else_statements)
            };
            Ok(WireStatement::If {
                cond,
                then_body,
                else_body,
            })
        }
        _ => Err(HirWireError::UnknownKind(tag)),
    }
}

fn decode_wire_expr<'a>(
    record: &mut impl core::ops::DerefMut<Target = scoop_identity::CborReader<'a>>,
) -> Result<WireExpr, HirWireError> {
    let mut inner = record.map().map_err(map_wire_error)?;
    if inner.next_key().map_err(map_wire_error)? != Some(1) {
        return Err(HirWireError::Malformed("expression must lead with its tag"));
    }
    let tag = inner.unsigned().map_err(map_wire_error)?;
    fn next(inner: &mut scoop_identity::cbor::MapGuard<'_, '_>) -> Result<u64, HirWireError> {
        inner
            .next_key()
            .map_err(map_wire_error)?
            .ok_or(HirWireError::Malformed("expression truncated"))
    }
    let result = match tag {
        1 => {
            if next(&mut inner)? != 2 {
                return Err(HirWireError::Malformed("string literal field order"));
            }
            WireExpr::StringLiteral(inner.text().map_err(map_wire_error)?.to_owned())
        }
        2 => {
            if next(&mut inner)? != 2 {
                return Err(HirWireError::Malformed("integer literal field order"));
            }
            let tag_value = inner.unsigned().map_err(map_wire_error)?;
            if next(&mut inner)? != 3 {
                return Err(HirWireError::Malformed("integer literal field order"));
            }
            let bits = inner.unsigned().map_err(map_wire_error)?;
            WireExpr::IntegerLiteral {
                tag: tag_value,
                bits,
            }
        }
        3 => {
            if next(&mut inner)? != 2 {
                return Err(HirWireError::Malformed("bool literal field order"));
            }
            WireExpr::BoolLiteral(inner.unsigned().map_err(map_wire_error)? != 0)
        }
        4 => WireExpr::UnitLiteral,
        5 => {
            if next(&mut inner)? != 2 {
                return Err(HirWireError::Malformed("local field order"));
            }
            WireExpr::Local(inner.unsigned().map_err(map_wire_error)? as u32)
        }
        6 => {
            if next(&mut inner)? != 2 {
                return Err(HirWireError::Malformed("tuple field order"));
            }
            let elements = {
                let mut entries = inner.array().map_err(map_wire_error)?;
                let mut values = Vec::new();
                for _ in 0..entries.count() {
                    values.push(decode_wire_expr(&mut entries)?);
                }
                values
            };
            WireExpr::TupleLiteral(elements)
        }
        7 => {
            if next(&mut inner)? != 2 {
                return Err(HirWireError::Malformed("call field order"));
            }
            let bytes = inner.bytes().map_err(map_wire_error)?;
            let callee: [u8; 32] = bytes
                .try_into()
                .map_err(|_| HirWireError::Malformed("callee id must be 32 bytes"))?;
            if next(&mut inner)? != 3 {
                return Err(HirWireError::Malformed("call field order"));
            }
            let generic = inner.unsigned().map_err(map_wire_error)? != 0;
            if next(&mut inner)? != 4 {
                return Err(HirWireError::Malformed("call field order"));
            }
            let type_arguments = {
                let mut type_entries = inner.array().map_err(map_wire_error)?;
                let mut arguments = Vec::new();
                for _ in 0..type_entries.count() {
                    arguments.push(type_entries.unsigned().map_err(map_wire_error)? as u32);
                }
                arguments
            };
            if next(&mut inner)? != 5 {
                return Err(HirWireError::Malformed("call field order"));
            }
            let arguments = {
                let mut entries = inner.array().map_err(map_wire_error)?;
                let mut values = Vec::new();
                for _ in 0..entries.count() {
                    values.push(decode_wire_expr(&mut entries)?);
                }
                values
            };
            WireExpr::Call {
                callee,
                generic,
                type_arguments,
                arguments,
            }
        }
        8 => {
            if next(&mut inner)? != 2 {
                return Err(HirWireError::Malformed("method call field order"));
            }
            let receiver = Box::new(decode_wire_expr(&mut inner)?);
            if next(&mut inner)? != 3 {
                return Err(HirWireError::Malformed("method call field order"));
            }
            let bytes = inner.bytes().map_err(map_wire_error)?;
            let callee: [u8; 32] = bytes
                .try_into()
                .map_err(|_| HirWireError::Malformed("callee id must be 32 bytes"))?;
            if next(&mut inner)? != 4 {
                return Err(HirWireError::Malformed("method call field order"));
            }
            let callee_kind = inner.unsigned().map_err(map_wire_error)?;
            if next(&mut inner)? != 5 {
                return Err(HirWireError::Malformed("method call field order"));
            }
            let arguments = {
                let mut entries = inner.array().map_err(map_wire_error)?;
                let mut values = Vec::new();
                for _ in 0..entries.count() {
                    values.push(decode_wire_expr(&mut entries)?);
                }
                values
            };
            WireExpr::MethodCall {
                receiver,
                callee,
                callee_kind,
                arguments,
            }
        }
        _ => return Err(HirWireError::UnknownKind(tag)),
    };
    Ok(result)
}

fn map_wire_error(error: scoop_identity::CborError) -> HirWireError {
    match error {
        scoop_identity::CborError::UnexpectedEof => HirWireError::Truncated,
        scoop_identity::CborError::TrailingBytes(_) => HirWireError::TrailingBytes,

        _ => HirWireError::Malformed("malformed canonical CBOR"),
    }
}
