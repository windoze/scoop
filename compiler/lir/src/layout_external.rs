//! Materialized external references from a complete layout/ABI
//! selection.

use scoop_identity::{ConeIdentity, PersistentExactTypeId, StrongCallableDefinitionOwner};
use scoop_wire::{BudgetMeter, WirePath};

use crate::{
    DependencyExternalCallable, DependencyExternalTypeDescriptorV2, EnumDefs,
    ExternalStrongShapeSubjectV1, LayoutAbiSemanticRecordV1, LayoutAbiSemanticTargetV1,
    ScoopAbiSignature, SelectedDependencyLayoutAbiSetV1, ShapeLinkContractV1,
    StrongProductionDependencySelectionV2,
};

impl SelectedDependencyLayoutAbiSetV1<'_> {
    /// Materializes one external TypeDescriptor only when both the semantic
    /// terminal record and its physical import are present in this selection.
    pub fn materialize_type_descriptor(
        &self,
        provider: ConeIdentity,
        exact: PersistentExactTypeId,
        meter: &mut BudgetMeter,
    ) -> Result<DependencyExternalTypeDescriptorV2, LayoutExternalMaterializationError> {
        materialize_type_descriptor(self, provider, exact, meter)
    }

    /// Materializes one dispatch-callable declaration from the same selected
    /// semantic record and physical import used by Strong production.
    pub fn materialize_dispatch_callable(
        &self,
        provider: ConeIdentity,
        target: StrongCallableDefinitionOwner,
        signature: ScoopAbiSignature,
        enums: &EnumDefs,
        meter: &mut BudgetMeter,
    ) -> Result<DependencyExternalCallable, LayoutExternalMaterializationError> {
        materialize_dispatch_callable(self, provider, target, signature, enums, meter)
    }
}

impl StrongProductionDependencySelectionV2<'_> {
    pub fn materialize_type_descriptor(
        &self,
        provider: ConeIdentity,
        exact: PersistentExactTypeId,
        meter: &mut BudgetMeter,
    ) -> Result<DependencyExternalTypeDescriptorV2, LayoutExternalMaterializationError> {
        materialize_type_descriptor(self, provider, exact, meter)
    }

    pub fn materialize_dispatch_callable(
        &self,
        provider: ConeIdentity,
        target: StrongCallableDefinitionOwner,
        signature: ScoopAbiSignature,
        enums: &EnumDefs,
        meter: &mut BudgetMeter,
    ) -> Result<DependencyExternalCallable, LayoutExternalMaterializationError> {
        materialize_dispatch_callable(self, provider, target, signature, enums, meter)
    }
}

trait MaterializationSelection<'a> {
    fn consumer(&self) -> ConeIdentity;
    fn semantic_count(&self) -> usize;
    fn semantic_record(
        &'a self,
        provider: ConeIdentity,
        target: LayoutAbiSemanticTargetV1,
    ) -> Option<LayoutAbiSemanticRecordV1<'a>>;
    fn physical_imports(&'a self) -> &'a crate::CanonicalExternalShapeLinkImportsV1<'a>;
}

impl<'a> MaterializationSelection<'a> for SelectedDependencyLayoutAbiSetV1<'a> {
    fn consumer(&self) -> ConeIdentity {
        self.consumer()
    }

    fn semantic_count(&self) -> usize {
        self.len()
    }

    fn semantic_record(
        &'a self,
        provider: ConeIdentity,
        target: LayoutAbiSemanticTargetV1,
    ) -> Option<LayoutAbiSemanticRecordV1<'a>> {
        self.reference(provider, target)
            .and_then(|reference| self.resolve(reference))
    }

    fn physical_imports(&'a self) -> &'a crate::CanonicalExternalShapeLinkImportsV1<'a> {
        self.physical_imports()
    }
}

impl<'a> MaterializationSelection<'a> for StrongProductionDependencySelectionV2<'a> {
    fn consumer(&self) -> ConeIdentity {
        self.consumer()
    }

    fn semantic_count(&self) -> usize {
        self.semantic_count()
    }

    fn semantic_record(
        &'a self,
        provider: ConeIdentity,
        target: LayoutAbiSemanticTargetV1,
    ) -> Option<LayoutAbiSemanticRecordV1<'a>> {
        self.semantic_record(provider, target)
    }

    fn physical_imports(&'a self) -> &'a crate::CanonicalExternalShapeLinkImportsV1<'a> {
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
    meter: &mut BudgetMeter,
) -> Result<DependencyExternalTypeDescriptorV2, LayoutExternalMaterializationError> {
    validate_provider(selected, provider)?;
    meter.charge_work(selected.semantic_count() as u64, &WirePath::root())?;
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
        meter,
    )?;
    let ShapeLinkContractV1::Type {
        descriptor_projection,
    } = import.contract()
    else {
        return Err(LayoutExternalMaterializationError::DescriptorContract(
            exact,
        ));
    };
    if *descriptor_projection != record
        || import.expected_symbol() != record.physical_definition().symbol()
        || import.required_definition() != record.physical_definition().definition()
    {
        return Err(LayoutExternalMaterializationError::DescriptorContract(
            exact,
        ));
    }
    Ok(DependencyExternalTypeDescriptorV2::from_layout_v1(
        provider,
        exact,
        import.expected_symbol(),
        import.required_definition(),
    ))
}

fn materialize_dispatch_callable<'a>(
    selected: &'a impl MaterializationSelection<'a>,
    provider: ConeIdentity,
    target: StrongCallableDefinitionOwner,
    signature: ScoopAbiSignature,
    enums: &EnumDefs,
    meter: &mut BudgetMeter,
) -> Result<DependencyExternalCallable, LayoutExternalMaterializationError> {
    validate_provider(selected, provider)?;
    meter.charge_work(selected.semantic_count() as u64, &WirePath::root())?;
    let LayoutAbiSemanticRecordV1::Callable(record) = selected
        .semantic_record(provider, LayoutAbiSemanticTargetV1::Callable(target))
        .ok_or(LayoutExternalMaterializationError::MissingCallable { provider, target })?
    else {
        return Err(LayoutExternalMaterializationError::CallableKind(target));
    };
    let import = physical_import(
        selected,
        provider,
        ExternalStrongShapeSubjectV1::Callable(target),
        meter,
    )?;
    let ShapeLinkContractV1::CallableAbi {
        canonical_signature,
        calling_convention,
        protocol,
    } = import.contract()
    else {
        return Err(LayoutExternalMaterializationError::CallableContract(target));
    };
    if *canonical_signature != record.canonical_signature()
        || *calling_convention != record.calling_convention()
        || *protocol != record.call_protocol()
        || import.expected_symbol() != record.physical_definition().symbol()
        || import.required_definition() != record.physical_definition().definition()
    {
        return Err(LayoutExternalMaterializationError::CallableContract(target));
    }
    DependencyExternalCallable::from_layout_v1(
        provider,
        record,
        import.expected_symbol(),
        import.required_definition(),
        signature,
        enums,
        meter,
    )
}

fn physical_import<'a>(
    selected: &'a impl MaterializationSelection<'a>,
    provider: ConeIdentity,
    subject: ExternalStrongShapeSubjectV1,
    meter: &mut BudgetMeter,
) -> Result<&'a crate::ExternalShapeLinkImportV1<'a>, LayoutExternalMaterializationError> {
    meter.charge_work(
        selected.physical_imports().records().len() as u64,
        &WirePath::root(),
    )?;
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
    CallableAbi(crate::ExactCallablePhysicalAbiError),
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
