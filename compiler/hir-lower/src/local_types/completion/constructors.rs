use super::*;

impl Completion {
    fn arguments(&mut self, state: &mut Lowerer, value: &mut hir::ConstructorArguments) {
        self.region(
            state,
            &mut value.locals,
            &mut value.statements,
            &mut value.args,
        );
    }

    fn base(&mut self, state: &mut Lowerer, base: &mut hir::BaseInitialization) {
        if let hir::BaseInitialization::Super { arguments, .. } = base {
            self.arguments(state, arguments);
        }
    }

    fn initialization(&mut self, state: &mut Lowerer, steps: &mut [hir::ClassInitializationStep]) {
        for step in steps {
            match step {
                hir::ClassInitializationStep::Field { initializer, .. } => self.region(
                    state,
                    &mut initializer.locals,
                    &mut initializer.statements,
                    std::slice::from_mut(&mut initializer.value),
                ),
                hir::ClassInitializationStep::InitBlock { body, .. } => self.body(state, body),
            }
        }
    }

    pub(super) fn constructor(
        &mut self,
        state: &mut Lowerer,
        value: &mut hir::ClassConstructorKind,
    ) {
        match value {
            hir::ClassConstructorKind::Primary {
                base,
                common_initialization,
                ..
            } => {
                self.base(state, base);
                self.initialization(state, common_initialization);
            }
            hir::ClassConstructorKind::Secondary { delegation, body } => {
                match delegation {
                    hir::ClassSecondaryDelegation::This { arguments, .. } => {
                        self.arguments(state, arguments)
                    }
                    hir::ClassSecondaryDelegation::Terminal {
                        base,
                        common_initialization,
                    } => {
                        self.base(state, base);
                        self.initialization(state, common_initialization);
                    }
                }
                self.body(state, body);
            }
        }
    }
}
