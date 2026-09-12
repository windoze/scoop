use std::fmt;

use scoop_identity::{
    CLayoutByteAlignment, CLayoutOverride, CanonicalIdentifier, ConeIdentity, DeclarationScope,
    DecodedPersistentId, DefinitionOwnerChain, EnumVariantFieldKey, EnumVariantFieldSelector,
    EnumVariantIdentityKey, FieldIdentityKey, PackagePath, PersistentEnumVariantFieldId,
    PersistentEnumVariantId, PersistentFieldId, PersistentGenericTypeId, PersistentId,
    PersistentIdResolver, PersistentKeyResolver, PersistentTypeId, SignatureTypeKey,
    SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind,
};
use scoop_wire::{DecodeLimits, decode_canonical, encode};

use super::{
    DecodedNativeBoundaryTypeDefinitionRecord, NativeBoundaryCLayoutPolicy,
    NativeBoundaryDefinitionError, NativeBoundaryFieldDefinition, NativeBoundaryNominalShape,
    NativeBoundaryResolver, NativeBoundaryTypeDefinitionRecord, NativeBoundaryVariantDefinition,
    NativeBoundaryVariantFieldDefinition,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ResolutionError;

impl fmt::Display for ResolutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("test reference does not exist")
    }
}

impl std::error::Error for ResolutionError {}

#[derive(Clone)]
struct Resolver {
    types: Vec<(PersistentTypeId, SourceDeclarationKey)>,
    generic_types: Vec<(PersistentGenericTypeId, SourceDeclarationKey)>,
    fields: Vec<(PersistentFieldId, FieldIdentityKey)>,
    variants: Vec<(PersistentEnumVariantId, EnumVariantIdentityKey)>,
    variant_fields: Vec<(PersistentEnumVariantFieldId, EnumVariantFieldKey)>,
}

impl PersistentIdResolver<PersistentTypeId> for Resolver {
    type Error = ResolutionError;

    fn resolve(
        &mut self,
        decoded: DecodedPersistentId<PersistentTypeId>,
    ) -> Result<PersistentTypeId, Self::Error> {
        resolve_id(decoded, self.types.iter().map(|(id, _)| *id))
    }
}

impl PersistentIdResolver<PersistentGenericTypeId> for Resolver {
    type Error = ResolutionError;

    fn resolve(
        &mut self,
        decoded: DecodedPersistentId<PersistentGenericTypeId>,
    ) -> Result<PersistentGenericTypeId, Self::Error> {
        resolve_id(decoded, self.generic_types.iter().map(|(id, _)| *id))
    }
}

macro_rules! resolve_fixture_key {
    ($id:ty, $key:ty, $field:ident) => {
        impl PersistentKeyResolver<$id, $key> for Resolver {
            type Error = ResolutionError;

            fn resolve_key(
                &mut self,
                decoded: DecodedPersistentId<$id>,
            ) -> Result<std::sync::Arc<$key>, Self::Error> {
                self.$field
                    .iter()
                    .find_map(|(id, key)| {
                        decoded
                            .verify(*id)
                            .ok()
                            .map(|_| std::sync::Arc::new(key.clone()))
                    })
                    .ok_or(ResolutionError)
            }
        }
    };
}

resolve_fixture_key!(PersistentTypeId, SourceDeclarationKey, types);
resolve_fixture_key!(PersistentGenericTypeId, SourceDeclarationKey, generic_types);
resolve_fixture_key!(PersistentFieldId, FieldIdentityKey, fields);
resolve_fixture_key!(PersistentEnumVariantId, EnumVariantIdentityKey, variants);
resolve_fixture_key!(
    PersistentEnumVariantFieldId,
    EnumVariantFieldKey,
    variant_fields
);

impl NativeBoundaryResolver<ResolutionError> for Resolver {
    fn native_boundary_type_parameter_count(
        &mut self,
        declaration: &SourceDeclarationKey,
    ) -> Result<u32, ResolutionError> {
        Ok(declaration.duplicate_signature().type_parameter_count())
    }
}

#[test]
fn reference_struct_and_enum_shapes_roundtrip_without_reordering() {
    let fixture = Fixture::new();
    for record in [
        fixture.reference_record(),
        fixture.struct_record(),
        fixture.enum_record(),
        fixture.generic_struct_record(),
    ] {
        let bytes = encode(&record).unwrap();
        let decoded = decode_canonical::<DecodedNativeBoundaryTypeDefinitionRecord>(
            &bytes,
            DecodeLimits::default(),
        )
        .unwrap();
        assert_eq!(encode(&decoded).unwrap(), bytes);
        assert_eq!(decoded.resolve(&mut fixture.resolver()).unwrap(), record);
    }

    let NativeBoundaryNominalShape::Struct { fields, .. } = fixture.struct_record().shape().clone()
    else {
        panic!("fixture must be a struct")
    };
    assert_eq!(fields[0].field(), fixture.fields[0].0);
    assert_eq!(fields[1].field(), fixture.fields[1].0);
}

#[test]
fn constructors_reject_wrong_shape_and_cross_owner_members() {
    let fixture = Fixture::new();
    assert_eq!(
        NativeBoundaryTypeDefinitionRecord::new(
            &fixture.class.1,
            &[0],
            NativeBoundaryNominalShape::Struct {
                c_layout: NativeBoundaryCLayoutPolicy::NotCLayout,
                fields: Vec::new(),
            },
        ),
        Err(NativeBoundaryDefinitionError::ShapeKindMismatch)
    );

    let foreign_field =
        NativeBoundaryFieldDefinition::new(&fixture.foreign_field.1, fixture.value_type()).unwrap();
    assert_eq!(
        NativeBoundaryTypeDefinitionRecord::new(
            &fixture.structure.1,
            &[0],
            NativeBoundaryNominalShape::Struct {
                c_layout: NativeBoundaryCLayoutPolicy::NotCLayout,
                fields: vec![foreign_field],
            },
        ),
        Err(NativeBoundaryDefinitionError::FieldOwnerMismatch)
    );
}

#[test]
fn constructors_reject_duplicate_fields_variants_and_variant_fields() {
    let fixture = Fixture::new();
    let field =
        NativeBoundaryFieldDefinition::new(&fixture.fields[0].1, fixture.value_type()).unwrap();
    assert_eq!(
        NativeBoundaryTypeDefinitionRecord::new(
            &fixture.structure.1,
            &[0],
            NativeBoundaryNominalShape::Struct {
                c_layout: NativeBoundaryCLayoutPolicy::NotCLayout,
                fields: vec![field.clone(), field],
            },
        ),
        Err(NativeBoundaryDefinitionError::DuplicateField)
    );

    let variant_field = NativeBoundaryVariantFieldDefinition::new(
        &fixture.variant_fields[0].1,
        fixture.value_type(),
    )
    .unwrap();
    assert_eq!(
        NativeBoundaryVariantDefinition::new(
            &fixture.variants[1].1,
            vec![variant_field.clone(), variant_field],
        ),
        Err(NativeBoundaryDefinitionError::DuplicateVariantField)
    );

    let variant = NativeBoundaryVariantDefinition::new(&fixture.variants[0].1, Vec::new()).unwrap();
    assert_eq!(
        NativeBoundaryTypeDefinitionRecord::new(
            &fixture.enumeration.1,
            &[0],
            NativeBoundaryNominalShape::Enum {
                variants: vec![variant.clone(), variant],
            },
        ),
        Err(NativeBoundaryDefinitionError::DuplicateVariant)
    );
}

#[test]
fn field_binders_must_fit_the_owner_stack() {
    let fixture = Fixture::new();
    let key = FieldIdentityKey::source_declared(
        &fixture.generic_struct.1,
        CanonicalIdentifier::new("value").unwrap(),
    )
    .unwrap();
    let shape = |ty| NativeBoundaryNominalShape::Struct {
        c_layout: NativeBoundaryCLayoutPolicy::NotCLayout,
        fields: vec![NativeBoundaryFieldDefinition::new(&key, ty).unwrap()],
    };

    assert!(
        NativeBoundaryTypeDefinitionRecord::new(
            &fixture.generic_struct.1,
            &[1],
            shape(SignatureTypeKey::Binder { depth: 0, index: 0 }),
        )
        .is_ok()
    );
    assert_eq!(
        NativeBoundaryTypeDefinitionRecord::new(
            &fixture.generic_struct.1,
            &[1],
            shape(SignatureTypeKey::Binder { depth: 0, index: 1 }),
        ),
        Err(NativeBoundaryDefinitionError::BinderIndexOutOfRange {
            depth: 0,
            index: 1,
            count: 1,
        })
    );
    assert_eq!(
        NativeBoundaryTypeDefinitionRecord::new(
            &fixture.generic_struct.1,
            &[1],
            shape(SignatureTypeKey::Binder { depth: 1, index: 0 }),
        ),
        Err(NativeBoundaryDefinitionError::BinderDepthOutOfRange { depth: 1 })
    );
}

#[test]
fn decoded_record_rejects_an_owner_count_mismatch() {
    let fixture = Fixture::new();
    let record = fixture.generic_struct_record();
    let mut bytes = encode(&record).unwrap();
    let owner_end = bytes
        .windows(34)
        .position(|window| window[0] == 0x58 && window[1] == 0x20)
        .unwrap()
        + 34;
    assert_eq!(&bytes[owner_end..owner_end + 2], &[0x02, 0x01]);
    bytes[owner_end + 1] = 2;
    let decoded = decode_canonical::<DecodedNativeBoundaryTypeDefinitionRecord>(
        &bytes,
        DecodeLimits::default(),
    )
    .unwrap();
    assert!(decoded.resolve(&mut fixture.resolver()).is_err());
}

#[test]
fn decoded_record_rejects_a_field_from_another_owner() {
    let fixture = Fixture::new();
    let foreign =
        NativeBoundaryFieldDefinition::new(&fixture.foreign_field.1, fixture.value_type()).unwrap();
    let foreign_record = NativeBoundaryTypeDefinitionRecord::new(
        &fixture.foreign_struct.1,
        &[0],
        NativeBoundaryNominalShape::Struct {
            c_layout: NativeBoundaryCLayoutPolicy::NotCLayout,
            fields: vec![foreign],
        },
    )
    .unwrap();
    let mut bytes = encode(&foreign_record).unwrap();
    replace_once(
        &mut bytes,
        fixture.foreign_struct.0.as_array(),
        fixture.structure.0.as_array(),
    );
    let decoded = decode_canonical::<DecodedNativeBoundaryTypeDefinitionRecord>(
        &bytes,
        DecodeLimits::default(),
    )
    .unwrap();
    assert!(decoded.resolve(&mut fixture.resolver()).is_err());
}

#[test]
fn unknown_shape_tag_is_rejected() {
    let fixture = Fixture::new();
    let mut bytes = encode(&fixture.reference_record()).unwrap();
    let tag = bytes
        .windows(3)
        .rposition(|window| window == [0xa1, 0x00, 0x01])
        .unwrap()
        + 2;
    bytes[tag] = 4;
    assert!(
        decode_canonical::<DecodedNativeBoundaryTypeDefinitionRecord>(
            &bytes,
            DecodeLimits::default(),
        )
        .is_err()
    );
}

struct Fixture {
    class: (PersistentTypeId, SourceDeclarationKey),
    structure: (PersistentTypeId, SourceDeclarationKey),
    foreign_struct: (PersistentTypeId, SourceDeclarationKey),
    generic_struct: (PersistentGenericTypeId, SourceDeclarationKey),
    enumeration: (PersistentTypeId, SourceDeclarationKey),
    fields: Vec<(PersistentFieldId, FieldIdentityKey)>,
    foreign_field: (PersistentFieldId, FieldIdentityKey),
    variants: Vec<(PersistentEnumVariantId, EnumVariantIdentityKey)>,
    variant_fields: Vec<(PersistentEnumVariantFieldId, EnumVariantFieldKey)>,
}

impl Fixture {
    fn new() -> Self {
        let class = declaration("Value", SourceNominalKind::Class, 0);
        let structure = declaration("Pair", SourceNominalKind::Struct, 0);
        let foreign_struct = declaration("Other", SourceNominalKind::Struct, 0);
        let generic_struct = declaration("Boxed", SourceNominalKind::Struct, 1);
        let enumeration = declaration("Choice", SourceNominalKind::Enum, 0);

        let fields = ["left", "right"]
            .into_iter()
            .map(|name| {
                let key = FieldIdentityKey::source_declared(
                    &structure,
                    CanonicalIdentifier::new(name).unwrap(),
                )
                .unwrap();
                (PersistentFieldId::from_key(&key).unwrap(), key)
            })
            .collect();
        let foreign_field_key = FieldIdentityKey::source_declared(
            &foreign_struct,
            CanonicalIdentifier::new("value").unwrap(),
        )
        .unwrap();
        let foreign_field = (
            PersistentFieldId::from_key(&foreign_field_key).unwrap(),
            foreign_field_key,
        );
        let variants = ["None", "Some"]
            .into_iter()
            .map(|name| {
                let key = EnumVariantIdentityKey::source(
                    &enumeration,
                    CanonicalIdentifier::new(name).unwrap(),
                )
                .unwrap();
                (PersistentEnumVariantId::from_key(&key).unwrap(), key)
            })
            .collect::<Vec<_>>();
        let variant_field_key = EnumVariantFieldKey::new(
            variants[1].0,
            EnumVariantFieldSelector::Named(CanonicalIdentifier::new("value").unwrap()),
        );
        let variant_fields = vec![(
            PersistentEnumVariantFieldId::from_key(&variant_field_key).unwrap(),
            variant_field_key,
        )];

        Self {
            class: concrete(class),
            structure: concrete(structure),
            foreign_struct: concrete(foreign_struct),
            generic_struct: generic(generic_struct),
            enumeration: concrete(enumeration),
            fields,
            foreign_field,
            variants,
            variant_fields,
        }
    }

    fn resolver(&self) -> Resolver {
        Resolver {
            types: vec![
                self.class.clone(),
                self.structure.clone(),
                self.foreign_struct.clone(),
                self.enumeration.clone(),
            ],
            generic_types: vec![self.generic_struct.clone()],
            fields: self
                .fields
                .iter()
                .cloned()
                .chain([self.foreign_field.clone()])
                .collect(),
            variants: self.variants.clone(),
            variant_fields: self.variant_fields.clone(),
        }
    }

    fn value_type(&self) -> SignatureTypeKey {
        SignatureTypeKey::Nominal(self.class.0)
    }

    fn reference_record(&self) -> NativeBoundaryTypeDefinitionRecord {
        NativeBoundaryTypeDefinitionRecord::new(
            &self.class.1,
            &[0],
            NativeBoundaryNominalShape::Reference,
        )
        .unwrap()
    }

    fn struct_record(&self) -> NativeBoundaryTypeDefinitionRecord {
        NativeBoundaryTypeDefinitionRecord::new(
            &self.structure.1,
            &[0],
            NativeBoundaryNominalShape::Struct {
                c_layout: NativeBoundaryCLayoutPolicy::CLayout {
                    aligned: CLayoutOverride::Bytes(CLayoutByteAlignment::Bytes8),
                    packed: CLayoutOverride::Natural,
                },
                fields: self
                    .fields
                    .iter()
                    .map(|(_, key)| {
                        NativeBoundaryFieldDefinition::new(key, self.value_type()).unwrap()
                    })
                    .collect(),
            },
        )
        .unwrap()
    }

    fn generic_struct_record(&self) -> NativeBoundaryTypeDefinitionRecord {
        NativeBoundaryTypeDefinitionRecord::new(
            &self.generic_struct.1,
            &[1],
            NativeBoundaryNominalShape::Struct {
                c_layout: NativeBoundaryCLayoutPolicy::NotCLayout,
                fields: Vec::new(),
            },
        )
        .unwrap()
    }

    fn enum_record(&self) -> NativeBoundaryTypeDefinitionRecord {
        NativeBoundaryTypeDefinitionRecord::new(
            &self.enumeration.1,
            &[0],
            NativeBoundaryNominalShape::Enum {
                variants: vec![
                    NativeBoundaryVariantDefinition::new(&self.variants[0].1, Vec::new()).unwrap(),
                    NativeBoundaryVariantDefinition::new(
                        &self.variants[1].1,
                        vec![
                            NativeBoundaryVariantFieldDefinition::new(
                                &self.variant_fields[0].1,
                                self.value_type(),
                            )
                            .unwrap(),
                        ],
                    )
                    .unwrap(),
                ],
            },
        )
        .unwrap()
    }
}

fn declaration(
    name: &str,
    kind: SourceNominalKind,
    type_parameter_count: u32,
) -> SourceDeclarationKey {
    SourceDeclarationKey::nominal(
        SourceDeclarationSite::new(
            ConeIdentity::CORE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap(),
        CanonicalIdentifier::new(name).unwrap(),
        kind,
        type_parameter_count,
    )
}

fn concrete(key: SourceDeclarationKey) -> (PersistentTypeId, SourceDeclarationKey) {
    (
        PersistentTypeId::from_source_declaration(&key).unwrap(),
        key,
    )
}

fn generic(key: SourceDeclarationKey) -> (PersistentGenericTypeId, SourceDeclarationKey) {
    (
        PersistentGenericTypeId::from_source_declaration(&key).unwrap(),
        key,
    )
}

fn resolve_id<I: PersistentId>(
    decoded: DecodedPersistentId<I>,
    ids: impl IntoIterator<Item = I>,
) -> Result<I, ResolutionError> {
    ids.into_iter()
        .find_map(|id| decoded.verify(id).ok())
        .ok_or(ResolutionError)
}

fn replace_once(bytes: &mut [u8], from: &[u8; 32], to: &[u8; 32]) {
    let position = bytes
        .windows(from.len())
        .position(|window| window == from)
        .unwrap();
    bytes[position..position + to.len()].copy_from_slice(to);
}
