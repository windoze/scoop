use crate::{ContextParameter, ContextParameterLabel};

pub(super) fn dump_context(parameters: &[ContextParameter], indent: usize, out: &mut String) {
    if parameters.is_empty() {
        return;
    }
    out.push_str(&format!("{}context(\n", "  ".repeat(indent)));
    for parameter in parameters {
        let name = match &parameter.label {
            ContextParameterLabel::Named(name) => name.text.as_str(),
            ContextParameterLabel::Unnamed(_) => "_",
        };
        out.push_str(&format!(
            "{}{}: {}\n",
            "  ".repeat(indent + 1),
            name,
            super::dump_type_ref(&parameter.ty)
        ));
    }
    out.push_str(&format!("{})\n", "  ".repeat(indent)));
}
