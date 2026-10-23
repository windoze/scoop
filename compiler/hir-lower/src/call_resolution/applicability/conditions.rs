//! Keep an inherited member's original constraint subjects intact.

use super::*;

impl Lowerer {
    pub(super) fn add_applied_member_condition(
        &mut self,
        session: &mut InferenceSession,
        guard: hir::ConditionalInterfaceGuard,
    ) {
        let application = self.conditional_guard_application(guard);
        let mut parameters = self
            .declaration_conditional_interfaces(application.owner)
            .iter()
            .find(|declaration| declaration.id() == application.declaration)
            .expect("a member retains its original conditional declaration")
            .parameters
            .clone();
        let bindings = parameters
            .iter_mut()
            .enumerate()
            .map(|(index, parameter)| {
                let original = parameter.id;
                parameter.id = self.fresh_type_param(index);
                (original, self.intern_type(hir::Type::Param(parameter.id)))
            })
            .collect::<Vec<_>>();
        self.substitute_parameter_bounds(&mut parameters, &bindings);
        let environment = session.add_environment(&parameters, &[]);
        let variables = session.owner_variables(environment).to_vec();
        for (variable, argument) in variables.into_iter().zip(application.arguments) {
            session.push(
                Constraint::Equal(variable.into(), TypeTerm::Type(argument)),
                ConstraintOrigin::Receiver,
            );
        }
        self.add_declaration_bounds(session, parameters.iter());
    }
}
