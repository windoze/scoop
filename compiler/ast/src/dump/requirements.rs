use super::{dump_annotations, dump_type_ref, expressions::dump_call_type_args};
use crate::*;

pub(super) fn dump_requirements(block: &MemberRequirements) -> String {
    let parameters = block
        .parameters
        .iter()
        .map(|parameter| format!("{}: {}", parameter.name.text, dump_type_ref(&parameter.ty)))
        .collect::<Vec<_>>()
        .join(", ");
    let calls = block
        .calls
        .iter()
        .map(|requirement| {
            let mut effects = String::new();
            dump_annotations(&requirement.annotations, 0, &mut effects);
            let effects = effects.replace('\n', " ");
            let suspend = if requirement.suspend_span.is_some() {
                "suspend "
            } else {
                ""
            };
            let arguments = requirement
                .arguments
                .iter()
                .map(|argument| {
                    let name = argument
                        .name
                        .as_ref()
                        .map_or_else(String::new, |name| format!("{} = ", name.text));
                    let spread = if matches!(argument.spread, SpreadSyntax::Spread(_)) {
                        "*"
                    } else {
                        ""
                    };
                    let value = match &argument.value {
                        RequirementArgumentValue::Parameter(name) => name.text.clone(),
                        RequirementArgumentValue::Inline { name, ty } => {
                            format!("{}: {}", name.text, dump_type_ref(ty))
                        }
                    };
                    format!("{name}{spread}{value}")
                })
                .collect::<Vec<_>>()
                .join(", ");
            let call = match &requirement.call {
                RequiredCall::Method {
                    receiver,
                    companion,
                    name,
                    type_arguments,
                    result,
                } => format!(
                    "{}{}.{}{}({arguments}): {}",
                    receiver.text,
                    if *companion { ".Companion" } else { "" },
                    name.text,
                    dump_call_type_args(type_arguments),
                    dump_type_ref(result)
                ),
                RequiredCall::Constructor { parameter, result } => format!(
                    "{}({arguments}){}",
                    parameter.text,
                    result
                        .as_ref()
                        .map_or_else(String::new, |ty| format!(": {}", dump_type_ref(ty)))
                ),
            };
            format!("{effects}{suspend}{call}")
        })
        .collect::<Vec<_>>()
        .join("; ");
    format!("requires({parameters}) {{ {calls} }}")
}
