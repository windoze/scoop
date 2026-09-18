//! Recursive projection of one typed HIR default body.

use scoop_identity::{LocalValueSelector, SignatureTypeKey};

use super::{entities::DefaultEntityProjector, locals::TemplateLocalProjection};
use crate::{
    DefaultExpressionV1, DefaultStatementV1, ExportDefaultBodyV1, HirSignatureBinder, Span, TypeId,
};

mod bindings;
mod expressions;
mod nested;
mod patterns;
mod statements;

pub(super) fn project(
    entities: &DefaultEntityProjector<'_, '_>,
    locals: &TemplateLocalProjection,
    binders: &[HirSignatureBinder],
    template_origin: crate::DefinitionOrigin,
    statements: &[crate::Statement],
    value: &crate::Expr,
) -> Result<ExportDefaultBodyV1, super::DefaultBodyProjectionError> {
    let mut projection = BodyProjection {
        entities,
        locals,
        binders,
        template_origin,
        loops: Vec::new(),
    };
    let statements = projection.statements(statements)?;
    let value = projection.expression(value)?;
    ExportDefaultBodyV1::try_new(statements, value).map_err(super::DefaultBodyProjectionError::Body)
}

pub(super) struct BodyProjection<'a, 'core> {
    entities: &'a DefaultEntityProjector<'a, 'core>,
    locals: &'a TemplateLocalProjection,
    binders: &'a [HirSignatureBinder],
    template_origin: crate::DefinitionOrigin,
    loops: Vec<crate::LoopId>,
}

impl BodyProjection<'_, '_> {
    pub(super) fn type_key(
        &self,
        ty: TypeId,
    ) -> Result<SignatureTypeKey, super::DefaultBodyProjectionError> {
        self.entities.type_key(ty, self.binders).map_err(Into::into)
    }

    pub(super) fn function_type(
        &self,
        id: crate::FunctionTypeId,
    ) -> Result<SignatureTypeKey, super::DefaultBodyProjectionError> {
        let function_type = super::arena_get(&self.entities.export().function_types, id).ok_or(
            super::DefaultBodyProjectionError::Entity(
                super::DefaultEntityProjectionError::Unknown {
                    kind: "function type",
                    index: super::raw_index(id),
                },
            ),
        )?;
        self.type_key(function_type.canonical_type)
    }

    pub(super) fn local(
        &self,
        id: crate::LocalId,
    ) -> Result<LocalValueSelector, super::DefaultBodyProjectionError> {
        self.locals.selector(id)
    }

    pub(super) fn origin(
        &self,
        origin: crate::DefinitionOrigin,
    ) -> Result<crate::ExportDefinitionSourceV1, super::DefaultBodyProjectionError> {
        super::super::definition_sources::project_definition_source(self.entities.export(), origin)
            .map_err(Into::into)
    }

    pub(super) fn span_origin(
        &self,
        span: Span,
    ) -> Result<crate::ExportDefinitionSourceV1, super::DefaultBodyProjectionError> {
        self.origin(crate::DefinitionOrigin {
            span,
            ..self.template_origin
        })
    }

    pub(super) fn statements(
        &mut self,
        statements: &[crate::Statement],
    ) -> Result<Vec<DefaultStatementV1>, super::DefaultBodyProjectionError> {
        statements
            .iter()
            .map(|statement| self.statement(statement))
            .collect()
    }

    pub(super) fn expressions(
        &mut self,
        expressions: &[crate::Expr],
    ) -> Result<Vec<DefaultExpressionV1>, super::DefaultBodyProjectionError> {
        expressions
            .iter()
            .map(|expression| self.expression(expression))
            .collect()
    }
}
