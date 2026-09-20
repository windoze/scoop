use super::*;

impl NestedNominalSemanticAuthority<TestAuthorityError> for EmptyDeclarations {
    fn nominal_source_binders(
        &self,
        _owner: SourceNominalId,
    ) -> Result<&CanonicalBinderListV1, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }

    fn nominal_source_supertypes(
        &self,
        _owner: SourceNominalId,
    ) -> Result<&CanonicalSignatureTypesV1, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }

    fn nominal_source_constructors(
        &self,
        _owner: SourceNominalId,
    ) -> Result<&CanonicalPersistentIdsV1<PersistentConstructorId>, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }

    fn nominal_source_members(
        &self,
        _owner: SourceNominalId,
    ) -> Result<&CanonicalNestedMemberRefsV1, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }

    fn nominal_source_children(
        &self,
        _owner: SourceNominalId,
    ) -> Result<&CanonicalNestedNominalRefsV1, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }

    fn nominal_source_shape(
        &self,
        _owner: SourceNominalId,
    ) -> Result<&NominalSourceShapeV1, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }
}

impl InheritanceSlotSchemaSemanticAuthority<TestAuthorityError> for EmptyDeclarations {
    fn interface_dispatch_source(
        &self,
        _owner: PersistentExactTypeId,
    ) -> Result<&InterfaceSourceDispatchV1, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }
    fn schemas(
        &self,
        owner: PersistentExactTypeId,
    ) -> Result<&CanonicalInheritanceSlotSchemasV1, TestAuthorityError> {
        self.nominal
            .as_ref()
            .filter(|nominal| nominal.exact == owner)
            .map(|nominal| &nominal.schemas)
            .ok_or(TestAuthorityError::UnexpectedCall)
    }

    fn dispatch_slot_key(
        &self,
        _slot: PersistentDispatchSlotId,
    ) -> Result<&DispatchSlotKey, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }

    fn function_key(
        &self,
        _function: PersistentFunctionId,
    ) -> Result<&SourceDeclarationKey, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }

    fn accessor_key(
        &self,
        _accessor: PersistentPropertyAccessorId,
    ) -> Result<&PropertyAccessorKey, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }

    fn property_key(
        &self,
        _property: PersistentPropertyId,
    ) -> Result<&SourceDeclarationKey, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }
}

impl InheritanceSlotContractSemanticAuthority<TestAuthorityError> for EmptyDeclarations {
    fn unit_exact_type(&self) -> Result<PersistentExactTypeId, TestAuthorityError> {
        PersistentExactTypeId::from_key(&ExactTypeKey::Nominal(
            CoreBuiltinNominal::Unit.identity_record().id(),
        ))
        .map_err(|_| TestAuthorityError::UnexpectedCall)
    }
}

impl NominalInheritanceInterfaceSemanticAuthority<TestAuthorityError> for EmptyDeclarations {
    fn required_inheritance_owners(
        &self,
    ) -> Result<&CanonicalPersistentIdsV1<PersistentExactTypeId>, TestAuthorityError> {
        Ok(&self.inheritance)
    }

    fn required_inheritance_constructors(
        &self,
        owner: PersistentExactTypeId,
    ) -> Result<&CanonicalPersistentIdsV1<PersistentConstructorId>, TestAuthorityError> {
        self.nominal
            .as_ref()
            .filter(|nominal| nominal.exact == owner)
            .map(|_| &self.constructors)
            .ok_or(TestAuthorityError::UnexpectedCall)
    }

    fn required_inheritance_protected_members(
        &self,
        owner: PersistentExactTypeId,
    ) -> Result<&CanonicalProtectedDeclarationRefsV1, TestAuthorityError> {
        self.nominal
            .as_ref()
            .filter(|nominal| nominal.exact == owner)
            .map(|_| &self.protected)
            .ok_or(TestAuthorityError::UnexpectedCall)
    }

    fn constructor_source(
        &self,
        _declaration: PersistentConstructorId,
    ) -> Result<&NominalSupportConstructorInterfaceV1, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }

    fn inheritance_callable_source(
        &self,
        _declaration: InheritanceCallableDeclarationV1,
    ) -> Result<InheritanceSourceCallableFactsV1<'_>, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }

    fn inheritance_slot_selection(
        &self,
        _owner: PersistentExactTypeId,
        _slot: PersistentDispatchSlotId,
    ) -> Result<InheritanceSourceSlotSelectionV1, TestAuthorityError> {
        Err(TestAuthorityError::UnexpectedCall)
    }
}
