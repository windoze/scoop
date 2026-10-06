use super::*;
use scoop_identity::{PersistentPropertyId, SignatureTypeKey};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StaticParameterTarget {
    Plain,
    Field(StaticFieldIdentity),
    Property(PersistentPropertyId),
}

#[derive(Clone, Copy)]
pub enum StaticDefaultReference<'a> {
    Current {
        id: ExportDefaultSourceId,
        source: &'a ExportDefaultSource,
    },
    Dependency {
        key: ExportDefaultTemplateKeyV1,
        template: &'a ExportDefaultTemplateV1,
    },
}

#[derive(Clone, Copy)]
pub struct StaticConstructorParameter<'a> {
    pub(super) constructor: StaticConstructorShape<'a>,
    pub(super) index: usize,
}

impl<'a> StaticConstructorParameter<'a> {
    pub fn name(self) -> &'a str {
        match self.constructor.source {
            ParameterSource::Current(source) => &source.parameters[self.index].name,
            ParameterSource::Dependency { parameters, .. } => parameters.parameters().parameters()
                [self.index]
                .name()
                .as_str(),
        }
    }

    pub fn value_type(
        self,
        binders: &[HirSignatureBinder],
    ) -> Result<SignatureTypeKey, HirSignatureTypeMappingError> {
        let owner = self.constructor.owner;
        match self.constructor.source {
            ParameterSource::Current(source) => {
                let ty = match source.parameters[self.index].calling {
                    ExportParameterCalling::Required { value_type }
                    | ExportParameterCalling::Default { value_type, .. } => value_type,
                    ExportParameterCalling::Vararg { parameter_type, .. } => {
                        owner.module.export_vararg_parameter_types[parameter_type].array_type
                    }
                };
                owner.type_use(ty).signature(owner.module, binders)
            }
            ParameterSource::Dependency { parameters, .. } => owner.substitute_signature(
                parameters.parameters().parameters()[self.index].value_type(),
                binders,
            ),
        }
    }

    pub fn target(self) -> StaticParameterTarget {
        let owner = self.constructor.owner;
        match self.constructor.targets {
            ParameterTargets::Struct => StaticParameterTarget::Field(
                owner
                    .fields()
                    .nth(self.index)
                    .expect("struct primary parameters match declared fields")
                    .identity(),
            ),
            ParameterTargets::Variant(variant) => StaticParameterTarget::Field(
                variant
                    .fields()
                    .nth(self.index)
                    .expect("variant constructor parameters match payload fields")
                    .identity(),
            ),
            ParameterTargets::ImportedClass(primary) => primary.properties()[self.index].map_or(
                StaticParameterTarget::Plain,
                StaticParameterTarget::Property,
            ),
            ParameterTargets::Class(constructor) => {
                let parameter = constructor.parameters[self.index].id;
                owner.module.classes[constructor.owner]
                    .fields
                    .iter()
                    .find_map(|field| {
                        let field = &owner.module.class_fields[*field];
                        if field.source != ClassFieldSource::PrimaryParameter(parameter) {
                            return None;
                        }
                        Some(StaticParameterTarget::Property(
                            owner.module.property_identities[field.property]
                                .ordinary_id()
                                .expect("primary parameters refer to ordinary properties"),
                        ))
                    })
                    .unwrap_or(StaticParameterTarget::Plain)
            }
        }
    }

    pub fn is_vararg(self) -> bool {
        match self.constructor.source {
            ParameterSource::Current(source) => matches!(
                source.parameters[self.index].calling,
                ExportParameterCalling::Vararg { .. }
            ),
            ParameterSource::Dependency { parameters, .. } => parameters.parameters().parameters()
                [self.index]
                .calling()
                .is_vararg(),
        }
    }

    pub fn default(self) -> Option<StaticDefaultReference<'a>> {
        match self.constructor.source {
            ParameterSource::Current(source) => {
                let source = match source.parameters[self.index].calling {
                    ExportParameterCalling::Default { source, .. }
                    | ExportParameterCalling::Vararg {
                        omission: ExportVarargOmission::Default(source),
                        ..
                    } => source,
                    ExportParameterCalling::Required { .. }
                    | ExportParameterCalling::Vararg {
                        omission: ExportVarargOmission::EmptyArray,
                        ..
                    } => return None,
                };
                Some(StaticDefaultReference::Current {
                    id: source,
                    source: &self.constructor.owner.module.export_default_sources[source],
                })
            }
            ParameterSource::Dependency {
                parameters,
                interface,
            } => {
                let key = parameters.parameters().parameters()[self.index]
                    .calling()
                    .template()?;
                let template = interface
                    .default_templates()
                    .get(key)
                    .expect("the source parameter refers to its original default template");
                Some(StaticDefaultReference::Dependency { key, template })
            }
        }
    }
}
