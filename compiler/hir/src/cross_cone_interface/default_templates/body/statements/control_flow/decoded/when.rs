use super::*;
use crate::ExhaustivenessProof;
use scoop_identity::SignatureTypeKey;
use std::convert::Infallible;

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
        match self.subject {
            DecodedOptionalDefaultExpressionV1::Present(subject) => {
                let subject = subject.resolve(resolver, locals).map_err(|error| {
                    DefaultControlFlowResolutionError::Expression {
                        context: "when subject",
                        error,
                    }
                })?;
                let mut arms = Vec::with_capacity(self.arms.len());
                for (index, arm) in self.arms.into_iter().enumerate() {
                    arms.push(arm.resolve(
                        resolver,
                        locals,
                        index,
                        |pattern, resolver, locals| {
                            pattern.resolve(resolver, locals).map_err(|error| {
                                DefaultControlFlowResolutionError::Pattern {
                                    arm_index: index,
                                    error,
                                }
                            })
                        },
                    )?);
                }
                let fallback =
                    self.fallback
                        .resolve_subject(resolver, locals, subject.result_type())?;
                DefaultWhenV1::try_subject(subject, arms, fallback)
                    .map_err(DefaultControlFlowResolutionError::Shape)
            }
            DecodedOptionalDefaultExpressionV1::Absent => {
                let mut arms = Vec::with_capacity(self.arms.len());
                for (index, arm) in self.arms.into_iter().enumerate() {
                    arms.push(arm.resolve(
                        resolver,
                        locals,
                        index,
                        |_, _, _| -> Result<Infallible, _> {
                            Err(DefaultControlFlowResolutionError::Shape(
                                DefaultControlFlowBuildError::MissingWhenSubject,
                            ))
                        },
                    )?);
                }
                let fallback = self.fallback.resolve_conditions(resolver, locals)?;
                DefaultWhenV1::try_conditions(arms, fallback)
                    .map_err(DefaultControlFlowResolutionError::Shape)
            }
        }
    }
}

impl DecodedDefaultWhenArmV1 {
    fn resolve<R, L, E, P>(
        self,
        resolver: &mut R,
        locals: &mut L,
        arm_index: usize,
        resolve_pattern: impl FnOnce(
            crate::DecodedDefaultPatternV1,
            &mut R,
            &mut L,
        )
            -> Result<P, DefaultControlFlowResolutionError<E, L::Error>>,
    ) -> Result<DefaultWhenArmV1<P>, DefaultControlFlowResolutionError<E, L::Error>>
    where
        R: DefaultStatementReferenceResolver<E>,
        L: TemplateLocalSelectorResolver,
    {
        require_statement_count(self.body.len())?;
        let condition = match self.condition {
            DecodedDefaultWhenConditionV1::Case(pattern) => {
                DefaultWhenConditionV1::Case(resolve_pattern(pattern, resolver, locals)?)
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
    fn resolve_subject<R, L, E>(
        self,
        resolver: &mut R,
        locals: &mut L,
        expected: &SignatureTypeKey,
    ) -> Result<DefaultWhenFallbackV1, DefaultControlFlowResolutionError<E, L::Error>>
    where
        R: DefaultStatementReferenceResolver<E>,
        L: TemplateLocalSelectorResolver,
    {
        let (proof, subject) = match self {
            Self::Fallthrough => return Ok(DefaultWhenFallbackV1::Fallthrough),
            Self::Else(statements) => return resolve_else(statements, resolver, locals),
            Self::IrrefutableArm { subject_type } => (
                ExhaustivenessProof::IrrefutableArm,
                subject_type.resolve(resolver).map_err(|error| {
                    DefaultControlFlowResolutionError::Type {
                        context: "when irrefutable subject",
                        error,
                    }
                })?,
            ),
            Self::PatternMatrix { subject_type } => (
                ExhaustivenessProof::PatternMatrix,
                subject_type.resolve(resolver).map_err(|error| {
                    DefaultControlFlowResolutionError::Type {
                        context: "when pattern-matrix subject",
                        error,
                    }
                })?,
            ),
            Self::EnumPatternMatrix {
                subject_type,
                owner_type,
            } => {
                let subject = subject_type.resolve(resolver).map_err(|error| {
                    DefaultControlFlowResolutionError::Type {
                        context: "when enum-pattern subject",
                        error,
                    }
                })?;
                let owner = owner_type.resolve(resolver).map_err(|error| {
                    DefaultControlFlowResolutionError::Type {
                        context: "when enum-pattern owner",
                        error,
                    }
                })?;
                if &owner != expected {
                    return Err(DefaultControlFlowResolutionError::Shape(
                        DefaultControlFlowBuildError::WhenSubjectTypeMismatch,
                    ));
                }
                (ExhaustivenessProof::EnumPatternMatrix, subject)
            }
        };
        if &subject != expected {
            return Err(DefaultControlFlowResolutionError::Shape(
                DefaultControlFlowBuildError::WhenSubjectTypeMismatch,
            ));
        }
        Ok(DefaultWhenFallbackV1::Impossible(proof))
    }

    fn resolve_conditions<R, L, E>(
        self,
        resolver: &mut R,
        locals: &mut L,
    ) -> Result<DefaultWhenFallbackV1<Infallible>, DefaultControlFlowResolutionError<E, L::Error>>
    where
        R: DefaultStatementReferenceResolver<E>,
        L: TemplateLocalSelectorResolver,
    {
        match self {
            Self::Fallthrough => Ok(DefaultWhenFallbackV1::Fallthrough),
            Self::Else(statements) => resolve_else(statements, resolver, locals),
            Self::IrrefutableArm { .. }
            | Self::PatternMatrix { .. }
            | Self::EnumPatternMatrix { .. } => Err(DefaultControlFlowResolutionError::Shape(
                DefaultControlFlowBuildError::MissingWhenSubject,
            )),
        }
    }
}

fn resolve_else<R, L, E, P>(
    statements: Vec<DecodedDefaultStatementV1>,
    resolver: &mut R,
    locals: &mut L,
) -> Result<DefaultWhenFallbackV1<P>, DefaultControlFlowResolutionError<E, L::Error>>
where
    R: DefaultStatementReferenceResolver<E>,
    L: TemplateLocalSelectorResolver,
{
    require_statement_count(statements.len())?;
    let statements = resolve_statements(statements, resolver, locals, StatementContext::WhenElse)?;
    DefaultWhenFallbackV1::try_else(statements).map_err(DefaultControlFlowResolutionError::Shape)
}

#[cfg(test)]
mod tests;
