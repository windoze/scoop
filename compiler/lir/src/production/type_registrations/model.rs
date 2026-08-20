use super::*;

pub type StrongTypeRegistrationPlanV1 =
    StrongTypeRegistrationPlan<StrongTypeDescriptorRefV1, StrongTypeDispatchCallableRefV1>;
pub type StrongTypeRegistrationPlanV2 = StrongTypeRegistrationPlan<
    crate::StrongTypeDescriptorRefV2,
    crate::StrongTypeDispatchCallableRefV2,
>;
pub type StrongTypeRegistrationPlanSetV1 =
    StrongTypeRegistrationPlanSet<StrongTypeDescriptorRefV1, StrongTypeDispatchCallableRefV1>;
pub type StrongTypeRegistrationPlanSetV2 = StrongTypeRegistrationPlanSet<
    crate::StrongTypeDescriptorRefV2,
    crate::StrongTypeDispatchCallableRefV2,
>;

/// Exact strong definition, if any, referenced by the descriptor's runtime
/// inline-scan pointer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StrongTypeDescriptorInlineScanPlanV1 {
    Null,
    Defined {
        scan: scoop_identity::PersistentScanId,
        definition_plan: ObjectDefinitionPlanId,
    },
}

impl StrongTypeDescriptorInlineScanPlanV1 {
    pub const fn scan(self) -> Option<scoop_identity::PersistentScanId> {
        match self {
            Self::Null => None,
            Self::Defined { scan, .. } => Some(scan),
        }
    }

    pub const fn definition_plan(self) -> Option<ObjectDefinitionPlanId> {
        match self {
            Self::Null => None,
            Self::Defined {
                definition_plan, ..
            } => Some(definition_plan),
        }
    }
}

/// Exact associated atom, if any, that owns the descriptor's physical itable
/// directory.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypeDescriptorITableDirectoryV1 {
    Null,
    Defined(ObjectDefinitionAtomId),
}

impl TypeDescriptorITableDirectoryV1 {
    pub const fn atom(self) -> Option<ObjectDefinitionAtomId> {
        match self {
            Self::Null => None,
            Self::Defined(atom) => Some(atom),
        }
    }
}

/// All semantic identities and graph writers required to emit one strong
/// type-registration record.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongTypeRegistrationPlan<D, C> {
    pub(super) definition_owner: crate::RegistrationDefinitionOwner,
    pub(super) semantic: StrongTypeDescriptorSemanticPlan<D, C>,
    pub(super) runtime_type: RuntimeTypeId,
    pub(super) symbol: PersistentSymbolRequest,
    pub(super) definition_plan: ObjectDefinitionPlanId,
    pub(super) primary_atom: ObjectDefinitionAtomId,
    pub(super) descriptor_symbol: PersistentSymbolRequest,
    pub(super) descriptor_definition_plan: ObjectDefinitionPlanId,
    pub(super) descriptor_primary_atom: ObjectDefinitionAtomId,
    pub(super) diagnostic_atom: ObjectDefinitionAtomId,
    pub(super) itable_directory: TypeDescriptorITableDirectoryV1,
    pub(super) layout_symbol: PersistentSymbolRequest,
    pub(super) layout_definition_plan: ObjectDefinitionPlanId,
    pub(super) layout_primary_atom: ObjectDefinitionAtomId,
    pub(super) inline_scan: StrongTypeDescriptorInlineScanPlanV1,

    pub(super) descriptor_definition_node: DigestNodeId,
    pub(super) layout_fingerprint_node: DigestNodeId,

    pub(super) descriptor_definition_patch: DigestPatchIntentId,
    pub(super) layout_fingerprint_patch: DigestPatchIntentId,
}

impl<D: Copy, C> StrongTypeRegistrationPlan<D, C> {
    pub const fn definition_owner(&self) -> crate::RegistrationDefinitionOwner {
        self.definition_owner
    }

    pub const fn semantic(&self) -> &StrongTypeDescriptorSemanticPlan<D, C> {
        &self.semantic
    }

    pub const fn exact_type(&self) -> PersistentExactTypeId {
        self.semantic.exact_type()
    }

    pub const fn runtime_type(&self) -> RuntimeTypeId {
        self.runtime_type
    }

    pub const fn symbol(&self) -> PersistentSymbolRequest {
        self.symbol
    }

    pub const fn definition_plan(&self) -> ObjectDefinitionPlanId {
        self.definition_plan
    }

    pub const fn primary_atom(&self) -> ObjectDefinitionAtomId {
        self.primary_atom
    }

    pub const fn descriptor_symbol(&self) -> PersistentSymbolRequest {
        self.descriptor_symbol
    }

    pub const fn descriptor_definition_plan(&self) -> ObjectDefinitionPlanId {
        self.descriptor_definition_plan
    }

    pub const fn descriptor_primary_atom(&self) -> ObjectDefinitionAtomId {
        self.descriptor_primary_atom
    }

    pub const fn diagnostic_atom(&self) -> ObjectDefinitionAtomId {
        self.diagnostic_atom
    }

    pub const fn itable_directory(&self) -> TypeDescriptorITableDirectoryV1 {
        self.itable_directory
    }

    pub const fn layout(&self) -> PersistentLayoutId {
        self.semantic.instance_layout()
    }

    pub const fn layout_symbol(&self) -> PersistentSymbolRequest {
        self.layout_symbol
    }

    pub const fn layout_definition_plan(&self) -> ObjectDefinitionPlanId {
        self.layout_definition_plan
    }

    pub const fn layout_primary_atom(&self) -> ObjectDefinitionAtomId {
        self.layout_primary_atom
    }

    pub const fn inline_scan(&self) -> StrongTypeDescriptorInlineScanPlanV1 {
        self.inline_scan
    }

    pub const fn descriptor_definition_node(&self) -> DigestNodeId {
        self.descriptor_definition_node
    }

    pub const fn layout_fingerprint_node(&self) -> DigestNodeId {
        self.layout_fingerprint_node
    }

    pub const fn descriptor_definition_patch(&self) -> DigestPatchIntentId {
        self.descriptor_definition_patch
    }

    pub const fn layout_fingerprint_patch(&self) -> DigestPatchIntentId {
        self.layout_fingerprint_patch
    }
}

/// Proof that every locally materialized type descriptor has exactly one
/// complete strong type-registration production plan.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StrongTypeRegistrationPlanSet<D, C> {
    pub(super) producer: ConeIdentity,
    pub(super) target: TargetProfileWireId,
    pub(super) registrations: Vec<StrongTypeRegistrationPlan<D, C>>,
}
