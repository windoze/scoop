use super::*;

pub(super) fn snapshot(name: &str, selected: &CanonicalSelectedExternalTypeUsesV1) {
    let text = selected
        .records()
        .iter()
        .map(|record| {
            let usage = match record.usage() {
                SelectedTypeUseV1::Representation { exact } => format!("Representation {exact}"),
                SelectedTypeUseV1::Signature { exact } => format!("Signature {exact}"),
                SelectedTypeUseV1::Construct { exact, declaration } => match declaration {
                    SelectedTypeConstructionV1::Constructor(id) => {
                        format!("Construct {exact} Constructor {id}")
                    }
                    SelectedTypeConstructionV1::EnumVariant(id) => {
                        format!("Construct {exact} EnumVariant {id}")
                    }
                },
                SelectedTypeUseV1::MemberCall {
                    receiver,
                    declaration,
                } => match declaration {
                    InheritanceCallableDeclarationV1::Function(id) => {
                        format!("MemberCall {receiver} Function {id}")
                    }
                    InheritanceCallableDeclarationV1::Getter(id) => {
                        format!("MemberCall {receiver} Getter {id}")
                    }
                    InheritanceCallableDeclarationV1::Setter(id) => {
                        format!("MemberCall {receiver} Setter {id}")
                    }
                    InheritanceCallableDeclarationV1::DerivedEquality(owner) => {
                        format!("MemberCall {receiver} DerivedEquality {owner:?}")
                    }
                },
                SelectedTypeUseV1::Inheritance { derived, edge } => match edge {
                    SelectedDirectInheritanceEdgeV1::ClassBase { exact } => {
                        format!("Inheritance {derived} ClassBase {exact}")
                    }
                    SelectedDirectInheritanceEdgeV1::Interface { exact } => {
                        format!("Inheritance {derived} Interface {exact}")
                    }
                },
                _ => panic!("unexpected selected use in the call fixture"),
            };
            format!("{} {usage}\n", record.provider())
        })
        .collect::<String>();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../tests/fixtures/m23-shared-type-uses/{name}.snap"
    ));
    if std::env::var_os("SCOOP_UPDATE_SHARED_TYPE_USES").is_some() {
        std::fs::write(&path, &text).unwrap();
    }
    assert_eq!(text, std::fs::read_to_string(path).unwrap());
}
