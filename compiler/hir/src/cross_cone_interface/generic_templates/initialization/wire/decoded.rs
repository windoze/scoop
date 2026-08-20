use super::release::DecodedReleaseTemplate;
use super::*;
use crate::{
    BinderUseListValidationError, CallableSourceEffectsBuildError, DecodedCallableSourceEffectsV1,
    DecodedCanonicalTemplateLocalTableV1, DecodedDefaultConstructorRefV1, DecodedDefaultFieldRefV1,
    DecodedExportDefinitionSourceV1, DecodedExportTemplateFragmentV1,
    DecodedGenericTemplatePredicatesV1, DefaultConstructorRefResolutionError,
    DefaultFieldRefResolutionError, DefaultStatementReferenceResolver,
    TemplateFragmentResolutionError, TemplateLocalTableValidationError,
};
use scoop_identity::{DecodedPersistentId, SourceOriginResolutionError};

mod codec;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DecodedCanonicalExportGenericInitializationsV1 {
    records: Vec<DecodedNominalInitialization>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DecodedNominalInitialization {
    owner: DecodedPersistentId<PersistentGenericTypeId>,
    common: Vec<DecodedCommonStep>,
    constructors: Vec<DecodedConstructor>,
    release_policy: crate::ReleasePolicy<DecodedReleaseTemplate>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum DecodedCommonStep {
    Field(DecodedDefaultFieldRefV1, DecodedExportTemplateFragmentV1),
    Body(DecodedExportTemplateFragmentV1),
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DecodedDelegation {
    target: DecodedDefaultConstructorRefV1,
    arguments: DecodedExportTemplateFragmentV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DecodedPrimaryStore {
    field: DecodedDefaultFieldRefV1,
    parameter: LocalValueSelector,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DecodedConstructor {
    declaration: DecodedDefaultConstructorRefV1,
    inputs: DecodedCanonicalTemplateLocalTableV1,
    effects: DecodedCallableSourceEffectsV1,
    predicates: DecodedGenericTemplatePredicatesV1,
    definition_origin: DecodedExportDefinitionSourceV1,
    kind: DecodedConstructorKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum DecodedConstructorKind {
    StructPrimary,
    StructSecondary(DecodedDelegation, DecodedExportTemplateFragmentV1),
    ClassPrimary(Option<DecodedDelegation>, Vec<DecodedPrimaryStore>),
    ClassSecondaryThis(DecodedDelegation, DecodedExportTemplateFragmentV1),
    ClassSecondaryTerminal(Option<DecodedDelegation>, DecodedExportTemplateFragmentV1),
}

impl DecodedCanonicalExportGenericInitializationsV1 {
    pub fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<CanonicalExportGenericInitializationsV1, GenericInitializationResolutionError<E>>
    where
        R: DefaultStatementReferenceResolver<E>,
    {
        let records = self
            .records
            .into_iter()
            .map(|record| record.resolve(resolver))
            .collect::<Result<Vec<_>, _>>()?;
        require_owner_order(&records).map_err(GenericInitializationResolutionError::Record)?;
        Ok(CanonicalExportGenericInitializationsV1 { records })
    }
}

impl DecodedNominalInitialization {
    fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<ExportGenericNominalInitializationV1, GenericInitializationResolutionError<E>>
    where
        R: DefaultStatementReferenceResolver<E>,
    {
        use GenericInitializationResolutionError as Error;
        let owner = resolver.resolve(self.owner).map_err(Error::Owner)?;
        let common = self
            .common
            .into_iter()
            .map(|step| {
                Ok(match step {
                    DecodedCommonStep::Field(field, value) => {
                        ExportCommonInitializationStepV1::Field {
                            field: field.resolve(resolver).map_err(Error::Field)?,
                            value: fragment(value, resolver)?,
                        }
                    }
                    DecodedCommonStep::Body(body) => {
                        ExportCommonInitializationStepV1::Body(fragment(body, resolver)?)
                    }
                })
            })
            .collect::<Result<_, Error<E>>>()?;
        let constructors = self
            .constructors
            .into_iter()
            .map(|constructor| constructor.resolve(resolver))
            .collect::<Result<Vec<_>, _>>()?;
        let release_policy = match self.release_policy {
            crate::ReleasePolicy::None => crate::ReleasePolicy::None,
            crate::ReleasePolicy::SynchronousGcFree { hook } => {
                crate::ReleasePolicy::SynchronousGcFree {
                    hook: hook.resolve(resolver)?,
                }
            }
        };
        ExportGenericNominalInitializationV1::from_canonical(
            owner,
            common,
            constructors,
            release_policy,
        )
        .map_err(Error::Record)
    }
}

impl DecodedConstructor {
    fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<ExportConstructorInitializationV1, GenericInitializationResolutionError<E>>
    where
        R: DefaultStatementReferenceResolver<E>,
    {
        use GenericInitializationResolutionError as Error;
        let declaration = self
            .declaration
            .resolve(resolver)
            .map_err(Error::Constructor)?;
        let inputs = self.inputs.resolve(resolver).map_err(Error::Inputs)?;
        let effects = self.effects.validate().map_err(Error::Effects)?;
        let predicates = self
            .predicates
            .resolve(resolver)
            .map_err(Error::Predicates)?;
        let definition_origin = self
            .definition_origin
            .resolve(resolver)
            .map_err(Error::Origin)?;
        let kind = self.kind.resolve(resolver)?;
        ExportConstructorInitializationV1::try_new(
            declaration,
            inputs,
            effects,
            predicates,
            definition_origin,
            kind,
        )
        .map_err(Error::Record)
    }
}

impl DecodedConstructorKind {
    fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<ExportConstructorInitializationKindV1, GenericInitializationResolutionError<E>>
    where
        R: DefaultStatementReferenceResolver<E>,
    {
        use ExportConstructorInitializationKindV1 as Kind;
        Ok(match self {
            Self::StructPrimary => Kind::StructPrimary,
            Self::StructSecondary(delegation, body) => Kind::StructSecondary {
                delegation: delegation.resolve(resolver)?,
                body: fragment(body, resolver)?,
            },
            Self::ClassPrimary(base, stores) => Kind::ClassPrimary {
                base: base.map(|base| base.resolve(resolver)).transpose()?,
                primary_stores: stores
                    .into_iter()
                    .map(|store| {
                        Ok(ExportPrimaryFieldStoreV1 {
                            field: store
                                .field
                                .resolve(resolver)
                                .map_err(GenericInitializationResolutionError::Field)?,
                            parameter: store.parameter,
                        })
                    })
                    .collect::<Result<_, GenericInitializationResolutionError<E>>>()?,
            },
            Self::ClassSecondaryThis(delegation, body) => Kind::ClassSecondaryThis {
                delegation: delegation.resolve(resolver)?,
                body: fragment(body, resolver)?,
            },
            Self::ClassSecondaryTerminal(base, body) => Kind::ClassSecondaryTerminal {
                base: base.map(|base| base.resolve(resolver)).transpose()?,
                body: fragment(body, resolver)?,
            },
        })
    }
}

impl DecodedDelegation {
    fn resolve<R, E>(
        self,
        resolver: &mut R,
    ) -> Result<ExportConstructorDelegationV1, GenericInitializationResolutionError<E>>
    where
        R: DefaultStatementReferenceResolver<E>,
    {
        Ok(ExportConstructorDelegationV1 {
            target: self
                .target
                .resolve(resolver)
                .map_err(GenericInitializationResolutionError::Constructor)?,
            arguments: fragment(self.arguments, resolver)?,
        })
    }
}

fn fragment<R, E>(
    fragment: DecodedExportTemplateFragmentV1,
    resolver: &mut R,
) -> Result<ExportTemplateFragmentV1, GenericInitializationResolutionError<E>>
where
    R: DefaultStatementReferenceResolver<E>,
{
    fragment
        .resolve(resolver)
        .map_err(|error| GenericInitializationResolutionError::Fragment(Box::new(error)))
}

#[derive(Debug)]
pub enum GenericInitializationResolutionError<E> {
    Owner(E),
    Constructor(DefaultConstructorRefResolutionError<E>),
    Field(DefaultFieldRefResolutionError<E>),
    Inputs(TemplateLocalTableValidationError<E>),
    Effects(CallableSourceEffectsBuildError),
    Predicates(BinderUseListValidationError<E>),
    Origin(SourceOriginResolutionError<E>),
    Fragment(Box<TemplateFragmentResolutionError<E>>),
    Record(GenericInitializationBuildError),
}

impl<E: fmt::Display> fmt::Display for GenericInitializationResolutionError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Owner(error) => write!(formatter, "invalid initialization owner: {error}"),
            Self::Constructor(error) => error.fmt(formatter),
            Self::Field(error) => error.fmt(formatter),
            Self::Inputs(error) => error.fmt(formatter),
            Self::Effects(error) => error.fmt(formatter),
            Self::Predicates(error) => error.fmt(formatter),
            Self::Origin(error) => error.fmt(formatter),
            Self::Fragment(error) => error.fmt(formatter),
            Self::Record(error) => error.fmt(formatter),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for GenericInitializationResolutionError<E> {}
