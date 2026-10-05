use std::num::NonZeroU64;

use super::*;

mod arrays;
mod static_storage;
pub use arrays::ArrayLayoutV1;
pub use static_storage::StaticStorageLayout;

#[derive(Debug)]
pub struct StructDef {
    /// Exact semantic identity, independent of this store's local index.
    pub exact_type: scoop_identity::PersistentExactTypeId,
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
        exact_type: scoop_identity::PersistentExactTypeId,
        name: String,
        size: u64,
        align: u64,
        interior_mutable: bool,
        fields: Vec<StructField>,
    ) -> StructDefId {
        self.definitions.alloc(StructDef {
            exact_type,
            name,
            size,
            align,
            interior_mutable,
            representation: StructRepresentation::Scoop { fields },
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn alloc_c(
        &mut self,
        exact_type: scoop_identity::PersistentExactTypeId,
        name: String,
        size: u64,
        align: u64,
        interior_mutable: bool,
        contract: LirCLayoutContract,
        fields: Vec<CStructField>,
    ) -> CStructRef {
        let id = self.definitions.alloc(StructDef {
            exact_type,
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
        exact_type: scoop_identity::PersistentExactTypeId,
        name: String,
        size: u64,
        align: u64,
        representation: IntrinsicTypeRepresentation,
    ) -> StructDefId {
        self.definitions.alloc(StructDef {
            exact_type,
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
    pub identity: scoop_identity::PersistentFieldId,
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
    /// Complete persistent exact-type identities for every locally
    /// materialized source or generated nominal. Physical metadata keeps the
    /// semantic key instead of exposing only its derived id.
    pub exact_types: Vec<
        scoop_identity::CborIdentityRecord<
            scoop_identity::PersistentExactTypeId,
            scoop_identity::ExactTypeKey,
        >,
    >,
    /// Complete target capabilities used to compute every physical layout in
    /// this metadata. Codegen must consume the matching full target profile.
    pub target_profile: LirTargetProfile,
    /// Complete canonical C source-storage leaf records used by native
    /// functions, globals, and callbacks in this module.
    pub canonical_c_abi: CanonicalCAbiMetadata,
    /// Complete target-normalized contracts for source extern declarations.
    pub native_externals: NativeExternalMetadata,
    /// Non-optional identities selected from typed intrinsic declarations.
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
    /// External callables admitted by initialization protocols, the legacy
    /// direct-call bridge, or the layout/ABI bridge used by dispatch tables.
    /// They are external declarations and never enter local Strong ownership,
    /// callable registrations, or image plans.
    pub external_callables: Arena<ExternalCallable>,
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
/// Element storage and offsets are fixed by lir-lower and checked together.
#[derive(Debug)]
pub struct ArrayType {
    /// Persistent managed-object layout and array-element scan identities for
    /// this exact intrinsic array application. Variable-size array layout
    /// payload remains in this typed record instead of masquerading as a
    /// fixed-size [`Layout`].
    pub identity: LayoutIdentity,
    pub kind: ArrayKind,
    pub element_exact: scoop_identity::PersistentExactTypeId,
    pub element: LirType,
    pub layout: ArrayLayoutV1,
    /// The descriptor owns the recursive repeated-element scan program.
    pub type_descriptor: TypeDescriptorRef,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TypeDescriptorRef {
    Local(TypeDescriptorId),
    External(ExternalTypeDescriptorId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CallableRef {
    Local(LocalFunctionId),
    Runtime(RuntimeFunction),
    External(ExternalCallableId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DispatchEntry {
    pub callable: CallableRef,
}

/// Everything codegen needs to emit one `ScoopTypeDescriptor`
/// global (see runtime/include/scoop_runtime_metadata_v1.h for the field order).
#[derive(Debug)]
pub struct TypeDescriptor {
    pub release_policy: ReleasePolicy,
    /// Canonical exact-type UTF-8 used only for diagnostics.
    pub diagnostic_name: String,
    /// Complete persistent-to-runtime identity selected by lir-lower.
    /// Codegen consumes the typed runtime id and foundation projection keeps
    /// the full exact-type relation; neither infers it from arena position or
    /// descriptor category.
    pub identity: TypeDescriptorIdentity,
    /// Persistent identity of the managed-instance layout promised by this
    /// descriptor. This remains explicit even for `AbstractRef`, whose shape
    /// has no allocatable byte extent, so registration production never has
    /// to recover the relation from an unrelated layout arena.
    pub instance_layout: LayoutIdentity,
    pub instance_shape: TypeInstanceShapeV1,
    /// Exact strong scan definition referenced by the inline-scan field, or
    /// an explicit null branch when the inline scan is empty.
    pub inline_scan: TypeDescriptorInlineScanV1,
    /// Classes reference their base descriptor; root/reference-key entities
    /// have no parent. The absence is emitted as a metadata-provenance null.
    pub parent: Option<TypeDescriptorRef>,
    /// The exact-type-owned virtual dispatch table. Even a type with no
    /// virtual slots has a typed empty table rather than an identity-less
    /// vector.
    pub vtable: VtableRecord,
    pub itables: Vec<ItableRecord>,
    pub relations: crate::TypeDescriptorRelations<Option<TypeDescriptorRef>>,
}

/// Persistent runtime and materialization identity of one TypeDescriptor.
/// Construction binds the ODR member, when present, to the same exact type
/// that derives the runtime type id.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeDescriptorIdentity {
    runtime_type: RuntimeTypeMappingRecord,
    materialization: MaterializationIdentity,
    symbol: MaterializedSymbol,
}

impl TypeDescriptorIdentity {
    pub fn new(
        runtime_type: RuntimeTypeMappingRecord,
        root: MaterializationRoot,
    ) -> Result<Self, scoop_wire::HashError> {
        let materialization = root.type_descriptor(runtime_type.exact_type())?;
        let symbol = materialization
            .symbol(scoop_identity::PersistentSymbolKey::TypeDescriptor(
                runtime_type.exact_type(),
            ))
            .expect("type-descriptor symbols admit their materialization linkage");
        Ok(Self {
            runtime_type,
            materialization,
            symbol,
        })
    }

    pub const fn runtime_type(&self) -> RuntimeTypeMappingRecord {
        self.runtime_type
    }

    pub const fn exact_type(&self) -> scoop_identity::PersistentExactTypeId {
        self.runtime_type.exact_type()
    }

    pub const fn symbol_request(&self) -> scoop_identity::PersistentSymbolRequest {
        self.symbol.request()
    }

    pub fn symbol(&self) -> &str {
        self.symbol.as_str()
    }

    pub const fn lir_odr_group_record(
        &self,
    ) -> Option<
        &scoop_identity::CborIdentityRecord<
            scoop_identity::OdrGroupId,
            scoop_identity::SpecializationKey,
        >,
    > {
        self.materialization.lir_odr_group_record()
    }

    pub const fn odr_member_record(
        &self,
    ) -> Option<
        &scoop_identity::CborIdentityRecord<
            scoop_identity::OdrMemberId,
            scoop_identity::OdrMemberKey,
        >,
    > {
        self.materialization.odr_member_record()
    }

    fn dispatch_table(
        &self,
        table: scoop_identity::PersistentDispatchTableId,
    ) -> Result<MaterializationIdentity, scoop_wire::HashError> {
        self.materialization.dispatch_table(table)
    }
}

/// One exact type's virtual dispatch table and its persistent identity.
///
/// The fields are private so callers cannot attach an itable identity to a
/// vtable payload or replace the exact-type key independently of its slots.
#[derive(Debug)]
pub struct VtableRecord {
    identity: scoop_identity::CborIdentityRecord<
        scoop_identity::PersistentDispatchTableId,
        scoop_identity::DispatchTableKey,
    >,
    materialization: MaterializationIdentity,
    slots: Vec<DispatchEntry>,
}

impl VtableRecord {
    pub fn new(
        owner: &TypeDescriptorIdentity,
        slots: Vec<DispatchEntry>,
    ) -> Result<Self, scoop_wire::HashError> {
        let identity = scoop_identity::CborIdentityRecord::from_key(
            scoop_identity::DispatchTableKey::vtable(owner.exact_type()),
        )?;
        let materialization = owner.dispatch_table(identity.id())?;
        Ok(Self {
            identity,
            materialization,
            slots,
        })
    }

    pub const fn identity_record(
        &self,
    ) -> &scoop_identity::CborIdentityRecord<
        scoop_identity::PersistentDispatchTableId,
        scoop_identity::DispatchTableKey,
    > {
        &self.identity
    }

    pub fn belongs_to_exact_type(&self, exact_type: scoop_identity::PersistentExactTypeId) -> bool {
        self.identity.key() == &scoop_identity::DispatchTableKey::vtable(exact_type)
    }

    pub fn slots(&self) -> &[DispatchEntry] {
        &self.slots
    }

    pub fn slots_mut(&mut self) -> &mut Vec<DispatchEntry> {
        &mut self.slots
    }

    pub const fn lir_odr_group_record(
        &self,
    ) -> Option<
        &scoop_identity::CborIdentityRecord<
            scoop_identity::OdrGroupId,
            scoop_identity::SpecializationKey,
        >,
    > {
        self.materialization.lir_odr_group_record()
    }

    pub const fn odr_member_record(
        &self,
    ) -> Option<
        &scoop_identity::CborIdentityRecord<
            scoop_identity::OdrMemberId,
            scoop_identity::OdrMemberKey,
        >,
    > {
        self.materialization.odr_member_record()
    }
}

/// One exact type's implementation table for one exact interface.
///
/// Construction binds the table role, owner and interface into one immutable
/// identity/payload relation. The descriptor reference is retained solely for
/// code generation of the runtime lookup key.
#[derive(Debug)]
pub struct ItableRecord {
    identity: scoop_identity::CborIdentityRecord<
        scoop_identity::PersistentDispatchTableId,
        scoop_identity::DispatchTableKey,
    >,
    materialization: MaterializationIdentity,
    interface: TypeDescriptorRef,
    slots: Vec<DispatchEntry>,
}

impl ItableRecord {
    pub fn new(
        owner: &TypeDescriptorIdentity,
        interface_exact_type: scoop_identity::PersistentExactTypeId,
        interface: TypeDescriptorRef,
        slots: Vec<DispatchEntry>,
    ) -> Result<Self, scoop_wire::HashError> {
        let identity = scoop_identity::CborIdentityRecord::from_key(
            scoop_identity::DispatchTableKey::itable(owner.exact_type(), interface_exact_type),
        )?;
        let materialization = owner.dispatch_table(identity.id())?;
        Ok(Self {
            identity,
            materialization,
            interface,
            slots,
        })
    }

    pub const fn identity_record(
        &self,
    ) -> &scoop_identity::CborIdentityRecord<
        scoop_identity::PersistentDispatchTableId,
        scoop_identity::DispatchTableKey,
    > {
        &self.identity
    }

    pub fn belongs_to_exact_type(&self, exact_type: scoop_identity::PersistentExactTypeId) -> bool {
        self.identity.key().exact_type() == exact_type
    }

    pub fn belongs_to_interface_exact_type(
        &self,
        exact_type: scoop_identity::PersistentExactTypeId,
    ) -> bool {
        matches!(
            self.identity.key().interface(),
            scoop_identity::OptionalExactInterface::Present(interface) if interface == exact_type
        )
    }

    pub const fn interface(&self) -> TypeDescriptorRef {
        self.interface
    }

    pub fn slots(&self) -> &[DispatchEntry] {
        &self.slots
    }

    pub const fn lir_odr_group_record(
        &self,
    ) -> Option<
        &scoop_identity::CborIdentityRecord<
            scoop_identity::OdrGroupId,
            scoop_identity::SpecializationKey,
        >,
    > {
        self.materialization.lir_odr_group_record()
    }

    pub const fn odr_member_record(
        &self,
    ) -> Option<
        &scoop_identity::CborIdentityRecord<
            scoop_identity::OdrMemberId,
            scoop_identity::OdrMemberKey,
        >,
    > {
        self.materialization.odr_member_record()
    }
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
    layout_materialization: MaterializationIdentity,
    scan_materialization: MaterializationIdentity,
}

impl LayoutIdentity {
    pub(crate) fn new(
        exact_type: scoop_identity::PersistentExactTypeId,
        target_profile: LirTargetProfile,
        representation: scoop_identity::RepresentationRole,
        scan_role: scoop_identity::ScanRole,
        root: MaterializationRoot,
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
        let layout_materialization = root.layout(layout.id())?;
        let scan_materialization = root.scan(scan.id())?;
        Ok(Self {
            layout,
            scan,
            layout_materialization,
            scan_materialization,
        })
    }

    pub fn managed_value(
        exact_type: scoop_identity::PersistentExactTypeId,
        target_profile: LirTargetProfile,
        root: MaterializationRoot,
    ) -> Result<Self, scoop_wire::HashError> {
        Self::new(
            exact_type,
            target_profile,
            scoop_identity::RepresentationRole::ManagedValue,
            scoop_identity::ScanRole::InlineValue,
            root,
        )
    }

    pub fn managed_object(
        exact_type: scoop_identity::PersistentExactTypeId,
        target_profile: LirTargetProfile,
        root: MaterializationRoot,
    ) -> Result<Self, scoop_wire::HashError> {
        Self::new(
            exact_type,
            target_profile,
            scoop_identity::RepresentationRole::ManagedObject,
            scoop_identity::ScanRole::ManagedObject,
            root,
        )
    }

    pub fn c_value(
        exact_type: scoop_identity::PersistentExactTypeId,
        target_profile: LirTargetProfile,
        root: MaterializationRoot,
    ) -> Result<Self, scoop_wire::HashError> {
        Self::new(
            exact_type,
            target_profile,
            scoop_identity::RepresentationRole::CValue,
            scoop_identity::ScanRole::InlineValue,
            root,
        )
    }

    pub fn native_function_pointer(
        exact_type: scoop_identity::PersistentExactTypeId,
        target_profile: LirTargetProfile,
        root: MaterializationRoot,
    ) -> Result<Self, scoop_wire::HashError> {
        Self::new(
            exact_type,
            target_profile,
            scoop_identity::RepresentationRole::NativeFunctionPointer,
            scoop_identity::ScanRole::InlineValue,
            root,
        )
    }

    pub fn managed_array(
        exact_type: scoop_identity::PersistentExactTypeId,
        target_profile: LirTargetProfile,
        root: MaterializationRoot,
    ) -> Result<Self, scoop_wire::HashError> {
        Self::new(
            exact_type,
            target_profile,
            scoop_identity::RepresentationRole::ManagedObject,
            scoop_identity::ScanRole::ArrayElement,
            root,
        )
    }

    pub fn is_managed_instance_of(
        &self,
        exact_type: scoop_identity::PersistentExactTypeId,
        target_profile: LirTargetProfile,
    ) -> bool {
        self.layout.key().exact_type() == exact_type
            && self.layout.key().target_profile() == &target_profile.wire_id()
            && self.layout.key().representation()
                == scoop_identity::RepresentationRole::ManagedObject
            && self.scan.key().layout() == self.layout.id()
            && self.scan.key().role() == scoop_identity::ScanRole::ManagedObject
    }

    pub fn is_managed_value_of(
        &self,
        exact_type: scoop_identity::PersistentExactTypeId,
        target_profile: LirTargetProfile,
    ) -> bool {
        self.layout.key().exact_type() == exact_type
            && self.layout.key().target_profile() == &target_profile.wire_id()
            && self.layout.key().representation()
                == scoop_identity::RepresentationRole::ManagedValue
            && self.scan.key().layout() == self.layout.id()
            && self.scan.key().role() == scoop_identity::ScanRole::InlineValue
    }

    pub fn is_managed_array_of(
        &self,
        exact_type: scoop_identity::PersistentExactTypeId,
        target_profile: LirTargetProfile,
    ) -> bool {
        self.layout.key().exact_type() == exact_type
            && self.layout.key().target_profile() == &target_profile.wire_id()
            && self.layout.key().representation()
                == scoop_identity::RepresentationRole::ManagedObject
            && self.scan.key().layout() == self.layout.id()
            && self.scan.key().role() == scoop_identity::ScanRole::ArrayElement
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

    pub const fn lir_odr_group_record(
        &self,
    ) -> Option<
        &scoop_identity::CborIdentityRecord<
            scoop_identity::OdrGroupId,
            scoop_identity::SpecializationKey,
        >,
    > {
        self.layout_materialization.lir_odr_group_record()
    }

    pub fn odr_member_records(
        &self,
    ) -> impl Iterator<
        Item = &scoop_identity::CborIdentityRecord<
            scoop_identity::OdrMemberId,
            scoop_identity::OdrMemberKey,
        >,
    > {
        [&self.layout_materialization, &self.scan_materialization]
            .into_iter()
            .filter_map(MaterializationIdentity::odr_member_record)
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
    Unit,
    Integer(IntegerKind),
    Boolean,
    Char,
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
    Array {
        length_offset: u64,
        first_element_offset: u64,
        stride: NonZeroU64,
        element: Box<NonEmptyRefScan>,
    },
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
            Self::Array {
                length_offset,
                first_element_offset,
                stride,
                element,
            } => format!(
                "array(length@{length_offset}, first@{first_element_offset}, stride={}, {})",
                stride.get(),
                element.dump()
            ),
        }
    }

    pub fn contains_reference(&self) -> bool {
        match self {
            Self::None => false,
            Self::References(offsets) => !offsets.is_empty(),
            Self::Sequence(parts) => parts.iter().any(Self::contains_reference),
            Self::Array { .. } => true,
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
    /// Provenance of the address produced by `Value::Global`.
    pub address_kind: PointerKind,
    /// Complete recursive scan program for the writable global storage.
    /// Immortal object and C-string globals explicitly carry `None`.
    pub scan: RefScan,
    pub init: GlobalInit,
}

/// Persistent owner and atom identity for one callable-local NUL-terminated
/// diagnostic string.
///
/// The physical global uses the atom's start-boundary symbol directly. This
/// keeps the support bytes attributable after object sharding and prevents a
/// session-local global ordinal from becoming linker-visible authority.
#[derive(Debug)]
pub struct CallableCStringIdentity {
    owner: scoop_identity::PersistentCallableBodyId,
    path: scoop_identity::StructuralDefinitionPath,
    atom: scoop_identity::CborIdentityRecord<
        scoop_identity::ObjectDefinitionAtomId,
        scoop_identity::ObjectDefinitionAtomKey,
    >,
    symbol: MaterializedSymbol,
}

impl CallableCStringIdentity {
    pub fn new(
        producer: scoop_identity::ConeIdentity,
        owner: &crate::CallableBodyIdentity,
        path: scoop_identity::StructuralDefinitionPath,
    ) -> Result<Self, scoop_wire::HashError> {
        let plan_key = owner.definition_plan_key(producer);
        let plan = scoop_identity::ObjectDefinitionPlanId::from_key(&plan_key)?;
        let atom = scoop_identity::CborIdentityRecord::from_key(
            scoop_identity::ObjectDefinitionAtomKey::new(
                plan,
                scoop_identity::DefinitionAtomRole::AddressTakenConstant,
                scoop_identity::DefinitionAtomSubkey::StructuralPath(path.clone()),
            ),
        )?;
        let symbol = MaterializedSymbol::new(
            scoop_identity::PersistentSymbolKey::DefinitionBoundaryStart(atom.id()),
            owner.symbol_request().linkage(),
        )
        .expect("a definition boundary inherits its body linkage");
        Ok(Self {
            owner: owner.id(),
            path,
            atom,
            symbol,
        })
    }

    pub const fn owner(&self) -> scoop_identity::PersistentCallableBodyId {
        self.owner
    }

    pub const fn path(&self) -> &scoop_identity::StructuralDefinitionPath {
        &self.path
    }

    pub const fn atom_record(
        &self,
    ) -> &scoop_identity::CborIdentityRecord<
        scoop_identity::ObjectDefinitionAtomId,
        scoop_identity::ObjectDefinitionAtomKey,
    > {
        &self.atom
    }

    pub const fn symbol_request(&self) -> PersistentSymbolRequest {
        self.symbol.request()
    }

    pub fn symbol(&self) -> &str {
        self.symbol.as_str()
    }
}

impl Global {
    pub fn symbol(&self) -> &str {
        match &self.init {
            GlobalInit::StringConst { identity, .. } => identity.symbol(),
            GlobalInit::CString { identity, .. } => identity.symbol(),
            GlobalInit::Storage { identity, .. } | GlobalInit::RawStorage { identity, .. } => {
                identity.symbol()
            }
            GlobalInit::ImportedStorage { definition, .. } => definition.symbol(),
        }
    }

    pub const fn persistent_symbol_request(
        &self,
    ) -> Option<scoop_identity::PersistentSymbolRequest> {
        match &self.init {
            GlobalInit::StringConst { identity, .. } => Some(identity.symbol_request()),
            GlobalInit::Storage { identity, .. } | GlobalInit::RawStorage { identity, .. } => {
                Some(identity.symbol_request())
            }
            GlobalInit::CString { .. } | GlobalInit::ImportedStorage { .. } => None,
        }
    }
}

/// An enum definition with its representation fixed by lir-lower.
#[derive(Debug)]
pub struct EnumDef {
    /// Exact semantic identity, including generated and generic applications.
    pub exact_type: scoop_identity::PersistentExactTypeId,
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
    /// Storage defined and registered by an actual dependency provider.
    ImportedStorage {
        definition: Box<crate::ExternalShapeLinkImportV1>,
        ty: LirType,
    },
    /// A `ScoopString` constant whose header points at the typed String
    /// descriptor selected by `WellKnownTypeDescriptors`.
    StringConst {
        identity: ImmortalObjectIdentity,
        value: String,
    },
    /// A callable-owned NUL-terminated C string (e.g. trap messages).
    CString {
        identity: CallableCStringIdentity,
        value: String,
    },
    /// Explicit GC-free raw global or TLS, initialized by the object loader.
    RawStorage {
        identity: StaticStorageIdentity,
        ty: LirType,
        initializer: LirConstantImage,
        thread_local: bool,
    },
    Storage {
        /// Persistent semantic identity of this compiler-owned writable
        /// storage. Native extern globals are represented separately and do
        /// not fabricate one.
        identity: StaticStorageIdentity,
        /// Canonical value layout and scan identities for this storage.
        /// Registration production consumes these identities directly and
        /// never reconstructs them from the lowered type or arena position.
        layout: StaticStorageLayout,
        ty: LirType,
        initial_state: LirStaticInitialState,
    },
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ImmortalObjectIdentity {
    record: scoop_identity::CborIdentityRecord<
        scoop_identity::PersistentImmortalObjectId,
        scoop_identity::ImmortalObjectKey,
    >,
    materialization: MaterializationIdentity,
    symbol: MaterializedSymbol,
}

impl ImmortalObjectIdentity {
    pub fn from_key(
        key: scoop_identity::ImmortalObjectKey,
        root: MaterializationRoot,
    ) -> Result<Self, scoop_wire::HashError> {
        let record = scoop_identity::CborIdentityRecord::from_key(key)?;
        let materialization = root.immortal_object(record.id())?;
        let symbol = materialization
            .symbol(scoop_identity::PersistentSymbolKey::ImmortalObject(
                record.id(),
            ))
            .expect("immortal-object symbols admit their materialization linkage");
        Ok(Self {
            record,
            materialization,
            symbol,
        })
    }

    pub const fn identity_record(
        &self,
    ) -> &scoop_identity::CborIdentityRecord<
        scoop_identity::PersistentImmortalObjectId,
        scoop_identity::ImmortalObjectKey,
    > {
        &self.record
    }

    pub const fn lir_odr_group_record(
        &self,
    ) -> Option<
        &scoop_identity::CborIdentityRecord<
            scoop_identity::OdrGroupId,
            scoop_identity::SpecializationKey,
        >,
    > {
        self.materialization.lir_odr_group_record()
    }

    pub const fn odr_member_record(
        &self,
    ) -> Option<
        &scoop_identity::CborIdentityRecord<
            scoop_identity::OdrMemberId,
            scoop_identity::OdrMemberKey,
        >,
    > {
        self.materialization.odr_member_record()
    }

    pub const fn symbol_request(&self) -> scoop_identity::PersistentSymbolRequest {
        self.symbol.request()
    }

    pub fn symbol(&self) -> &str {
        self.symbol.as_str()
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct StaticStorageIdentity {
    record: scoop_identity::CborIdentityRecord<
        scoop_identity::PersistentStaticStorageId,
        scoop_identity::StaticStorageKey,
    >,
    materialization: MaterializationIdentity,
    symbol: MaterializedSymbol,
}

impl StaticStorageIdentity {
    fn new(
        key: scoop_identity::StaticStorageKey,
        root: MaterializationRoot,
    ) -> Result<Self, scoop_wire::HashError> {
        let record = scoop_identity::CborIdentityRecord::from_key(key)?;
        let materialization = root.static_storage(record.id())?;
        let symbol = materialization
            .symbol(scoop_identity::PersistentSymbolKey::StaticStorage(
                record.id(),
            ))
            .expect("static-storage symbols admit their materialization linkage");
        Ok(Self {
            record,
            materialization,
            symbol,
        })
    }

    pub fn property_backing(
        owner: scoop_identity::PropertyOwner,
        root: MaterializationRoot,
    ) -> Result<Self, scoop_wire::HashError> {
        Self::new(
            scoop_identity::StaticStorageKey::property_backing(owner),
            root,
        )
    }

    pub fn property_delegate(
        owner: scoop_identity::PropertyOwner,
        root: MaterializationRoot,
    ) -> Result<Self, scoop_wire::HashError> {
        Self::new(
            scoop_identity::StaticStorageKey::property_delegate(owner),
            root,
        )
    }

    pub fn delegated_application(
        unit: &scoop_identity::InitializationUnitKey,
        zero_sized: bool,
        root: MaterializationRoot,
    ) -> Result<Self, scoop_identity::RuntimeIdentityError> {
        let key = if zero_sized {
            scoop_identity::StaticStorageKey::static_place_for_delegated_application(unit)?
        } else {
            scoop_identity::StaticStorageKey::delegated_application_delegate(unit)?
        };
        Self::new(key, root).map_err(scoop_identity::RuntimeIdentityError::Hash)
    }

    pub fn static_place_for_property(
        owner: scoop_identity::PropertyOwner,
        root: MaterializationRoot,
    ) -> Result<Self, scoop_wire::HashError> {
        Self::new(
            scoop_identity::StaticStorageKey::static_place_for_property(owner),
            root,
        )
    }

    pub fn singleton_application_root(
        owner: scoop_identity::PersistentExactTypeId,
        root: MaterializationRoot,
    ) -> Result<Self, scoop_wire::HashError> {
        Self::new(
            scoop_identity::StaticStorageKey::singleton_application_root(owner),
            root,
        )
    }

    pub fn singleton_published_root(
        owner: scoop_identity::PersistentTypeId,
        root: MaterializationRoot,
    ) -> Result<Self, scoop_wire::HashError> {
        Self::new(
            scoop_identity::StaticStorageKey::singleton_published_root(owner),
            root,
        )
    }

    pub fn initialization_failure_root(
        unit: scoop_identity::PersistentInitializationUnitId,
        root: MaterializationRoot,
    ) -> Result<Self, scoop_wire::HashError> {
        Self::new(
            scoop_identity::StaticStorageKey::initialization_failure_root(unit),
            root,
        )
    }

    pub fn root_entry_failure_root(
        root_cone: scoop_identity::ConeIdentity,
        main: scoop_identity::MainCallableBodyId,
        root: MaterializationRoot,
    ) -> Result<Self, scoop_wire::HashError> {
        Self::new(
            scoop_identity::StaticStorageKey::root_entry_failure_root(root_cone, main),
            root,
        )
    }

    pub const fn identity_record(
        &self,
    ) -> &scoop_identity::CborIdentityRecord<
        scoop_identity::PersistentStaticStorageId,
        scoop_identity::StaticStorageKey,
    > {
        &self.record
    }

    pub const fn lir_odr_group_record(
        &self,
    ) -> Option<
        &scoop_identity::CborIdentityRecord<
            scoop_identity::OdrGroupId,
            scoop_identity::SpecializationKey,
        >,
    > {
        self.materialization.lir_odr_group_record()
    }

    pub const fn odr_member_record(
        &self,
    ) -> Option<
        &scoop_identity::CborIdentityRecord<
            scoop_identity::OdrMemberId,
            scoop_identity::OdrMemberKey,
        >,
    > {
        self.materialization.odr_member_record()
    }

    pub const fn symbol_request(&self) -> scoop_identity::PersistentSymbolRequest {
        self.symbol.request()
    }

    pub fn symbol(&self) -> &str {
        self.symbol.as_str()
    }
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
