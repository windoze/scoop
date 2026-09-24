use super::*;

pub(super) fn insert(
    selections: &mut Selections,
    slot: PersistentDispatchSlotId,
    selection: Selection,
    meter: &mut BudgetMeter,
) -> Result<(), Error> {
    let path = WirePath::root();
    meter
        .charge_work(u64::from(selections.len().max(1).ilog2()) + 1, &path)
        .map_err(resource)?;
    meter.charge_collection_slots(1, &path).map_err(resource)?;
    if let Some(previous) = selections.get(&slot) {
        if *previous != selection {
            return Err(invalid(
                "one source slot has conflicting implementation selections",
            ));
        }
    } else {
        meter
            .check_table_entries(selections.len() as u64 + 1, &path)
            .map_err(resource)?;
        selections.insert(slot, selection);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoop_identity::*;
    use scoop_wire::DecodeLimits;

    #[test]
    fn repeated_source_paths_merge_only_when_their_actual_choices_agree() {
        let site = SourceDeclarationSite::new(
            ConeIdentity::SINGLE_FILE,
            PackagePath::root(),
            DefinitionOwnerChain::top_level(),
            DeclarationScope::ConeWide,
        )
        .unwrap();
        let function =
            PersistentFunctionId::from_source_declaration(&SourceDeclarationKey::function(
                site,
                CanonicalIdentifier::new("method").unwrap(),
                0,
                None,
                Vec::new(),
            ))
            .unwrap();
        let slot = PersistentDispatchSlotId::from_key(&DispatchSlotKey::interface_method(function))
            .unwrap();
        let callable = InheritanceCallableDeclarationV1::Function(function);
        let mut selections = Selections::new();
        let mut meter = BudgetMeter::new(DecodeLimits::default());
        for _ in 0..2 {
            insert(
                &mut selections,
                slot,
                Selection::Concrete(callable),
                &mut meter,
            )
            .unwrap();
        }
        assert_eq!(selections.len(), 1);
        for conflicting in [Selection::Abstract, Selection::InterfaceDefault(callable)] {
            assert!(
                matches!(insert(&mut selections, slot, conflicting, &mut meter),
                Err(Error::InvalidSourceDeclaration(reason)) if reason.contains("conflicting implementation selections"))
            );
            assert_eq!(selections[&slot], Selection::Concrete(callable));
        }
        let mut exhausted = BudgetMeter::new(DecodeLimits {
            validation_work_units: 0,
            ..DecodeLimits::default()
        });
        assert!(matches!(
            insert(
                &mut selections,
                slot,
                Selection::Concrete(callable),
                &mut exhausted
            ),
            Err(Error::SourceInventory(SourceInventoryError::Resource(_)))
        ));
    }
}
