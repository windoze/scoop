use super::*;

#[test]
fn subscript_aliases_only_replace_their_actual_selected_targets() {
    for (get_extension, set_extension) in [(true, false), (false, true), (true, true)] {
        let get = "operator fun get(index: Boolean): Value = item";
        let set = "operator fun set(index: Boolean, value: Value) { item = value }";
        let mut provider = String::from(
            "package slots\nclass Value { operator fun inc(): Value = this }\nclass Box(var item: Value) {\n",
        );
        if !get_extension {
            provider.push_str(get);
            provider.push(';');
        }
        if !set_extension {
            provider.push_str(set);
        }
        provider.push_str("}\n");
        let mut consumer = String::from("import slots.Box\n");
        if get_extension {
            provider.push_str(&get.replace("fun get", "fun Box.get"));
            provider.push('\n');
            consumer.push_str("import slots.get as readSlot\n");
        }
        if set_extension {
            provider.push_str(&set.replace("fun set", "fun Box.set"));
            provider.push('\n');
            consumer.push_str("import slots.set as writeSlot\n");
        }
        consumer.push_str("fun editing(box: Box) { box[true]++ }");
        let mut analysis =
            analyze_files(&[("src/slots.scoop", &provider), ("src/use.scoop", &consumer)]);
        let references = references(&mut analysis);
        assert_eq!(references.len(), 3);
        let index = analysis.reference_index(|| false).unwrap();
        assert!(index.unavailable.is_empty(), "{:?}", index.unavailable);
        let bracket = at("src/use.scoop", &consumer, "[true]", 1);
        let targets = index.targets_at(&bracket);
        assert_eq!(targets.len(), 2, "{targets:?}");
        for (method, alias, extension) in [
            ("get", "readSlot", get_extension),
            ("set", "writeSlot", set_extension),
        ] {
            let target = index.targets_at(&at(
                "src/slots.scoop",
                &provider,
                &format!("{method}("),
                method.len(),
            ))[0];
            let occurrence = index
                .references_to(target)
                .find(|reference| reference.origin == bracket)
                .unwrap();
            if extension {
                let alias_id = index
                    .aliases
                    .iter()
                    .find(|(_, entry)| entry.name == alias)
                    .unwrap()
                    .0;
                let alias_target = hir::EditingReferenceTarget::ImportAlias(alias_id);
                assert!(targets.contains(&alias_target));
                assert!(!targets.contains(&target));
                assert_eq!(
                    occurrence.aliases,
                    vec![at("src/use.scoop", &consumer, alias, alias.len())]
                );
                assert_eq!(index.references_to(alias_target).count(), 2);
            } else {
                assert!(targets.contains(&target));
                assert!(occurrence.aliases.is_empty());
            }
        }
    }
}
