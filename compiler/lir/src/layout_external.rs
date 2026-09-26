//! Materialized external references from a complete layout/ABI
//! selection.

use scoop_identity::{ConeIdentity, PersistentExactTypeId, StrongCallableDefinitionOwner};

use crate::{
    ExternalCallable, ExternalStrongShapeSubjectV1, ExternalTypeDescriptor,
    LayoutAbiSemanticRecordV1, LayoutAbiSemanticTargetV1, ScoopAbiSignature,
    SelectedDependencyLayoutAbiSetV1, ShapeLinkContractV1, StrongProductionDependencySelectionV2,
};

mod boxing;
mod shapes;

impl SelectedDependencyLayoutAbiSetV1<'_> {
    /// Materializes only the source descriptor or one of its finite helpers.
    pub fn materialize_shape_type_descriptor(
        &self,
        provider: ConeIdentity,
        source: scoop_identity::PersistentTypeId,
        exact: PersistentExactTypeId,
    ) -> Result<ExternalTypeDescriptor, LayoutExternalMaterializationError> {
        shapes::materialize(self, provider, source, exact)
    }

    /// Refines an imported helper from the complete source shape and physical
    /// descriptor contract selected for this consumer.
    pub fn materialize_boxed_value_descriptor(
        &self,
        provider: ConeIdentity,
        source: scoop_identity::PersistentTypeId,
        external: &la_arena::Arena<ExternalTypeDescriptor>,
        descriptor: crate::ExternalTypeDescriptorId,
        storage_type: crate::LirType,
    ) -> Result<crate::BoxedValueDescriptor, LayoutExternalMaterializationError> {
        boxing::materialize(self, provider, source, external, descriptor, storage_type)
    }

    /// Materializes one external TypeDescriptor only when both the semantic
    /// terminal record and its physical import are present in this selection.
    pub fn materialize_type_descriptor(
        &self,
        provider: ConeIdentity,
        exact: PersistentExactTypeId,
    ) -> Result<ExternalTypeDescriptor, LayoutExternalMaterializationError> {
        materialize_type_descriptor(self, provider, exact)
    }

    /// Materializes one callable declaration from the same selected
    /// semantic record and physical import used by Strong production.
    pub fn materialize_callable(
        &self,
        provider: ConeIdentity,
        target: StrongCallableDefinitionOwner,
        signature: ScoopAbiSignature,
    ) -> Result<ExternalCallable, LayoutExternalMaterializationError> {
        materialize_callable(self, provider, target, signature)
    }
}

impl StrongProductionDependencySelectionV2<'_> {
    pub fn selected_shape_support(
        &self,
        provider: ConeIdentity,
        source: scoop_identity::PersistentTypeId,
    ) -> Option<&crate::ParamFreeShapeSupportExportV1> {
        shapes::selected(self, provider, source)
    }

    pub fn materialize_shape_type_descriptor(
        &self,
        provider: ConeIdentity,
        source: scoop_identity::PersistentTypeId,
        exact: PersistentExactTypeId,
    ) -> Result<ExternalTypeDescriptor, LayoutExternalMaterializationError> {
        shapes::materialize(self, provider, source, exact)
    }

    pub fn materialize_boxed_value_descriptor(
        &self,
        provider: ConeIdentity,
        source: scoop_identity::PersistentTypeId,
        external: &la_arena::Arena<ExternalTypeDescriptor>,
        descriptor: crate::ExternalTypeDescriptorId,
        storage_type: crate::LirType,
    ) -> Result<crate::BoxedValueDescriptor, LayoutExternalMaterializationError> {
        boxing::materialize(self, provider, source, external, descriptor, storage_type)
    }

    pub fn materialize_type_descriptor(
        &self,
        provider: ConeIdentity,
        exact: PersistentExactTypeId,
    ) -> Result<ExternalTypeDescriptor, LayoutExternalMaterializationError> {
        materialize_type_descriptor(self, provider, exact)
    }

    pub fn materialize_callable(
        &self,
        provider: ConeIdentity,
        target: StrongCallableDefinitionOwner,
        signature: ScoopAbiSignature,
    ) -> Result<ExternalCallable, LayoutExternalMaterializationError> {
        materialize_callable(self, provider, target, signature)
    }
}

trait MaterializationSelection<'a> {
    fn consumer(&self) -> ConeIdentity;
    fn semantic_record(
        &'a self,
        provider: ConeIdentity,
        target: LayoutAbiSemanticTargetV1,
    ) -> Option<LayoutAbiSemanticRecordV1<'a>>;
    fn physical_imports(&'a self) -> &'a crate::CanonicalExternalShapeLinkImportsV1;
}

impl<'a> MaterializationSelection<'a> for SelectedDependencyLayoutAbiSetV1<'a> {
    fn consumer(&self) -> ConeIdentity {
        self.consumer()
    }

    fn semantic_record(
        &'a self,
        provider: ConeIdentity,
        target: LayoutAbiSemanticTargetV1,
    ) -> Option<LayoutAbiSemanticRecordV1<'a>> {
        self.record(provider, target)
    }

    fn physical_imports(&'a self) -> &'a crate::CanonicalExternalShapeLinkImportsV1 {
        self.physical_imports()
    }
}

impl<'a> MaterializationSelection<'a> for StrongProductionDependencySelectionV2<'a> {
    fn consumer(&self) -> ConeIdentity {
        self.consumer()
    }

    fn semantic_record(
        &'a self,
        provider: ConeIdentity,
        target: LayoutAbiSemanticTargetV1,
    ) -> Option<LayoutAbiSemanticRecordV1<'a>> {
        self.semantic_record(provider, target)
    }

    fn physical_imports(&'a self) -> &'a crate::CanonicalExternalShapeLinkImportsV1 {
        self.physical_imports()
    }
}

fn validate_provider<'a>(
    selected: &impl MaterializationSelection<'a>,
    provider: ConeIdentity,
) -> Result<(), LayoutExternalMaterializationError> {
    if provider == selected.consumer() {
        Err(LayoutExternalMaterializationError::ProviderPartition {
            consumer: selected.consumer(),
            provider,
        })
    } else {
        Ok(())
    }
}

fn materialize_type_descriptor<'a>(
    selected: &'a impl MaterializationSelection<'a>,
    provider: ConeIdentity,
    exact: PersistentExactTypeId,
) -> Result<ExternalTypeDescriptor, LayoutExternalMaterializationError> {
    validate_provider(selected, provider)?;

    let LayoutAbiSemanticRecordV1::Descriptor(record) = selected
        .semantic_record(provider, LayoutAbiSemanticTargetV1::Descriptor(exact))
        .ok_or(LayoutExternalMaterializationError::MissingDescriptor { provider, exact })?
    else {
        return Err(LayoutExternalMaterializationError::DescriptorKind(exact));
    };
    let import = physical_import(
        selected,
        provider,
        ExternalStrongShapeSubjectV1::TypeDescriptor(exact),
    )?;
    let ShapeLinkContractV1::Type {
        descriptor_projection,
    } = import.contract()
    else {
        return Err(LayoutExternalMaterializationError::DescriptorContract(
            exact,
        ));
    };
    if descriptor_projection != record
        || import.expected_symbol() != record.physical_definition().symbol()
        || import.required_definition() != record.physical_definition().definition()
    {
        return Err(LayoutExternalMaterializationError::DescriptorContract(
            exact,
        ));
    }
    Ok(ExternalTypeDescriptor::from_selection(
        provider,
        exact,
        import.expected_symbol(),
        import.required_definition(),
    ))
}

fn materialize_callable<'a>(
    selected: &'a impl MaterializationSelection<'a>,
    provider: ConeIdentity,
    target: StrongCallableDefinitionOwner,
    signature: ScoopAbiSignature,
) -> Result<ExternalCallable, LayoutExternalMaterializationError> {
    validate_provider(selected, provider)?;

    let record = selected
        .semantic_record(provider, LayoutAbiSemanticTargetV1::Callable(target))
        .ok_or(LayoutExternalMaterializationError::MissingCallable { provider, target })?;
    let import = physical_import(
        selected,
        provider,
        ExternalStrongShapeSubjectV1::Callable(target),
    )?;
    let ShapeLinkContractV1::CallableAbi {
        canonical_signature,
        calling_convention,
        protocol,
    } = import.contract()
    else {
        return Err(LayoutExternalMaterializationError::CallableContract(target));
    };
    let record = match record {
        LayoutAbiSemanticRecordV1::DirectCallable(record) => {
            let gc = protocol.gc_effect();
            if canonical_signature != record.abi_signature()
                || *calling_convention != record.calling_convention()
                || gc != record.root_plan().canonical_gc_effect()
                || import.expected_symbol() != record.expected_symbol()
                || import.required_definition() != record.required_definition()
            {
                return Err(LayoutExternalMaterializationError::CallableContract(target));
            }
            return ExternalCallable::from_layout_direct(provider, record, signature);
        }
        LayoutAbiSemanticRecordV1::Callable(record) => record,
        _ => return Err(LayoutExternalMaterializationError::CallableKind(target)),
    };
    if canonical_signature != record.canonical_signature()
        || *calling_convention != record.calling_convention()
        || *protocol != record.call_protocol()
        || import.expected_symbol() != record.physical_definition().symbol()
        || import.required_definition() != record.physical_definition().definition()
    {
        return Err(LayoutExternalMaterializationError::CallableContract(target));
    }
    ExternalCallable::from_layout_v1(
        provider,
        record,
        import.expected_symbol(),
        import.required_definition(),
        signature,
    )
}

fn physical_import<'a>(
    selected: &'a impl MaterializationSelection<'a>,
    provider: ConeIdentity,
    subject: ExternalStrongShapeSubjectV1,
) -> Result<&'a crate::ExternalShapeLinkImportV1, LayoutExternalMaterializationError> {
    selected
        .physical_imports()
        .records()
        .iter()
        .find(|record| record.provider() == provider && record.subject() == subject)
        .ok_or(LayoutExternalMaterializationError::MissingPhysicalImport { provider, subject })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LayoutExternalMaterializationError {
    Resource(scoop_wire::WireError),
    ProviderPartition {
        consumer: ConeIdentity,
        provider: ConeIdentity,
    },
    MissingDescriptor {
        provider: ConeIdentity,
        exact: PersistentExactTypeId,
    },
    DescriptorKind(PersistentExactTypeId),
    DescriptorContract(PersistentExactTypeId),
    MissingShapeSupport {
        provider: ConeIdentity,
        source: scoop_identity::PersistentTypeId,
    },
    UnavailableBoxedValue(scoop_identity::PersistentTypeId),
    UnavailableShapeDescriptor {
        source: scoop_identity::PersistentTypeId,
        exact: PersistentExactTypeId,
    },
    BoxDescriptor(crate::BoxDescriptorError),
    MissingCallable {
        provider: ConeIdentity,
        target: StrongCallableDefinitionOwner,
    },
    CallableKind(StrongCallableDefinitionOwner),
    CallableContract(StrongCallableDefinitionOwner),
    PhysicalCallable(StrongCallableDefinitionOwner),
    MissingPhysicalImport {
        provider: ConeIdentity,
        subject: ExternalStrongShapeSubjectV1,
    },
    CallableAbi(crate::ExternalCallableBuildError),
}

impl From<scoop_wire::WireError> for LayoutExternalMaterializationError {
    fn from(error: scoop_wire::WireError) -> Self {
        Self::Resource(error)
    }
}

impl std::fmt::Display for LayoutExternalMaterializationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "cannot materialize selected layout/ABI external: {self:?}"
        )
    }
}

impl std::error::Error for LayoutExternalMaterializationError {}
