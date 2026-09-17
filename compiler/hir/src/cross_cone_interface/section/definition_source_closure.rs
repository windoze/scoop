use std::fmt;

use scoop_wire::{BudgetMeter, WireError, WirePath};

use super::CrossConeHirInterfaceSectionV1;
use crate::{
    DefaultBodyOriginSiteV1, ExportDefaultReferenceKindV1, ExportDefaultReferenceV1,
    ExportDefinitionSourceV1, TemplateLocalDefinitionV1,
};

impl CrossConeHirInterfaceSectionV1 {
    /// Proves that field 9 is exactly the canonical deduplicated set of every
    /// inline definition source in fields 1 through 8.
    pub fn validate_definition_source_closure(
        &self,
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<(), ExportDefinitionSourceClosureValidationError> {
        let mut validator = DefinitionSourceClosureValidator::new(
            self.definition_sources().sources(),
            meter,
            path,
        )?;

        for (alias_index, alias) in self.type_aliases().records().iter().enumerate() {
            validator.observe(
                alias.definition_origin(),
                ExportDefinitionSourceUseSiteV1::TypeAlias { alias_index },
            );
        }

        for (source_index, source) in self.source_interfaces().records().iter().enumerate() {
            for (parameter_index, parameter) in source.parameters().parameters().iter().enumerate()
            {
                validator.observe(
                    parameter.definition_origin(),
                    ExportDefinitionSourceUseSiteV1::CallableSourceParameter {
                        source_index,
                        parameter_index,
                    },
                );
            }
        }

        for (wire_index, (template_index, template)) in
            (0_u64..).zip(self.default_templates().records().iter().enumerate())
        {
            validator.observe(
                template.definition_origin(),
                ExportDefinitionSourceUseSiteV1::DefaultTemplateRoot { template_index },
            );

            for (local_index, local) in template.locals().records().iter().enumerate() {
                if let TemplateLocalDefinitionV1::Source(source) = local.definition() {
                    validator.observe(
                        source,
                        ExportDefinitionSourceUseSiteV1::DefaultTemplateLocal {
                            template_index,
                            local_index,
                        },
                    );
                }
            }

            template
                .body()
                .visit_definition_sources(
                    &mut |source, site| {
                        validator.observe(
                            source,
                            ExportDefinitionSourceUseSiteV1::DefaultTemplateBody {
                                template_index,
                                site,
                            },
                        );
                    },
                    meter,
                    &path.clone().field(7).index(wire_index).field(5),
                )
                .map_err(ExportDefinitionSourceClosureValidationError::Resource)?;

            let references = template.references();
            observe_references(
                &mut validator,
                template_index,
                ExportDefaultReferenceKindV1::Callable,
                references.callables(),
            );
            observe_references(
                &mut validator,
                template_index,
                ExportDefaultReferenceKindV1::Constructor,
                references.constructors(),
            );
            observe_references(
                &mut validator,
                template_index,
                ExportDefaultReferenceKindV1::Type,
                references.types(),
            );
            observe_references(
                &mut validator,
                template_index,
                ExportDefaultReferenceKindV1::Global,
                references.globals(),
            );
            observe_references(
                &mut validator,
                template_index,
                ExportDefaultReferenceKindV1::Singleton,
                references.singleton_values(),
            );
            observe_references(
                &mut validator,
                template_index,
                ExportDefaultReferenceKindV1::Field,
                references.fields(),
            );
        }

        for (constant_index, constant) in self.constants().records().iter().enumerate() {
            validator.observe(
                constant.definition_origin(),
                ExportDefinitionSourceUseSiteV1::Constant { constant_index },
            );
        }

        validator.finish()
    }
}

fn observe_references<T>(
    validator: &mut DefinitionSourceClosureValidator<'_>,
    template_index: usize,
    kind: ExportDefaultReferenceKindV1,
    references: &[ExportDefaultReferenceV1<T>],
) {
    for (reference_index, reference) in references.iter().enumerate() {
        validator.observe(
            reference.definition_origin(),
            ExportDefinitionSourceUseSiteV1::DefaultTemplateReference {
                template_index,
                kind,
                reference_index,
            },
        );
    }
}

struct DefinitionSourceClosureValidator<'a> {
    declared: &'a [ExportDefinitionSourceV1],
    seen: Vec<bool>,
    missing: Option<MissingDefinitionSource>,
}

impl<'a> DefinitionSourceClosureValidator<'a> {
    fn new(
        declared: &'a [ExportDefinitionSourceV1],
        meter: &mut BudgetMeter,
        path: &WirePath,
    ) -> Result<Self, ExportDefinitionSourceClosureValidationError> {
        let mut seen = Vec::new();
        meter
            .try_reserve_collection_slots(&mut seen, declared.len(), &path.clone().field(9))
            .map_err(ExportDefinitionSourceClosureValidationError::Resource)?;
        seen.resize(declared.len(), false);
        Ok(Self {
            declared,
            seen,
            missing: None,
        })
    }

    fn observe(
        &mut self,
        source: &ExportDefinitionSourceV1,
        site: ExportDefinitionSourceUseSiteV1,
    ) {
        match self.declared.binary_search(source) {
            Ok(index) => self.seen[index] = true,
            Err(insertion_index) if self.missing.is_none() => {
                self.missing = Some(MissingDefinitionSource {
                    source: Box::new(source.clone()),
                    insertion_index,
                    site,
                });
            }
            Err(_) => {}
        }
    }

    fn finish(self) -> Result<(), ExportDefinitionSourceClosureValidationError> {
        if let Some(missing) = self.missing {
            return Err(ExportDefinitionSourceClosureValidationError::Missing {
                source: missing.source,
                insertion_index: missing.insertion_index,
                site: missing.site,
            });
        }
        if let Some(index) = self.seen.iter().position(|seen| !seen) {
            return Err(ExportDefinitionSourceClosureValidationError::Extra {
                index,
                source: Box::new(self.declared[index].clone()),
            });
        }
        Ok(())
    }
}

struct MissingDefinitionSource {
    source: Box<ExportDefinitionSourceV1>,
    insertion_index: usize,
    site: ExportDefinitionSourceUseSiteV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExportDefinitionSourceUseSiteV1 {
    TypeAlias {
        alias_index: usize,
    },
    CallableSourceParameter {
        source_index: usize,
        parameter_index: usize,
    },
    DefaultTemplateRoot {
        template_index: usize,
    },
    DefaultTemplateLocal {
        template_index: usize,
        local_index: usize,
    },
    DefaultTemplateBody {
        template_index: usize,
        site: DefaultBodyOriginSiteV1,
    },
    DefaultTemplateReference {
        template_index: usize,
        kind: ExportDefaultReferenceKindV1,
        reference_index: usize,
    },
    Constant {
        constant_index: usize,
    },
}

#[derive(Debug, Eq, PartialEq)]
pub enum ExportDefinitionSourceClosureValidationError {
    Missing {
        source: Box<ExportDefinitionSourceV1>,
        insertion_index: usize,
        site: ExportDefinitionSourceUseSiteV1,
    },
    Extra {
        index: usize,
        source: Box<ExportDefinitionSourceV1>,
    },
    Resource(WireError),
}

impl fmt::Display for ExportDefinitionSourceClosureValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing {
                insertion_index,
                site,
                ..
            } => write!(
                formatter,
                "definition source used at {site:?} is absent from field 9 at canonical insertion index {insertion_index}"
            ),
            Self::Extra { index, .. } => write!(
                formatter,
                "definition source at field 9 index {index} has no use in fields 1 through 8"
            ),
            Self::Resource(error) => {
                write!(
                    formatter,
                    "definition source closure resource failure: {error}"
                )
            }
        }
    }
}

impl std::error::Error for ExportDefinitionSourceClosureValidationError {}
