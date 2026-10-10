use super::*;

impl DecodedDefaultWhenV1 {
    pub fn resolve<R, L, E>(
        self,
        resolver: &mut R,
        locals: &mut L,
    ) -> Result<DefaultWhenV1, DefaultControlFlowResolutionError<E, L::Error>>
    where
        R: DefaultStatementReferenceResolver<E>,
        L: TemplateLocalSelectorResolver,
    {
        require_count(
            self.arms.len(),
            DefaultControlFlowBuildError::TooManyWhenArms,
        )?;
        let subject = match self.subject {
            DecodedOptionalDefaultExpressionV1::Absent => OptionalDefaultExpressionV1::Absent,
            DecodedOptionalDefaultExpressionV1::Present(subject) => {
                OptionalDefaultExpressionV1::present(subject.resolve(resolver, locals).map_err(
                    |error| DefaultControlFlowResolutionError::Expression {
                        context: "when subject",
                        error,
                    },
                )?)
            }
        };
        let mut arms = Vec::with_capacity(self.arms.len());
        for (index, arm) in self.arms.into_iter().enumerate() {
            arms.push(arm.resolve(resolver, locals, index)?);
        }
        DefaultWhenV1::try_new(subject, arms, self.fallback.resolve(resolver, locals)?)
            .map_err(DefaultControlFlowResolutionError::Shape)
    }
}

impl DecodedDefaultWhenArmV1 {
    fn resolve<R, L, E>(
        self,
        resolver: &mut R,
        locals: &mut L,
        arm_index: usize,
    ) -> Result<DefaultWhenArmV1, DefaultControlFlowResolutionError<E, L::Error>>
    where
        R: DefaultStatementReferenceResolver<E>,
        L: TemplateLocalSelectorResolver,
    {
        require_statement_count(self.body.len())?;
        let condition = match self.condition {
            DecodedDefaultWhenConditionV1::Case(pattern) => {
                DefaultWhenConditionV1::Case(pattern.resolve(resolver, locals).map_err(
                    |error| DefaultControlFlowResolutionError::Pattern { arm_index, error },
                )?)
            }
            DecodedDefaultWhenConditionV1::Predicate(predicate) => {
                DefaultWhenConditionV1::Predicate(Box::new(
                    predicate.resolve(resolver, locals, arm_index)?,
                ))
            }
            DecodedDefaultWhenConditionV1::Always => DefaultWhenConditionV1::Always,
        };
        let guard = self.guard.resolve(resolver, locals, arm_index)?;
        let body = resolve_statements(
            self.body,
            resolver,
            locals,
            StatementContext::WhenArm { arm_index },
        )?;
        let definition_origin = self.definition_origin.resolve(resolver).map_err(|error| {
            DefaultControlFlowResolutionError::DefinitionOrigin {
                context: OriginContext::WhenArm { arm_index },
                error,
            }
        })?;
        DefaultWhenArmV1::try_new(condition, guard, body, definition_origin)
            .map_err(DefaultControlFlowResolutionError::Shape)
    }
}

impl DecodedOptionalDefaultWhenGuardV1 {
    fn resolve<R, L, E>(
        self,
        resolver: &mut R,
        locals: &mut L,
        arm_index: usize,
    ) -> Result<OptionalDefaultWhenGuardV1, DefaultControlFlowResolutionError<E, L::Error>>
    where
        R: DefaultStatementReferenceResolver<E>,
        L: TemplateLocalSelectorResolver,
    {
        match self {
            Self::Absent => Ok(OptionalDefaultWhenGuardV1::absent()),
            Self::Present(guard) => (*guard)
                .resolve(resolver, locals, arm_index)
                .map(OptionalDefaultWhenGuardV1::present),
        }
    }
}

impl DecodedDefaultWhenGuardV1 {
    fn resolve<R, L, E>(
        self,
        resolver: &mut R,
        locals: &mut L,
        arm_index: usize,
    ) -> Result<DefaultWhenGuardV1, DefaultControlFlowResolutionError<E, L::Error>>
    where
        R: DefaultStatementReferenceResolver<E>,
        L: TemplateLocalSelectorResolver,
    {
        require_statement_count(self.setup.len())?;
        let setup = resolve_statements(
            self.setup,
            resolver,
            locals,
            StatementContext::WhenGuard { arm_index },
        )?;
        let condition = self.condition.resolve(resolver, locals).map_err(|error| {
            DefaultControlFlowResolutionError::Expression {
                context: "when guard condition",
                error,
            }
        })?;
        DefaultWhenGuardV1::try_new(setup, condition)
            .map_err(DefaultControlFlowResolutionError::Shape)
    }
}

impl DecodedDefaultWhenFallbackV1 {
    fn resolve<R, L, E>(
        self,
        resolver: &mut R,
        locals: &mut L,
    ) -> Result<DefaultWhenFallbackV1, DefaultControlFlowResolutionError<E, L::Error>>
    where
        R: DefaultStatementReferenceResolver<E>,
        L: TemplateLocalSelectorResolver,
    {
        match self {
            Self::Fallthrough => Ok(DefaultWhenFallbackV1::fallthrough()),
            Self::Else(statements) => {
                require_statement_count(statements.len())?;
                let statements =
                    resolve_statements(statements, resolver, locals, StatementContext::WhenElse)?;
                DefaultWhenFallbackV1::try_else(statements)
                    .map_err(DefaultControlFlowResolutionError::Shape)
            }
            Self::IrrefutableArm { subject_type } => subject_type
                .resolve(resolver)
                .map(DefaultWhenFallbackV1::irrefutable_arm)
                .map_err(|error| DefaultControlFlowResolutionError::Type {
                    context: "when irrefutable subject",
                    error,
                }),
            Self::PatternMatrix { subject_type } => subject_type
                .resolve(resolver)
                .map(DefaultWhenFallbackV1::pattern_matrix)
                .map_err(|error| DefaultControlFlowResolutionError::Type {
                    context: "when pattern-matrix subject",
                    error,
                }),
            Self::EnumPatternMatrix {
                subject_type,
                owner_type,
            } => Ok(DefaultWhenFallbackV1::enum_pattern_matrix(
                subject_type.resolve(resolver).map_err(|error| {
                    DefaultControlFlowResolutionError::Type {
                        context: "when enum-pattern subject",
                        error,
                    }
                })?,
                owner_type.resolve(resolver).map_err(|error| {
                    DefaultControlFlowResolutionError::Type {
                        context: "when enum-pattern owner",
                        error,
                    }
                })?,
            )),
        }
    }
}
