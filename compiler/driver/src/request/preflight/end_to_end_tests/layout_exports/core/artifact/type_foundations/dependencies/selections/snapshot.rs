use super::*;
use scoop_identity::{DeclarationName, PersistentTypeId, PropertyAccessorKey, PropertyOwner};

pub(super) fn render(checked: CheckedSharedTypeFoundationV1<'_>) -> String {
    let mut lines = Vec::new();
    for nominal in checked.section().inheritance().records() {
        let key = checked
            .metadata()
            .identities
            .canonical_key::<_, scoop_identity::ExactTypeKey>(nominal.owner())
            .unwrap();
        let scoop_identity::ExactTypeKey::Nominal(owner) = key.as_ref() else {
            panic!("source nominal required");
        };
        for slot in nominal.slots().records() {
            let implementation = match slot.implementation() {
                Implementation::Abstract(target)
                | Implementation::Concrete(target)
                | Implementation::InterfaceDefault(target) => {
                    format!(
                        "{} {}.{}",
                        match slot.implementation() {
                            Implementation::Abstract(_) => "abstract",
                            Implementation::Concrete(_) => "concrete",
                            Implementation::InterfaceDefault(_) => "default",
                        },
                        nominal_name(checked, target.owner()),
                        callable_name(checked, target.declaration()),
                    )
                }
            };
            lines.push(format!(
                "{} {}.{} -> {implementation}\n",
                nominal_name(checked, *owner),
                nominal_name(checked, slot.declaration_owner()),
                callable_name(checked, slot.declaration())
            ));
        }
    }
    lines.sort();
    lines.concat()
}

fn nominal_name(checked: CheckedSharedTypeFoundationV1<'_>, owner: PersistentTypeId) -> String {
    name(
        &checked
            .metadata()
            .identities
            .canonical_key::<_, SourceDeclarationKey>(owner)
            .unwrap(),
    )
}

fn callable_name(checked: CheckedSharedTypeFoundationV1<'_>, declaration: Declaration) -> String {
    let identities = checked.metadata().identities;
    match declaration {
        Declaration::Function(id) => name(
            &identities
                .canonical_key::<_, SourceDeclarationKey>(id)
                .unwrap(),
        ),
        Declaration::Getter(id) | Declaration::Setter(id) => {
            let key = identities
                .canonical_key::<_, PropertyAccessorKey>(id)
                .unwrap();
            let PropertyOwner::Property(property) = key.owner() else {
                panic!("nominal property required");
            };
            format!(
                "{}.{}",
                name(
                    &identities
                        .canonical_key::<_, SourceDeclarationKey>(property)
                        .unwrap()
                ),
                if matches!(declaration, Declaration::Getter(_)) {
                    "get"
                } else {
                    "set"
                }
            )
        }
    }
}

fn name(key: &SourceDeclarationKey) -> String {
    let DeclarationName::Named(name) = key.name() else {
        panic!("named declaration required");
    };
    name.as_str().to_owned()
}
