use super::*;

impl ExportConstValueSemanticAuthority<TestAuthorityError> for EmptyPublicAuthority {
    fn current_cone(&self) -> ConeIdentity {
        self.0
    }

    fn const_property_declaration_source(
        &mut self,
        _property: PersistentPropertyId,
    ) -> Result<ConstPropertyDeclarationSourceV1, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }

    fn validated_property_interface(
        &mut self,
        _property: PersistentPropertyId,
    ) -> Result<&PropertyInterfaceRecordV1, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }

    fn validate_const_value_type(
        &mut self,
        _value_type: PersistentTypeId,
        _kind: scoop_hir::CanonicalConstValueKindV1,
    ) -> Result<(), TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }
}

impl PublicExportBindingClosureAuthority for EmptyPublicAuthority {
    fn closure_node_count(&self) -> usize {
        1
    }

    fn is_direct_dependency(
        &self,
        _provider: ConeIdentity,
        meter: &mut scoop_wire::BudgetMeter,
        path: &scoop_wire::WirePath,
    ) -> Result<bool, scoop_wire::WireError> {
        meter.charge_work(1, path)?;
        Ok(false)
    }

    fn binding_key(
        &self,
        _binding: PersistentExportBindingId,
        meter: &mut scoop_wire::BudgetMeter,
        path: &scoop_wire::WirePath,
    ) -> Result<Option<&ExportBindingKey>, scoop_wire::WireError> {
        meter.charge_work(1, path)?;
        Ok(None)
    }

    fn public_bindings(
        &self,
        _exporter: ConeIdentity,
        meter: &mut scoop_wire::BudgetMeter,
        path: &scoop_wire::WirePath,
    ) -> Result<Option<&CanonicalPublicExportBindingsV1>, scoop_wire::WireError> {
        meter.charge_work(1, path)?;
        Ok(None)
    }
}

impl ExternalHirReferenceSemanticAuthority<TestAuthorityError> for EmptyPublicAuthority {
    fn current_cone(&self) -> ConeIdentity {
        self.0
    }

    fn external_hir_target_origin(
        &mut self,
        _target: ExternalHirTargetV1,
    ) -> Result<ConeIdentity, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }

    fn external_hir_target_binding_root(
        &mut self,
        _target: ExternalHirTargetV1,
    ) -> Result<BindingTarget, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }
}
