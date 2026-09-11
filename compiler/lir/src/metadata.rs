use super::*;

#[derive(Debug)]
pub struct StructDef {
    pub name: String,
    pub size: u64,
    pub align: u64,
    pub interior_mutable: bool,
    pub representation: StructRepresentation,
}

/// Immutable-by-classification struct definition store. C-layout references
/// can only be minted here, and the only post-allocation updates preserve the
/// representation family that justified the refinement.
#[derive(Debug, Default)]
pub struct StructDefs {
    definitions: Arena<StructDef>,
}

impl StructDefs {
    pub fn alloc_scoop(
        &mut self,
        name: String,
        size: u64,
        align: u64,
        interior_mutable: bool,
        fields: Vec<StructField>,
    ) -> StructDefId {
        self.definitions.alloc(StructDef {
            name,
            size,
            align,
            interior_mutable,
            representation: StructRepresentation::Scoop { fields },
        })
    }

    pub fn alloc_c(
        &mut self,
        name: String,
        size: u64,
        align: u64,
        interior_mutable: bool,
        contract: LirCLayoutContract,
        fields: Vec<CStructField>,
    ) -> CStructRef {
        let id = self.definitions.alloc(StructDef {
            name,
            size,
            align,
            interior_mutable,
            representation: StructRepresentation::C { contract, fields },
        });
        CStructRef::from_validated_definition(id)
    }

    pub fn alloc_intrinsic(
        &mut self,
        name: String,
        size: u64,
        align: u64,
        representation: IntrinsicTypeRepresentation,
    ) -> StructDefId {
        self.definitions.alloc(StructDef {
            name,
            size,
            align,
            interior_mutable: false,
            representation: StructRepresentation::Intrinsic(representation),
        })
    }

    pub fn set_scoop_fields(&mut self, id: StructDefId, fields: Vec<StructField>) {
        let StructRepresentation::Scoop {
            fields: stored_fields,
        } = &mut self.definitions[id].representation
        else {
            panic!("only a Scoop struct shell accepts Scoop fields")
        };
        *stored_fields = fields;
    }

    pub fn set_c_fields(&mut self, reference: CStructRef, fields: Vec<CStructField>) {
        let StructRepresentation::C {
            fields: stored_fields,
            ..
        } = &mut self.definitions[reference.definition()].representation
        else {
            unreachable!("CStructRef can only identify a C struct")
        };
        *stored_fields = fields;
    }

    pub fn c_ref(&self, id: StructDefId) -> Option<CStructRef> {
        self.definitions[id]
            .is_c_layout()
            .then_some(CStructRef::from_validated_definition(id))
    }

    pub fn iter(&self) -> impl Iterator<Item = (StructDefId, &StructDef)> {
        self.definitions.iter()
    }

    pub fn len(&self) -> usize {
        self.definitions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.definitions.is_empty()
    }
}

impl std::ops::Index<StructDefId> for StructDefs {
    type Output = StructDef;

    fn index(&self, index: StructDefId) -> &Self::Output {
        &self.definitions[index]
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructField {
    pub ty: LirType,
    pub layout: FieldLayout,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CStructField {
    pub ty: CType,
    pub layout: FieldLayout,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructRepresentation {
    Scoop {
        fields: Vec<StructField>,
    },
    C {
        contract: LirCLayoutContract,
        fields: Vec<CStructField>,
    },
    /// Nominal declaration shell for a compiler pointer intrinsic. Canonical
    /// values use `LirType::Ptr`; this branch prevents the declaration from
    /// being mistaken for an ordinary zero-field aggregate.
    Intrinsic(IntrinsicTypeRepresentation),
}

impl StructDef {
    pub const fn c_layout(&self) -> Option<LirCLayoutContract> {
        match &self.representation {
            StructRepresentation::Scoop { .. } => None,
            StructRepresentation::C { contract, .. } => Some(*contract),
            StructRepresentation::Intrinsic(_) => None,
        }
    }

    pub const fn is_c_layout(&self) -> bool {
        matches!(&self.representation, StructRepresentation::C { .. })
    }

    pub fn scoop_fields(&self) -> Option<&[StructField]> {
        match &self.representation {
            StructRepresentation::Scoop { fields } => Some(fields),
            StructRepresentation::C { .. } | StructRepresentation::Intrinsic(_) => None,
        }
    }

    pub fn c_fields(&self) -> Option<&[CStructField]> {
        match &self.representation {
            StructRepresentation::Scoop { .. } => None,
            StructRepresentation::C { fields, .. } => Some(fields),
            StructRepresentation::Intrinsic(_) => None,
        }
    }

    pub fn field_count(&self) -> usize {
        match &self.representation {
            StructRepresentation::Scoop { fields } => fields.len(),
            StructRepresentation::C { fields, .. } => fields.len(),
            StructRepresentation::Intrinsic(_) => 0,
        }
    }

    pub fn field_layout(&self, index: usize) -> Option<FieldLayout> {
        match &self.representation {
            StructRepresentation::Scoop { fields } => fields.get(index).map(|field| field.layout),
            StructRepresentation::C { fields, .. } => fields.get(index).map(|field| field.layout),
            StructRepresentation::Intrinsic(_) => None,
        }
    }

    pub fn field_storage_type(&self, index: usize) -> Option<LirType> {
        match &self.representation {
            StructRepresentation::Scoop { fields } => {
                fields.get(index).map(|field| field.ty.clone())
            }
            StructRepresentation::C { fields, .. } => {
                fields.get(index).map(|field| field.ty.storage_type())
            }
            StructRepresentation::Intrinsic(_) => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FieldLayout {
    pub offset: u64,
    /// Alignment that a load/store of this field may claim. Packed layouts
    /// cap this independently of the field type's natural alignment.
    pub access_align: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LirCLayoutValue {
    Natural,
    A1,
    A2,
    A4,
    A8,
    A16,
}

impl LirCLayoutValue {
    pub const fn bytes(self) -> Option<u64> {
        match self {
            Self::Natural => None,
            Self::A1 => Some(1),
            Self::A2 => Some(2),
            Self::A4 => Some(4),
            Self::A8 => Some(8),
            Self::A16 => Some(16),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LirCLayoutContract {
    pub aligned: LirCLayoutValue,
    pub packed: LirCLayoutValue,
}

/// Per-Cone LIR metadata (impl spec 2.4): type layouts.
#[derive(Debug)]
pub struct LirMeta {
    /// Complete target capabilities used to compute every physical layout in
    /// this metadata. Codegen must consume the matching full target profile.
    pub target_profile: LirTargetProfile,
    /// Non-optional identities selected from typed intrinsic declarations.
    pub well_known_layouts: WellKnownLayouts,
    pub well_known_type_descriptors: WellKnownTypeDescriptors,
    /// Every fully specialized intrinsic `Array<T>` / `MutableArray<T>`
    /// application. Array instructions carry an `ArrayTypeId`; codegen never
    /// reconstructs nominal array identity or GC metadata from value layouts.
    pub arrays: Arena<ArrayType>,
    pub layouts: Arena<Layout>,
    /// Locally emitted TypeDescriptors. Every semantic edge uses a typed ref;
    /// `symbol` is only a final link attribute.
    pub type_descriptors: Arena<TypeDescriptor>,
    /// Cross-Cone descriptors are declared but not initialized by this Cone.
    pub external_type_descriptors: Arena<ExternalTypeDescriptor>,
    /// Cross-Cone callables referenced from local dispatch tables.
    pub external_callables: Arena<ExternalCallable>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WellKnownLayouts {
    pub string: LayoutId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WellKnownTypeDescriptors {
    pub string: TypeDescriptorRef,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArrayKind {
    Immutable,
    Mutable,
}

/// Complete LIR metadata for one concrete intrinsic array application.
/// `element_size` / `element_align` are fixed by lir-lower rather than
/// recomputed from LLVM ABI queries in codegen.
#[derive(Debug)]
pub struct ArrayType {
    /// Persistent managed-object layout and array-element scan identities for
    /// this exact intrinsic array application. Variable-size array layout
    /// payload remains in this typed record instead of masquerading as a
    /// fixed-size [`Layout`].
    pub identity: LayoutIdentity,
    pub kind: ArrayKind,
    pub element: LirType,
    pub element_size: u64,
    pub element_align: u64,
    /// The descriptor owns the recursive repeated-element scan program.
    pub type_descriptor: TypeDescriptorRef,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TypeDescriptorRef {
    Local(TypeDescriptorId),
    External(ExternalTypeDescriptorId),
}

#[derive(Debug)]
pub struct ExternalTypeDescriptor {
    /// Final linker spelling; never used as semantic identity.
    pub symbol: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CallableRef {
    Local(LocalFunctionId),
    Runtime(RuntimeFunction),
    External(ExternalCallableId),
}

#[derive(Debug)]
pub struct ExternalCallable {
    /// Final linker spelling; the typed arena id is the semantic identity.
    pub symbol: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DispatchEntry {
    pub callable: CallableRef,
}

/// Everything codegen needs to emit one `ScoopTypeDescriptor`
/// global (see runtime/include/scoop_rt.h for the field order).
#[derive(Debug)]
pub struct TypeDescriptor {
    /// Human-readable type name used in metadata dumps.
    pub name: String,
    /// Global symbol, e.g. `scoop_td_Point`.
    pub symbol: String,
    /// Complete persistent-to-runtime identity selected by lir-lower.
    /// Codegen consumes the typed runtime id and foundation projection keeps
    /// the full exact-type relation; neither infers it from arena position or
    /// descriptor category.
    pub runtime_type: RuntimeTypeMappingRecord,
    pub size: u64,
    pub align: u64,
    pub scan: TypeDescriptorScan,
    /// Classes reference their base descriptor; root/reference-key entities
    /// have no parent. The absence is emitted as a metadata-provenance null.
    pub parent: Option<TypeDescriptorRef>,
    pub vtable: Vec<DispatchEntry>,
    pub itables: Vec<ItableRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeDescriptorScan {
    /// Recursive GC scan program for a fixed-size object payload.
    Fixed(RefScan),
    /// Recursive scan for one inline array element, repeated at `stride`.
    ArrayElement { stride: u64, scan: RefScan },
}

#[derive(Debug)]
pub struct ItableRecord {
    pub interface: TypeDescriptorRef,
    pub slots: Vec<DispatchEntry>,
}

#[derive(Debug)]
pub struct Layout {
    /// Persistent identity of this exact physical representation. The key
    /// binds the semantic exact type, selected target profile and closed
    /// representation role; consumers never reconstruct it from the display
    /// name, arena position or coincidentally equal size/alignment.
    pub identity: LayoutIdentity,
    pub name: String,
    pub size: u64,
    pub align: u64,
    pub fields: Vec<FieldLayout>,
    pub c_layout: Option<LirCLayoutContract>,
    pub interior_mutable: bool,
    pub kind: LayoutKind,
}

/// Complete persistent identity bundle for one physical layout and its
/// top-level scan program. Constructors close the representation/scan-role
/// matrix so a layout cannot carry a scan identity for another role.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct LayoutIdentity {
    layout: scoop_identity::CborIdentityRecord<
        scoop_identity::PersistentLayoutId,
        scoop_identity::LayoutKey,
    >,
    scan: scoop_identity::CborIdentityRecord<
        scoop_identity::PersistentScanId,
        scoop_identity::ScanKey,
    >,
}

impl LayoutIdentity {
    fn new(
        exact_type: scoop_identity::PersistentExactTypeId,
        target_profile: LirTargetProfile,
        representation: scoop_identity::RepresentationRole,
        scan_role: scoop_identity::ScanRole,
    ) -> Result<Self, scoop_wire::HashError> {
        let layout = scoop_identity::CborIdentityRecord::from_key(scoop_identity::LayoutKey::new(
            exact_type,
            target_profile.wire_id(),
            representation,
        ))?;
        let scan = scoop_identity::CborIdentityRecord::from_key(scoop_identity::ScanKey::new(
            layout.id(),
            scan_role,
        ))?;
        Ok(Self { layout, scan })
    }

    pub fn managed_value(
        exact_type: scoop_identity::PersistentExactTypeId,
        target_profile: LirTargetProfile,
    ) -> Result<Self, scoop_wire::HashError> {
        Self::new(
            exact_type,
            target_profile,
            scoop_identity::RepresentationRole::ManagedValue,
            scoop_identity::ScanRole::InlineValue,
        )
    }

    pub fn managed_object(
        exact_type: scoop_identity::PersistentExactTypeId,
        target_profile: LirTargetProfile,
    ) -> Result<Self, scoop_wire::HashError> {
        Self::new(
            exact_type,
            target_profile,
            scoop_identity::RepresentationRole::ManagedObject,
            scoop_identity::ScanRole::ManagedObject,
        )
    }

    pub fn c_value(
        exact_type: scoop_identity::PersistentExactTypeId,
        target_profile: LirTargetProfile,
    ) -> Result<Self, scoop_wire::HashError> {
        Self::new(
            exact_type,
            target_profile,
            scoop_identity::RepresentationRole::CValue,
            scoop_identity::ScanRole::InlineValue,
        )
    }

    pub fn native_function_pointer(
        exact_type: scoop_identity::PersistentExactTypeId,
        target_profile: LirTargetProfile,
    ) -> Result<Self, scoop_wire::HashError> {
        Self::new(
            exact_type,
            target_profile,
            scoop_identity::RepresentationRole::NativeFunctionPointer,
            scoop_identity::ScanRole::InlineValue,
        )
    }

    pub fn managed_array(
        exact_type: scoop_identity::PersistentExactTypeId,
        target_profile: LirTargetProfile,
    ) -> Result<Self, scoop_wire::HashError> {
        Self::new(
            exact_type,
            target_profile,
            scoop_identity::RepresentationRole::ManagedObject,
            scoop_identity::ScanRole::ArrayElement,
        )
    }

    pub const fn layout_record(
        &self,
    ) -> &scoop_identity::CborIdentityRecord<
        scoop_identity::PersistentLayoutId,
        scoop_identity::LayoutKey,
    > {
        &self.layout
    }

    pub const fn scan_record(
        &self,
    ) -> &scoop_identity::CborIdentityRecord<
        scoop_identity::PersistentScanId,
        scoop_identity::ScanKey,
    > {
        &self.scan
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum LayoutKind {
    Plain {
        scan: RefScan,
    },
    /// Enum layouts retain their identity while exposing one fixed scan
    /// program for the complete physical value. Tagged-enum scans never
    /// branch on the tag: inactive ref-bearing slots are zero-filled.
    Enum {
        scan: RefScan,
    },
    /// Compiler representation selected by the typed intrinsic application
    /// in MIR. Generic family variants carry the fully lowered element type.
    Intrinsic(IntrinsicTypeRepresentation),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntrinsicTypeRepresentation {
    Integer(IntegerKind),
    Boolean,
    String,
    Ptr { pointee: LirDataPointee },
    FunPtr { signature: LirFunctionType },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LirDataPointee {
    OpaqueVoid,
    Value(Box<LirType>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LirFunctionType {
    pub params: Vec<LirType>,
    pub return_type: LirReturnType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LirReturnType {
    Void,
    Value(Box<LirType>),
}

impl LirReturnType {
    pub fn storage_type(&self) -> LirType {
        match self {
            Self::Void => LirType::Void,
            Self::Value(ty) => ty.as_ref().clone(),
        }
    }

    pub const fn is_void(&self) -> bool {
        matches!(self, Self::Void)
    }
}

impl LirFunctionType {
    pub fn storage_return_type(&self) -> LirType {
        self.return_type.storage_type()
    }
}

/// Recursive, layout-complete description of references in an inline
/// value. Every offset is relative to the base supplied by the caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefScan {
    None,
    References(Vec<u64>),
    Sequence(Vec<RefScan>),
}

impl RefScan {
    pub fn dump(&self) -> String {
        match self {
            Self::None => "none".to_string(),
            Self::References(offsets) => format!("refs{offsets:?}"),
            Self::Sequence(parts) => format!(
                "seq({})",
                parts.iter().map(Self::dump).collect::<Vec<_>>().join(", ")
            ),
        }
    }

    pub fn contains_reference(&self) -> bool {
        match self {
            Self::None => false,
            Self::References(offsets) => !offsets.is_empty(),
            Self::Sequence(parts) => parts.iter().any(Self::contains_reference),
        }
    }
}

/// A recursive scan program that is guaranteed to visit at least one managed
/// reference. This is the only scan representation accepted by caller roots.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NonEmptyRefScan(RefScan);

impl NonEmptyRefScan {
    pub fn new(scan: RefScan) -> Option<Self> {
        scan.contains_reference().then_some(Self(scan))
    }

    pub fn as_ref_scan(&self) -> &RefScan {
        &self.0
    }

    pub fn dump(&self) -> String {
        self.0.dump()
    }
}

#[derive(Debug)]
pub struct Global {
    pub symbol: String,
    /// Provenance of the address produced by `Value::Global`.
    pub address_kind: PointerKind,
    /// Complete recursive scan program for the writable global storage.
    /// Immortal object and C-string globals explicitly carry `None`.
    pub scan: RefScan,
    pub init: GlobalInit,
}

/// An enum definition with its representation fixed by lir-lower.
#[derive(Debug)]
pub struct EnumDef {
    pub name: String,
    pub repr: EnumRepr,
    /// Recursive scan program for one inline value of this enum type.
    pub scan: RefScan,
}

/// A variant identity refined against one [`EnumDefs`] definition store.
///
/// The fields are deliberately private: a raw variant index is not a valid
/// LIR identity until the owning enum definition has proved that it is in
/// range.  As with the other refined LIR references, consumers that receive a
/// complete module still revalidate the reference against that module's store
/// before using it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LirVariantRef {
    definition: EnumDefId,
    variant: u32,
}

impl LirVariantRef {
    pub const fn definition(self) -> EnumDefId {
        self.definition
    }

    pub const fn index(self) -> u32 {
        self.variant
    }
}

/// A payload field identity refined together with its owning variant.
///
/// Keeping the two indices in one closed value prevents an instruction from
/// pairing a checked variant with an unrelated or unchecked field number.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LirVariantFieldRef {
    variant: LirVariantRef,
    field: u32,
}

impl LirVariantFieldRef {
    pub const fn variant(self) -> LirVariantRef {
        self.variant
    }

    pub const fn definition(self) -> EnumDefId {
        self.variant.definition()
    }

    pub const fn index(self) -> u32 {
        self.field
    }
}

/// Enum definitions are immutable after allocation except for their recursive
/// scan program.  A C-nullable option's exact payload contract is bound once,
/// on the first refined-reference request, and every later request must agree.
/// Consequently a provenance-and-type-refined nullable reference minted by
/// this store cannot be invalidated or rebound later.
#[derive(Debug, Default)]
pub struct EnumDefs {
    definitions: Arena<EnumDef>,
    c_nullable_options: Vec<Option<CNullableOptionKind>>,
}

#[derive(Debug)]
enum CNullableOptionKind {
    DataPointer(std::sync::OnceLock<CDataPointee>),
    CodePointer(std::sync::OnceLock<CFunctionType>),
}

impl EnumDefs {
    pub fn alloc(&mut self, definition: EnumDef) -> EnumDefId {
        self.alloc_with_c_nullable_kind(definition, None)
    }

    pub fn alloc_c_nullable_data_pointer_option(&mut self, definition: EnumDef) -> EnumDefId {
        assert!(
            matches!(
                definition.repr,
                EnumRepr::Niche {
                    kind: NichePointerKind::Raw,
                    ..
                }
            ),
            "a nullable C data-pointer Option has raw-pointer niche storage",
        );
        self.alloc_with_c_nullable_kind(
            definition,
            Some(CNullableOptionKind::DataPointer(std::sync::OnceLock::new())),
        )
    }

    pub fn alloc_c_nullable_code_pointer_option(&mut self, definition: EnumDef) -> EnumDefId {
        assert!(
            matches!(
                definition.repr,
                EnumRepr::Niche {
                    kind: NichePointerKind::Code,
                    ..
                }
            ),
            "a nullable C code-pointer Option has code-pointer niche storage",
        );
        self.alloc_with_c_nullable_kind(
            definition,
            Some(CNullableOptionKind::CodePointer(std::sync::OnceLock::new())),
        )
    }

    fn alloc_with_c_nullable_kind(
        &mut self,
        definition: EnumDef,
        kind: Option<CNullableOptionKind>,
    ) -> EnumDefId {
        let id = self.definitions.alloc(definition);
        assert_eq!(
            id.into_raw().into_u32() as usize,
            self.c_nullable_options.len(),
            "enum definition and refinement stores remain index-aligned",
        );
        self.c_nullable_options.push(kind);
        id
    }

    pub fn set_scan(&mut self, id: EnumDefId, scan: RefScan) {
        self.definitions[id].scan = scan;
    }

    /// Refine a raw enum/id pair into a variant identity owned by this store.
    pub fn variant_ref(&self, definition: EnumDefId, variant: u32) -> Option<LirVariantRef> {
        let definition_index = definition.into_raw().into_u32() as usize;
        if definition_index >= self.definitions.len() {
            return None;
        }
        let variant_count = match &self.definitions[definition].repr {
            // A niche representation is structurally restricted to one unit
            // and one payload variant.
            EnumRepr::Niche { .. } => 2,
            EnumRepr::Tagged { variants, .. } => variants.len(),
        };
        ((variant as usize) < variant_count).then_some(LirVariantRef {
            definition,
            variant,
        })
    }

    /// Revalidate a refined reference received as part of a complete module.
    /// This rejects a reference minted by a different definition store whose
    /// raw arena index does not describe the same valid variant here.
    pub fn contains_variant(&self, variant: LirVariantRef) -> bool {
        self.variant_ref(variant.definition, variant.variant) == Some(variant)
    }

    /// Refine a checked variant plus raw field index into one closed payload
    /// field identity.
    pub fn variant_field_ref(
        &self,
        variant: LirVariantRef,
        field: u32,
    ) -> Option<LirVariantFieldRef> {
        self.variant_field_type_unchecked_index(variant, field)
            .map(|_| LirVariantFieldRef { variant, field })
    }

    /// Revalidate a refined field reference received in a complete module.
    pub fn contains_variant_field(&self, field: LirVariantFieldRef) -> bool {
        self.variant_field_ref(field.variant, field.field) == Some(field)
    }

    /// Exact payload field type for one checked field identity.
    ///
    /// Niche payloads are represented by their provenance-preserving carrier;
    /// tagged payloads use the layout-complete field record.  Returning the
    /// exact LIR type from this single authority prevents callers from
    /// reconstructing pointer provenance or a field type independently.
    pub fn variant_field_type(&self, field: LirVariantFieldRef) -> Option<LirType> {
        self.variant_field_type_unchecked_index(field.variant, field.field)
    }

    fn variant_field_type_unchecked_index(
        &self,
        variant: LirVariantRef,
        field: u32,
    ) -> Option<LirType> {
        if !self.contains_variant(variant) {
            return None;
        }
        match &self.definitions[variant.definition].repr {
            EnumRepr::Niche {
                kind,
                payload_variant,
            } if variant.variant == *payload_variant && field == 0 => {
                Some(LirType::Ptr(kind.pointer_kind()))
            }
            EnumRepr::Niche { .. } => None,
            EnumRepr::Tagged { variants, .. } => variants[variant.variant as usize]
                .fields
                .get(field as usize)
                .map(|field| field.ty.clone()),
        }
    }

    pub fn nullable_data_pointer_ref(
        &self,
        id: EnumDefId,
        pointee: CDataPointee,
    ) -> Option<NullableDataPointerEnumRef> {
        let Some(CNullableOptionKind::DataPointer(binding)) = self
            .c_nullable_options
            .get(id.into_raw().into_u32() as usize)
            .and_then(Option::as_ref)
        else {
            return None;
        };
        (binding.get_or_init(|| pointee.clone()) == &pointee).then_some(
            NullableDataPointerEnumRef::from_validated_definition(id, pointee),
        )
    }

    pub fn nullable_code_pointer_ref(
        &self,
        id: EnumDefId,
        signature: CFunctionType,
    ) -> Option<NullableCodePointerEnumRef> {
        let Some(CNullableOptionKind::CodePointer(binding)) = self
            .c_nullable_options
            .get(id.into_raw().into_u32() as usize)
            .and_then(Option::as_ref)
        else {
            return None;
        };
        (binding.get_or_init(|| signature.clone()) == &signature).then_some(
            NullableCodePointerEnumRef::from_validated_definition(id, signature),
        )
    }

    pub fn nullable_data_pointer_binding(&self, id: EnumDefId) -> Option<&CDataPointee> {
        let CNullableOptionKind::DataPointer(binding) = self
            .c_nullable_options
            .get(id.into_raw().into_u32() as usize)?
            .as_ref()?
        else {
            return None;
        };
        binding.get()
    }

    pub fn nullable_code_pointer_binding(&self, id: EnumDefId) -> Option<&CFunctionType> {
        let CNullableOptionKind::CodePointer(binding) = self
            .c_nullable_options
            .get(id.into_raw().into_u32() as usize)?
            .as_ref()?
        else {
            return None;
        };
        binding.get()
    }

    pub fn iter(&self) -> impl Iterator<Item = (EnumDefId, &EnumDef)> {
        self.definitions.iter()
    }

    /// Returns the definition for a possibly untrusted arena identity.
    ///
    /// Complete LIR normally carries ids minted by this store, but boundary
    /// validation must be able to reject a malformed module without indexing
    /// the arena first.
    pub fn get(&self, id: EnumDefId) -> Option<&EnumDef> {
        let index = id.into_raw().into_u32() as usize;
        (index < self.definitions.len()).then(|| &self.definitions[id])
    }

    pub fn len(&self) -> usize {
        self.definitions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.definitions.is_empty()
    }
}

impl std::ops::Index<EnumDefId> for EnumDefs {
    type Output = EnumDef;

    fn index(&self, index: EnumDefId) -> &Self::Output {
        &self.definitions[index]
    }
}

#[derive(Debug)]
pub enum EnumRepr {
    /// Niche optimization (spec 7.4): only an enum structurally isomorphic to
    /// `Option<ref/Ptr/FunPtr>` may use it — exactly one empty variant and one
    /// single pointer-represented payload variant. Names and order do not
    /// matter. The exact pointer provenance is carried atomically with the
    /// representation and cannot be reconstructed from a scan program.
    Niche {
        /// Exact pointer provenance of both the payload and its typed null.
        kind: NichePointerKind,
        /// Index of the payload-carrying variant.
        payload_variant: u32,
    },
    /// A tag followed by one optional shared pure-value payload slot and
    /// one disjoint slot for every variant that contains managed refs.
    Tagged {
        variants: Vec<EnumVariantRepr>,
        size: u64,
        align: u64,
    },
}

/// Physical storage assigned to one tagged-enum variant.
#[derive(Debug)]
pub struct EnumVariantRepr {
    pub fields: Vec<EnumFieldRepr>,
    pub slot_offset: u64,
    pub slot_size: u64,
    pub slot_align: u64,
    /// Copied from the fully specialized MIR variant. Only GC-free
    /// variants may share the pure-value payload slot.
    pub gc_free: bool,
}

/// The complete physical representation of one tagged-enum field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumFieldRepr {
    pub ty: LirType,
    /// Enum-relative offset, including the variant's slot offset.
    pub offset: u64,
}

#[derive(Debug)]
pub enum GlobalInit {
    /// A `ScoopString` constant: header points at `STRING_TD_SYMBOL`.
    StringConst(String),
    /// A NUL-terminated C string (e.g. trap messages).
    CString(String),
    Storage {
        ty: LirType,
        initial_state: LirStaticInitialState,
        thread_local: bool,
    },
}

#[derive(Debug)]
pub enum LirStaticInitialState {
    ZeroedForRuntimeUnit,
    EncodedStaticValue { payload: LirConstantImage },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LirConstantImage {
    Integer(LirIntegerConstant),
    Bool(bool),
    NullPointer(PointerKind),
    GlobalPointer {
        global: GlobalId,
        kind: PointerKind,
    },
    EnumUnit {
        variant: LirVariantRef,
    },
    Struct {
        struct_id: StructDefId,
        fields: Vec<LirConstantImage>,
    },
}
