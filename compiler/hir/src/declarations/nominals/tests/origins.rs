use super::*;

#[test]
fn application_origins_resolve_after_declarations_are_reordered() {
    let unit = TypeId::from_raw(0.into());
    let choice_application = EnumApplicationId::from_raw(0.into());
    let other_application = EnumApplicationId::from_raw(1.into());
    let variant = |name: &str| Variant {
        name: name.to_owned(),
        style: VariantStyle::Unit,
        fields: Vec::new(),
    };
    let mut enums = Arena::new();
    let choice = enums.alloc(enum_declaration(
        "Choice",
        choice_application,
        vec![variant("Chosen")],
    ));
    let other = enums.alloc(enum_declaration(
        "Other",
        other_application,
        vec![variant("Unused")],
    ));
    let original = identities(&Arena::new(), &enums);
    let mut applications = Arena::new();
    for declaration in [choice, other] {
        applications.alloc(EnumApplication {
            template: original[declaration].declaration_id(),
            arguments: Vec::new(),
            canonical_type: unit,
        });
    }

    let mut reordered = Arena::new();
    reordered.alloc(enums[other].clone());
    let reordered_choice = reordered.alloc(enums[choice].clone());
    let target = identities(&Arena::new(), &reordered);
    assert_ne!(choice, reordered_choice);
    assert_eq!(original[choice], target[reordered_choice]);
    let reference = AppliedEnumVariantRef::checked_index(
        &reordered,
        &applications,
        &target,
        choice_application,
        0,
    )
    .expect("the application selects the original declaration, independent of its arena index");
    assert_eq!(reference.declaration().enumeration(), reordered_choice);
    assert_eq!(
        reordered[reference.declaration().enumeration()].variants[reference.local_index() as usize]
            .name,
        "Chosen"
    );
    assert!(
        AppliedEnumVariantRef::checked(
            &reordered,
            &applications,
            &target,
            choice_application,
            EnumVariantRef::checked(&reordered, choice, 0).unwrap(),
        )
        .is_none()
    );
}
