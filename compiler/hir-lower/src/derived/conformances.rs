use super::*;

impl Lowerer {
    pub(crate) fn prepare_value_equality_conformance(
        &mut self,
        owner: Owner,
        span: ast::Span,
        file: usize,
    ) {
        let (function, generic) = match owner {
            Owner::Struct(id) => (
                self.structs[id].derived_equality,
                !self.structs[id].type_params.is_empty(),
            ),
            Owner::Enum(id) => (
                self.enums[id].derived_equality,
                !self.enums[id].type_params.is_empty(),
            ),
            _ => return,
        };
        let Some(_) = function else {
            return;
        };
        if generic {
            return;
        }
        let mut candidate = self.clone();
        let previous_file = candidate.current_file;
        let previous_owner = candidate.current_owner;
        candidate.current_file = file;
        candidate.current_owner = Some(owner);
        let ty = candidate.owner_ty(owner);
        if !matches!(candidate.derived_equality_candidate(ty, span), Ok(Some(_))) {
            return;
        }
        let Some(interface) = candidate.equality_interface(ty) else {
            return;
        };
        let interfaces = match owner {
            Owner::Struct(id) => &mut candidate.structs[id].definition.interfaces,
            Owner::Enum(id) => &mut candidate.enums[id].definition.interfaces,
            _ => unreachable!(),
        };
        if !interfaces.contains(&interface) {
            interfaces.push(interface);
        }
        candidate.current_file = previous_file;
        candidate.current_owner = previous_owner;
        *self = candidate;
    }
}
