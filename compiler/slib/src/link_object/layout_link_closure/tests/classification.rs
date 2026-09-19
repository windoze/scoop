use super::*;
use crate::link_object::layout_link_closure::tests::fixture::{Provider, TARGET, meter};
use crate::link_object::strong_relocation_closure::tests::verified_member_with_undefined;
use crate::link_object::symbol_verification::tests::fixture_for_producer;
use crate::link_object::verify_current_cone_strong_relocation_closure_v1;

#[test]
fn layout_link_classification_orders_many_uses_and_preserves_native_remainder() {
    let provider = Provider::new();
    let consumer = provider.consumer(ConeIdentity::SINGLE_FILE);
    let imports = consumer.selected().physical_imports();
    let symbols = ImportSymbolIndex::new(imports, TARGET, &mut meter()).unwrap();
    let name = TARGET
        .contract()
        .native_symbol_normalization()
        .compiler_generated_object_symbol(imports.records()[0].expected_symbol().symbol().as_str());
    let fixtures =
        ["first", "second", "native"].map(|name| fixture_for_producer(consumer.provider(), name));
    let strong = verify_current_cone_strong_relocation_closure_v1(vec![
        verified_member_with_undefined(&fixtures[0], name.as_bytes()),
        verified_member_with_undefined(&fixtures[1], name.as_bytes()),
        verified_member_with_undefined(&fixtures[2], b"_native"),
    ])
    .unwrap();
    let mut reversed = strong.bindings().to_vec();
    reversed.reverse();
    let result = classify(&reversed, &symbols, &mut meter()).unwrap();
    assert_eq!(result.requirements.len(), 2);
    assert!(
        use_key(result.requirements[0].use_site()) < use_key(result.requirements[1].use_site())
    );
    assert_eq!(result.used, [true]);
    assert_eq!(result.remaining.len(), 1);
    assert_eq!(result.remaining[0].symbol(), b"_native");
    let duplicated = vec![
        strong
            .bindings()
            .iter()
            .find(|binding| binding.symbol() == name.as_bytes())
            .unwrap()
            .clone();
        2
    ];
    assert!(matches!(
        classify(&duplicated, &symbols, &mut meter()),
        Err(LayoutLinkClosureError::DuplicateUse { .. })
    ));
}

#[test]
fn layout_link_classification_budget_covers_use_copies_and_search_work() {
    let provider = Provider::new();
    let consumer = provider.consumer(ConeIdentity::SINGLE_FILE);
    let imports = consumer.selected().physical_imports();
    let symbols = ImportSymbolIndex::new(imports, TARGET, &mut meter()).unwrap();
    let name = TARGET
        .contract()
        .native_symbol_normalization()
        .compiler_generated_object_symbol(imports.records()[0].expected_symbol().symbol().as_str());
    let object = fixture_for_producer(consumer.provider(), "copyBudget");
    let strong =
        verify_current_cone_strong_relocation_closure_v1(vec![verified_member_with_undefined(
            &object,
            name.as_bytes(),
        )])
        .unwrap();
    let mut limited = BudgetMeter::new(scoop_wire::DecodeLimits {
        owned_bytes: 0,
        ..Default::default()
    });
    assert!(matches!(
        classify(strong.bindings(), &symbols, &mut limited),
        Err(LayoutLinkClosureError::Resource(_))
    ));
    let mut limited = BudgetMeter::new(scoop_wire::DecodeLimits {
        validation_work_units: 0,
        ..Default::default()
    });
    assert!(matches!(
        classify(strong.bindings(), &symbols, &mut limited),
        Err(LayoutLinkClosureError::Resource(_))
    ));
}
