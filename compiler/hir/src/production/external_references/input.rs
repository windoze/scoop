use crate::{
    CanonicalCallableInterfacesV1, CanonicalCallableSourceInterfacesV1,
    CanonicalExportConstValuesV1, CanonicalExportDefaultTemplatesV1, CanonicalNominalInterfacesV1,
    CanonicalPropertyInterfacesV1, CanonicalPublicExportBindingsV1, CanonicalTypeAliasInterfacesV1,
    DependencyBindingWitnessV1, ExternalHirReferenceRoleV1, ExternalHirTargetV1,
};

/// The fields whose foreign typed leaves determine the export-side external
/// HIR reference closure.
#[derive(Clone, Copy)]
pub struct ExternalHirReferenceProductionInput<'a> {
    pub(super) public_bindings: &'a CanonicalPublicExportBindingsV1,
    pub(super) nominal_interfaces: &'a CanonicalNominalInterfacesV1,
    pub(super) callable_interfaces: &'a CanonicalCallableInterfacesV1,
    pub(super) property_interfaces: &'a CanonicalPropertyInterfacesV1,
    pub(super) type_aliases: &'a CanonicalTypeAliasInterfacesV1,
    pub(super) source_interfaces: &'a CanonicalCallableSourceInterfacesV1,
    pub(super) default_templates: &'a CanonicalExportDefaultTemplatesV1,
    pub(super) constants: &'a CanonicalExportConstValuesV1,
}

impl<'a> ExternalHirReferenceProductionInput<'a> {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        public_bindings: &'a CanonicalPublicExportBindingsV1,
        nominal_interfaces: &'a CanonicalNominalInterfacesV1,
        callable_interfaces: &'a CanonicalCallableInterfacesV1,
        property_interfaces: &'a CanonicalPropertyInterfacesV1,
        type_aliases: &'a CanonicalTypeAliasInterfacesV1,
        source_interfaces: &'a CanonicalCallableSourceInterfacesV1,
        default_templates: &'a CanonicalExportDefaultTemplatesV1,
        constants: &'a CanonicalExportConstValuesV1,
    ) -> Self {
        Self {
            public_bindings,
            nominal_interfaces,
            callable_interfaces,
            property_interfaces,
            type_aliases,
            source_interfaces,
            default_templates,
            constants,
        }
    }
}

/// A role whose exact source-name route cannot be reconstructed from fields
/// 1 through 8 and therefore must be retained by lowering.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ExternalHirBindingWitnessRole {
    AliasTarget,
    DefaultDependency,
    ConcreteSelectedUse,
}

impl ExternalHirBindingWitnessRole {
    pub const fn reference_role(self) -> ExternalHirReferenceRoleV1 {
        match self {
            Self::AliasTarget => ExternalHirReferenceRoleV1::AliasTarget,
            Self::DefaultDependency => ExternalHirReferenceRoleV1::DefaultDependency,
            Self::ConcreteSelectedUse => ExternalHirReferenceRoleV1::ConcreteSelectedUse,
        }
    }
}

/// One actual source-name authorization selected while lowering an alias,
/// default expression, or concrete foreign use.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalHirBindingWitnessUse {
    target: ExternalHirTargetV1,
    role: ExternalHirBindingWitnessRole,
    witness: DependencyBindingWitnessV1,
}

impl ExternalHirBindingWitnessUse {
    pub const fn new(
        target: ExternalHirTargetV1,
        role: ExternalHirBindingWitnessRole,
        witness: DependencyBindingWitnessV1,
    ) -> Self {
        Self {
            target,
            role,
            witness,
        }
    }

    pub const fn target(&self) -> ExternalHirTargetV1 {
        self.target
    }

    pub const fn role(&self) -> ExternalHirBindingWitnessRole {
        self.role
    }

    pub const fn witness(&self) -> &DependencyBindingWitnessV1 {
        &self.witness
    }
}
