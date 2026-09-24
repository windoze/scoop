use super::*;

pub(super) fn render(checked: CheckedSharedTypeFoundationV1<'_>) -> String {
    let mut output = format!(
        "protected declarations {}\n",
        checked.section().protected_declarations().records().len()
    );
    for record in checked.section().protected_declarations().records() {
        output.push_str(&format!("{:?}\n", record.reference()));
        match record {
            ProtectedDeclarationInterfaceV1::Callable(callable) => output.push_str(&format!(
                "  owner {:?} parameters {} slots {}\n",
                callable.payload().owner(),
                callable.payload().parameters().parameters().len(),
                callable.payload().slot_relations().slots().len(),
            )),
            ProtectedDeclarationInterfaceV1::Constructor(constructor) => output.push_str(&format!(
                "  owner {:?} parameters {}\n",
                constructor.payload().owner(),
                constructor.payload().parameters().parameters().len(),
            )),
            ProtectedDeclarationInterfaceV1::Property(property) => output.push_str(&format!(
                "  type {:?} representation {:?} mutability {:?}\n",
                property.payload().value_type(),
                property.payload().representation(),
                property.payload().mutability(),
            )),
            ProtectedDeclarationInterfaceV1::NestedNominal(nested) => {
                source(&mut output, nested.payload().source_interface(), 1)
            }
        }
    }
    output
}

fn source(output: &mut String, value: &hir::ProtectedNestedSourceInterfaceV1, depth: usize) {
    let indent = "  ".repeat(depth);
    output.push_str(&format!(
        "{indent}{:?} {:?} binders {} constructors {} members {} children {}\n",
        value.kind(),
        value.modality(),
        value.type_parameters().len_u32(),
        value.constructors().values().len(),
        value.members().values().len(),
        value.children().values().len()
    ));
    for record in value.source_support().records() {
        output.push_str(&format!(
            "{indent}support {:?} {:?}\n",
            record.declaration(),
            record.declaration_access().declared_visibility()
        ));
        if let hir::NestedSourceSupportV1::NestedNominal(nested) = record {
            source(output, nested.payload().source_interface(), depth + 1);
        }
    }
}
