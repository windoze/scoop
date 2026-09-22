use super::*;

pub(super) fn collect(
    public: &hir::CrossConeHirInterfaceSectionV1,
    source: &hir::CrossConeTypeSemanticsProductionV1,
    meter: &mut BudgetMeter,
) -> Result<BTreeMap<Declaration, SourceContract>, Error> {
    let mut required = BTreeMap::new();
    for record in public.callable_interfaces().records() {
        // Closed source signatures are required here; generic declarations
        // remain HIR metadata until their separate ODR materialization path.
        signature_cost(record, meter)?;
        if record.effects().implementation() != hir::CallableImplementationV1::Scoop
            || !record.type_parameters().is_empty()
            || matches!(
                record.owner().nominal_owner(),
                Some(hir::SourceNominalId::GenericTemplate(_))
            )
            || record.receiver().is_some_and(|ty| ty.contains_binder())
            || record.result().contains_binder()
            || record
                .parameters()
                .parameters()
                .iter()
                .any(|parameter| parameter.value_type().contains_binder())
        {
            continue;
        }
        if let Some(declaration) = declaration(record.declaration()) {
            insert(
                &mut required,
                declaration,
                SourceContract::new(record.effects(), record.modality()),
                meter,
            )?;
        }
    }
    for record in source.source_protected_callables().records() {
        work(1, meter)?;
        let payload = record.payload();
        if payload.effects().implementation() != hir::CallableImplementationV1::Scoop
            || !payload.type_parameters().is_empty()
            || matches!(payload.owner(), hir::SourceNominalId::GenericTemplate(_))
        {
            continue;
        }
        if let Some(declaration) = declaration(record.declaration()) {
            insert(
                &mut required,
                declaration,
                SourceContract::new(payload.effects(), payload.modality()),
                meter,
            )?;
        }
    }
    for record in source.source_callables().records() {
        work(1, meter)?;
        if record.signature().effects().implementation() != hir::CallableImplementationV1::Scoop {
            continue;
        }
        let declaration = match record.declaration() {
            hir::InheritanceCallableDeclarationV1::Function(id) => Declaration::Function(id),
            hir::InheritanceCallableDeclarationV1::Getter(id)
            | hir::InheritanceCallableDeclarationV1::Setter(id) => {
                Declaration::PropertyAccessor(id)
            }
        };
        insert(
            &mut required,
            declaration,
            SourceContract::new(record.signature().effects(), record.modality()),
            meter,
        )?;
    }
    Ok(required)
}

fn declaration(declaration: CallableTemplateOrigin) -> Option<Declaration> {
    match declaration {
        CallableTemplateOrigin::Function(id) => Some(Declaration::Function(id)),
        CallableTemplateOrigin::Accessor(id) => Some(Declaration::PropertyAccessor(id)),
        CallableTemplateOrigin::GenericFunction(_)
        | CallableTemplateOrigin::Constructor(_)
        | CallableTemplateOrigin::VariantConstructor(_) => None,
    }
}

pub(super) fn insert(
    required: &mut BTreeMap<Declaration, SourceContract>,
    declaration: Declaration,
    contract: SourceContract,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    work(
        u64::from(required.len().checked_ilog2().unwrap_or(0)) + 2,
        meter,
    )?;
    match required.entry(declaration) {
        std::collections::btree_map::Entry::Occupied(existing) => {
            if *existing.get() != contract {
                return Err(Error::ConflictingSource(declaration));
            }
        }
        std::collections::btree_map::Entry::Vacant(entry) => {
            meter.charge_collection_slots(1, &WirePath::root())?;
            meter.charge_owned_bytes(
                std::mem::size_of::<(Declaration, SourceContract)>() as u64,
                &WirePath::root(),
            )?;
            entry.insert(contract);
        }
    }
    Ok(())
}

fn signature_cost(
    record: &hir::CallableInterfaceRecordV1,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    // The canonical byte length bounds the binder walks over this existing
    // source record without allocating another signature tree.
    let length = scoop_wire::encoded_length(record).map_err(Error::Encoding)?;
    work(length, meter)
}
