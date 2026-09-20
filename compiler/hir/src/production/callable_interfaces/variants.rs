use scoop_identity::{CallableTemplateOrigin, NominalDeclarationOwner};

use super::{
    CallableInterfaceBuildError, CallableProjection, CallableProjectionError,
    CallableProjectionSubject, effects, parameters,
};
use crate::{
    CallableInterfaceRecordV1, CallableModalityV1, HirSourceNominalIdentity,
    PublicDeclarationOwnerV1, PublicLookupAccessV1,
};

pub(super) fn project_all(
    projection: &CallableProjection<'_>,
    records: &mut Vec<CallableInterfaceRecordV1>,
) -> Result<(), CallableInterfaceBuildError> {
    for &enum_id in &projection.export.public_surface.enums {
        let Some(enumeration) = super::arena_get(&projection.export.enums, enum_id) else {
            let subject = CallableProjectionSubject::Variant {
                enumeration: super::raw_index(enum_id),
                variant: 0,
            };
            return Err(CallableInterfaceBuildError::projection(
                subject,
                CallableProjectionError::UnknownDeclaration,
            ));
        };
        let owner = source_owner(projection, enum_id).map_err(|error| {
            CallableInterfaceBuildError::projection(
                CallableProjectionSubject::Variant {
                    enumeration: super::raw_index(enum_id),
                    variant: 0,
                },
                error,
            )
        })?;
        let binders = projection
            .signatures
            .binder_frame(&enumeration.type_params, 0)
            .map_err(|source| {
                CallableInterfaceBuildError::projection(
                    CallableProjectionSubject::Variant {
                        enumeration: super::raw_index(enum_id),
                        variant: 0,
                    },
                    CallableProjectionError::Signature(source),
                )
            })?;
        let type_parameters = projection
            .signatures
            .project_binder_list(&[], &binders)
            .map_err(|source| {
                CallableInterfaceBuildError::projection(
                    CallableProjectionSubject::Variant {
                        enumeration: super::raw_index(enum_id),
                        variant: 0,
                    },
                    CallableProjectionError::Signature(source),
                )
            })?;
        let self_application = super::arena_get(
            &projection.export.enum_applications,
            enumeration.self_application,
        )
        .ok_or_else(|| {
            CallableInterfaceBuildError::projection(
                CallableProjectionSubject::Variant {
                    enumeration: super::raw_index(enum_id),
                    variant: 0,
                },
                CallableProjectionError::InvalidOwner,
            )
        })?;
        let result = projection
            .signatures
            .map_type(self_application.canonical_type, &binders)
            .map_err(|source| {
                CallableInterfaceBuildError::projection(
                    CallableProjectionSubject::Variant {
                        enumeration: super::raw_index(enum_id),
                        variant: 0,
                    },
                    CallableProjectionError::Signature(source),
                )
            })?;

        for (variant_index, variant) in enumeration.variants.iter().enumerate() {
            let variant_index = u32::try_from(variant_index).map_err(|_| {
                CallableInterfaceBuildError::projection(
                    CallableProjectionSubject::Variant {
                        enumeration: super::raw_index(enum_id),
                        variant: u32::MAX,
                    },
                    CallableProjectionError::UnknownDeclaration,
                )
            })?;
            let subject = CallableProjectionSubject::Variant {
                enumeration: super::raw_index(enum_id),
                variant: variant_index,
            };
            let variant_ref =
                crate::EnumVariantRef::checked(&projection.export.enums, enum_id, variant_index)
                    .ok_or_else(|| {
                        CallableInterfaceBuildError::projection(
                            subject,
                            CallableProjectionError::UnknownDeclaration,
                        )
                    })?;
            let identity = projection
                .export
                .enum_member_identities
                .get_variant(variant_ref)
                .ok_or_else(|| {
                    CallableInterfaceBuildError::projection(
                        subject,
                        CallableProjectionError::MissingIdentity,
                    )
                })?;
            if identity.key().source_owner() != Some(owner) {
                return Err(CallableInterfaceBuildError::projection(
                    subject,
                    CallableProjectionError::PersistentOwnerMismatch,
                ));
            }
            let expected_types = variant
                .fields
                .iter()
                .map(|field| projection.signatures.map_type(field.ty, &binders))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|source| {
                    CallableInterfaceBuildError::projection(
                        subject,
                        CallableProjectionError::Signature(source),
                    )
                })?;
            let parameters = parameters::project(
                projection,
                crate::ExportParameterOwner::VariantConstructor(variant_ref),
                &binders,
                &expected_types,
            )
            .map_err(|source| {
                CallableInterfaceBuildError::projection(
                    subject,
                    CallableProjectionError::Parameters(source),
                )
            })?;
            let effects = effects::source_constructor(crate::Safety::Safe).map_err(|source| {
                CallableInterfaceBuildError::projection(
                    subject,
                    CallableProjectionError::Effects(source),
                )
            })?;
            let record = CallableInterfaceRecordV1::try_new(
                CallableTemplateOrigin::VariantConstructor(identity.id()),
                PublicDeclarationOwnerV1::Nominal(owner),
                type_parameters.clone(),
                None,
                parameters,
                result.clone(),
                effects,
                CallableModalityV1::Final,
                PublicLookupAccessV1::DirectOnly,
            )
            .map_err(|source| {
                CallableInterfaceBuildError::projection(
                    subject,
                    CallableProjectionError::Record(source),
                )
            })?;
            records.push(record);
        }
    }
    Ok(())
}

fn source_owner(
    projection: &CallableProjection<'_>,
    enum_id: crate::EnumId,
) -> Result<NominalDeclarationOwner, CallableProjectionError> {
    let identity = projection
        .export
        .nominal_identities
        .get_enum(enum_id)
        .and_then(crate::HirNominalIdentity::source)
        .ok_or(CallableProjectionError::MissingNominalOwner)?;
    if identity.declaration().origin() != projection.export.cone {
        return Err(CallableProjectionError::ForeignNominalOwner {
            expected: projection.export.cone,
            actual: identity.declaration().origin(),
        });
    }
    Ok(match identity {
        HirSourceNominalIdentity::Concrete(record) => {
            NominalDeclarationOwner::Concrete(record.id())
        }
        HirSourceNominalIdentity::Generic(record) => {
            NominalDeclarationOwner::GenericTemplate(record.id())
        }
    })
}
