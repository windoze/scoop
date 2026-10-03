use super::*;
use crate::{
    CanonicalBooleanV1, CanonicalTemplateLocalTableV1, ConstructorParameter, DefinitionOrigin,
    ExportDefinitionSourceV1, ExportTemplateFragmentV1, HirSignatureBinder,
    TemplateLocalDefinitionV1, TemplateLocalRecordV1,
};
use scoop_identity::{LocalValueSelector, SignatureTypeKey};

pub(super) struct ConstructorProjection<'a, 'hir> {
    pub entities: &'a DefaultEntityProjector<'hir>,
    pub binders: Vec<HirSignatureBinder>,
    pub owner: SignatureTypeKey,
    pub parameters: &'a [ConstructorParameter],
    pub origin: DefinitionOrigin,
    pub inputs: CanonicalTemplateLocalTableV1,
}

impl<'a, 'hir> ConstructorProjection<'a, 'hir> {
    pub fn new(
        entities: &'a DefaultEntityProjector<'hir>,
        parameters: &'a [ConstructorParameter],
        type_parameters: &[crate::TypeParamDecl],
        owner: crate::TypeId,
        origin: DefinitionOrigin,
    ) -> Result<Self, Error> {
        let binders =
            crate::production::signatures::HirInterfaceSignatureProjector::new(entities.export())
                .binder_frame(type_parameters, 0)
                .map_err(Error::Signature)?;
        let owner = entities.type_key(owner, &binders).map_err(Error::Entity)?;
        let mut inputs = Vec::new();
        inputs.push(
            TemplateLocalRecordV1::try_new(
                LocalValueSelector::This,
                owner.clone(),
                CanonicalBooleanV1::from(false),
                TemplateLocalDefinitionV1::Source(definition_source(entities.export(), origin)?),
            )
            .map_err(|source| {
                Error::Locals(Box::new(
                    crate::DefaultTemplateEnvelopeProjectionError::LocalRecord { local: 0, source },
                ))
            })?,
        );
        for (index, parameter) in parameters.iter().enumerate() {
            let index = u32::try_from(index).map_err(|_| {
                Error::Signature(crate::HirInterfaceSignatureProjectionError::TooManyTypeParameters)
            })?;
            let value_type = entities
                .type_key(parameter.ty, &binders)
                .map_err(Error::Entity)?;
            let definition = definition_source(entities.export(), parameter.definition)?;
            inputs.push(
                TemplateLocalRecordV1::try_new(
                    LocalValueSelector::Parameter {
                        declaration_index: index,
                    },
                    value_type,
                    CanonicalBooleanV1::from(false),
                    TemplateLocalDefinitionV1::Source(definition),
                )
                .map_err(|source| {
                    Error::Locals(Box::new(
                        crate::DefaultTemplateEnvelopeProjectionError::LocalRecord {
                            local: index,
                            source,
                        },
                    ))
                })?,
            );
        }
        let inputs = CanonicalTemplateLocalTableV1::try_new(inputs).map_err(|source| {
            Error::Locals(Box::new(
                crate::DefaultTemplateEnvelopeProjectionError::LocalTable(source),
            ))
        })?;
        Ok(Self {
            entities,
            binders,
            owner,
            parameters,
            origin,
            inputs,
        })
    }

    pub fn fragment(
        &self,
        locals: &la_arena::Arena<crate::Local>,
        statements: &[crate::Statement],
        results: &[crate::Expr],
    ) -> Result<ExportTemplateFragmentV1, Error> {
        let (mut projection, local_table) = super::super::locals::TemplateLocalProjection::project(
            self.entities,
            locals,
            &self.binders,
        )
        .map_err(|source| Error::Locals(Box::new(source)))?;
        projection.bind_constructor_inputs(self.parameters, self.owner.clone());
        let mut records = local_table.records().to_vec();
        records.extend(
            self.inputs
                .records()
                .iter()
                .filter(|input| local_table.get(input.selector()).is_none())
                .cloned(),
        );
        let local_table = CanonicalTemplateLocalTableV1::try_new(records).map_err(|source| {
            Error::Locals(Box::new(
                crate::DefaultTemplateEnvelopeProjectionError::LocalTable(source),
            ))
        })?;
        super::super::body::project_fragment(
            self.entities,
            &projection,
            &self.binders,
            self.origin,
            statements,
            results,
            local_table,
        )
        .map_err(|source| Error::Body(Box::new(source)))
    }

    pub fn arguments(
        &self,
        arguments: &crate::ConstructorArguments,
    ) -> Result<ExportTemplateFragmentV1, Error> {
        self.fragment(&arguments.locals, &arguments.statements, &arguments.args)
    }

    pub fn body(&self, body: &crate::Body) -> Result<ExportTemplateFragmentV1, Error> {
        self.fragment(&body.locals, &body.statements, &[])
    }

    pub fn predicates(
        &self,
        no_gc: &[crate::TypeParamId],
        pointees: &[crate::RequiresGcFreePointee],
    ) -> Result<crate::GenericTemplatePredicatesV1, Error> {
        Ok(crate::GenericTemplatePredicatesV1::new(
            super::super::generic::binder_uses(no_gc.iter().copied(), &self.binders)?,
            super::super::generic::binder_uses(
                pointees.iter().map(|requirement| requirement.type_param),
                &self.binders,
            )?,
        ))
    }

    pub fn definition_source(&self) -> Result<ExportDefinitionSourceV1, Error> {
        definition_source(self.entities.export(), self.origin)
    }
}

fn definition_source(
    export: &ExportHir,
    origin: DefinitionOrigin,
) -> Result<ExportDefinitionSourceV1, Error> {
    crate::production::definition_sources::project_definition_source(export, origin).map_err(
        |source| {
            Error::Body(Box::new(
                crate::DefaultBodyProjectionError::DefinitionOrigin(source),
            ))
        },
    )
}
