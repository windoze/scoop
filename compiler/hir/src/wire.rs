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
use crate::{ExportEntity, Module, PersistentIds};

/// Wire schema domain tag; readers reject any other magic.
pub const HIR_SURFACE_WIRE_MAGIC: &str = "scoop-hir-surface-wire-v1";

/// The closed wire kind set, mirroring [`ExportEntity`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    for (entity, purposes) in &surfaces.purposes {
        let (kind, id, discriminator) = resolve(*entity, ids)?;
        let name = entity_name(module, *entity).to_owned();
        let package = entity_package(module, entity);
        table.push(WireDefinition {
            id,
            kind,
            discriminator,
            name,
            package,
            purposes: WirePurposes::from_export(purposes),
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
    let mut writer = CborWriter::new();
    writer.map(6);
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
    Ok(writer.into_bytes())
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

fn entity_package(_module: &Module, _entity: &ExportEntity) -> Vec<String> {
    // Batch 1 carries the empty root package; per-entity package
    // provenance rides the binding entries.
    Vec::new()
}

/// Wire decode under budgets (DESIGN 4.5 defaults).
pub fn decode_surface_wire(data: &[u8]) -> Result<DecodedHirSurfaceWire, HirWireError> {
    let mut reader = CborReader::new(data, 128);
    let mut cone = [0u8; 32];
    let mut definitions = Vec::new();
    let mut bindings = Vec::new();
    let mut public_lookup = Vec::new();
    let mut template_support = Vec::new();
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
                                    return Err(HirWireError::Malformed("unknown definition field"));
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
    })
}

/// Structural validation + typed remap + commit: the last three reader
/// steps over a decoded document.
pub fn import_surface_wire(decoded: DecodedHirSurfaceWire) -> Result<ImportedHirSet, HirWireError> {
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

fn map_wire_error(error: scoop_identity::CborError) -> HirWireError {
    match error {
        scoop_identity::CborError::UnexpectedEof => HirWireError::Truncated,
        scoop_identity::CborError::TrailingBytes(_) => HirWireError::TrailingBytes,

        _ => HirWireError::Malformed("malformed canonical CBOR"),
    }
}
