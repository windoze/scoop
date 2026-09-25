use super::*;

pub(super) fn insert(
    selections: &mut Selections,
    slot: PersistentDispatchSlotId,
    selection: Selection,
) -> Result<(), Error> {
    if let Some(previous) = selections.get(&slot) {
        if *previous != selection {
            return Err(invalid(
                "one source slot has conflicting implementation selections",
            ));
        }
    } else {
        selections.insert(slot, selection);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use scoop_identity::*;

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

        for _ in 0..2 {
            insert(&mut selections, slot, Selection::Concrete(callable)).unwrap();
        }
        assert_eq!(selections.len(), 1);
        for conflicting in [Selection::Abstract, Selection::InterfaceDefault(callable)] {
            assert!(matches!(insert(&mut selections, slot, conflicting),
                Err(Error::InvalidSourceDeclaration(reason)) if reason.contains("conflicting implementation selections")));
            assert_eq!(selections[&slot], Selection::Concrete(callable));
        }
    }
}
