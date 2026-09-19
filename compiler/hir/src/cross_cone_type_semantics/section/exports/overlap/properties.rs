use super::*;

pub(super) fn validate<E>(
    new: &NominalSupportPropertyInterfaceV1,
    public: &CrossConeHirInterfaceSectionV1,
    graph: &CheckedNominalInheritanceGraphV1<'_>,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), TypeSectionExportValidationError<E>> {
    match new.payload() {
        NominalSupportPropertyPayloadV1::Runtime { interface } => runtime(
            new.declaration(),
            new.declaration_access(),
            interface,
            public,
            graph,
            meter,
            path,
        ),
        NominalSupportPropertyPayloadV1::Const { value } => {
            lookup(public.constants().records().len(), meter, path)?;
            lookup(public.property_interfaces().records().len(), meter, path)?;
            let old = public.constants().get(new.declaration());
            let interface = public
                .property_interfaces()
                .get(PropertyDeclarationId::Property(new.declaration()));
            if !effective_public(new.declaration_access(), graph, meter, path)? {
                return require(old.is_none() && interface.is_none());
            }
            let old = old.ok_or(TypeSectionExportValidationError::PublicOverlap)?;
            let interface = interface.ok_or(TypeSectionExportValidationError::PublicOverlap)?;
            require(
                interface.owner().nominal_owner() == Some(new.owner())
                    && interface.representation() == PropertyRepresentationV1::Const
                    && interface.type_parameters().is_empty()
                    && interface.receiver().is_none(),
            )?;
            require(signature(
                interface.value_type(),
                value.value_type(),
                meter,
                path,
            )?)?;
            require(signature(
                old.value_type(),
                value.value_type(),
                meter,
                path,
            )?)?;
            constant(old.value(), meter, path)?;
            constant(value.value(), meter, path)?;
            origin(old.definition_origin(), meter, path)?;
            origin(value.definition_origin(), meter, path)?;
            require(old == value)
        }
    }
}
pub(super) fn runtime<E>(
    declaration: PersistentPropertyId,
    access: &DeclarationAccessSourceV1,
    source: &NominalSourcePropertyPayloadV1,
    public: &CrossConeHirInterfaceSectionV1,
    graph: &CheckedNominalInheritanceGraphV1<'_>,
    meter: &mut BudgetMeter,
    path: &WirePath,
) -> Result<(), TypeSectionExportValidationError<E>> {
    lookup(public.property_interfaces().records().len(), meter, path)?;
    let old = public
        .property_interfaces()
        .get(PropertyDeclarationId::Property(declaration));
    if !effective_public(access, graph, meter, path)? {
        return require(old.is_none());
    }
    let old = old.ok_or(TypeSectionExportValidationError::PublicOverlap)?;
    require(
        old.owner().nominal_owner() == Some(source.owner())
            && old.type_parameters().is_empty()
            && old.receiver().is_none()
            && old.representation() == source.representation()
            && old.capability().getter() == source.getter(),
    )?;
    require(signature(
        old.value_type(),
        source.value_type(),
        meter,
        path,
    )?)?;
    require(
        (old.access() == PropertyPublicAccessV1::PublicSlot) == !source.slot_relations().is_empty(),
    )?;
    match source.mutability() {
        ProtectedPropertyMutabilityV1::ReadOnly => require(old.capability().setter().is_none()),
        ProtectedPropertyMutabilityV1::ReadWrite {
            setter,
            setter_access,
        } => {
            require(old.capability().setter() == Some(*setter))?;
            let expected = if effective_public(setter_access, graph, meter, path)? {
                PropertySetterPublicAccessV1::Public
            } else {
                PropertySetterPublicAccessV1::Restricted
            };
            require(old.capability().setter_access() == Some(expected))
        }
    }
}
