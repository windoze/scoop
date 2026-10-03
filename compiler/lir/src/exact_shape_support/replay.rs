use scoop_identity::{
    ExactTypeKey, GeneratedEnumVariantRole, GeneratedNominalKey, PersistentExactTypeId,
    PersistentTypeId, RepresentationRole, SourceDeclarationKey, SourceDeclarationKind,
};
use scoop_wire::WirePath;

use super::*;
use crate::{
    CanonicalExactDescriptorExportsV1, CanonicalExactLayoutExportsV1, ClosedShapeSupportReasonV1,
    ConeLirFoundation, ExactRepresentationKindV1, NichePointerKind,
    ParamFreeShapeSupportRolePartsV1, ParamFreeShapeSupportRolesV1, ShapeSupportAvailabilityV1,
    StrongExactShapeSupportV1, StrongShapeDefinitionV1,
};

mod helper;
use helper::{validate_boxed, validate_helper};

impl ParamFreeShapeSupportExportV1 {
    pub fn replay(
        source: &SourceDeclarationKey,
        layouts: &CanonicalExactLayoutExportsV1,
        descriptors: &CanonicalExactDescriptorExportsV1,
        foundation: &ConeLirFoundation,
    ) -> Result<Self, ParamFreeShapeSupportExportError> {
        validate_inputs(source, layouts, descriptors, foundation)?;
        let path = WirePath::root();
        scoop_wire::encode_canonical_temporary(source, &path)?;
        let nominal = PersistentTypeId::from_source_declaration(source)?;
        let exact = exact(ExactTypeKey::Nominal(nominal))?;
        let source_support = support(nominal, exact, layouts, descriptors)?;
        validate_source_representation(source.declaration_kind(), exact, descriptors)?;

        let boxed = match source.declaration_kind() {
            SourceDeclarationKind::Struct | SourceDeclarationKind::Enum => {
                let key = GeneratedNominalKey::BoxedValue { payload: exact };
                let generated = generated_support(&key, layouts, descriptors)?;
                validate_boxed(exact, generated.exact(), descriptors)?;
                ShapeSupportAvailabilityV1::Available(generated)
            }
            SourceDeclarationKind::Class
            | SourceDeclarationKind::Interface
            | SourceDeclarationKind::Object
            | SourceDeclarationKind::AnnotationClass => ShapeSupportAvailabilityV1::NotApplicable(
                ClosedShapeSupportReasonV1::ReferenceNominalRequiresNoBox,
            ),
            _ => return Err(ParamFreeShapeSupportExportError::NonNominalSource),
        };
        let step_key = GeneratedNominalKey::CoroutineStep { result: exact };
        let step = generated_support(&step_key, layouts, descriptors)?;
        validate_helper(
            &step_key,
            [
                GeneratedEnumVariantRole::CoroutineStepCompleted,
                GeneratedEnumVariantRole::CoroutineStepSuspended,
            ],
            0,
            exact,
            step.exact(),
            descriptors,
        )?;
        let slot_key = GeneratedNominalKey::CoroutineSlot { value: exact };
        let slot = generated_support(&slot_key, layouts, descriptors)?;
        validate_helper(
            &slot_key,
            [
                GeneratedEnumVariantRole::CoroutineSlotEmpty,
                GeneratedEnumVariantRole::CoroutineSlotValue,
            ],
            1,
            exact,
            slot.exact(),
            descriptors,
        )?;
        let roles = ParamFreeShapeSupportRolesV1::from_artifact(ParamFreeShapeSupportRolePartsV1 {
            source_nominal: ShapeSupportAvailabilityV1::Available(nominal),
            value_layout: ShapeSupportAvailabilityV1::Available(source_support.layout()),
            ref_scan: ShapeSupportAvailabilityV1::Available(source_support.scan()),
            type_descriptor: ShapeSupportAvailabilityV1::Available(source_support.descriptor()),
            type_registration: ShapeSupportAvailabilityV1::Available(source_support.registration()),
            boxed_value: boxed,
            coroutine_step: ShapeSupportAvailabilityV1::Available(step),
            coroutine_slot: ShapeSupportAvailabilityV1::Available(slot),
        });
        Ok(Self {
            source: nominal,
            exact,
            provider: foundation.producer(),
            target: layouts.target(),
            roles,
        })
    }
}

fn validate_inputs(
    source: &SourceDeclarationKey,
    layouts: &CanonicalExactLayoutExportsV1,
    descriptors: &CanonicalExactDescriptorExportsV1,
    foundation: &ConeLirFoundation,
) -> Result<(), ParamFreeShapeSupportExportError> {
    if layouts.provider() != foundation.producer()
        || descriptors.provider() != foundation.producer()
    {
        return Err(ParamFreeShapeSupportExportError::Provider);
    }
    if layouts.target() != descriptors.target() {
        return Err(ParamFreeShapeSupportExportError::Target);
    }
    if source.origin() != foundation.producer() {
        return Err(ParamFreeShapeSupportExportError::ForeignSource {
            expected: foundation.producer(),
            actual: source.origin(),
        });
    }
    if !source.declaration_kind().is_nominal() {
        return Err(ParamFreeShapeSupportExportError::NonNominalSource);
    }
    if source.duplicate_signature().type_parameter_count() != 0 {
        return Err(ParamFreeShapeSupportExportError::GenericSource);
    }
    Ok(())
}

fn generated_support(
    key: &GeneratedNominalKey,
    layouts: &CanonicalExactLayoutExportsV1,
    descriptors: &CanonicalExactDescriptorExportsV1,
) -> Result<StrongExactShapeSupportV1, ParamFreeShapeSupportExportError> {
    let nominal = PersistentTypeId::from_generated_key(key)?;
    support(
        nominal,
        exact(ExactTypeKey::Nominal(nominal))?,
        layouts,
        descriptors,
    )
}

fn support(
    nominal: PersistentTypeId,
    exact: PersistentExactTypeId,
    layouts: &CanonicalExactLayoutExportsV1,
    descriptors: &CanonicalExactDescriptorExportsV1,
) -> Result<StrongExactShapeSupportV1, ParamFreeShapeSupportExportError> {
    let descriptor = descriptor(exact, descriptors)?;
    if descriptor.exact_record().key() != &ExactTypeKey::Nominal(nominal) {
        return Err(ParamFreeShapeSupportExportError::ExactIdentity(exact));
    }

    let record = layouts
        .find_exact_role(exact, RepresentationRole::ManagedValue)
        .ok_or(ParamFreeShapeSupportExportError::MissingValueLayout(exact))?;
    let value = record
        .value_handle()
        .ok_or(ParamFreeShapeSupportExportError::MissingValueLayout(exact))?;
    if descriptor.value_layout() != value.as_ref() {
        return Err(ParamFreeShapeSupportExportError::DescriptorLayout(exact));
    }
    let scan = record.scan_definition();
    Ok(StrongExactShapeSupportV1::from_artifact(
        nominal,
        exact,
        value.identity().definition(),
        StrongShapeDefinitionV1::from_artifact(record.scan(), scan.definition(), scan.symbol()),
        descriptor.definition(),
        descriptor.registration(),
    ))
}

fn validate_source_representation(
    kind: SourceDeclarationKind,
    exact: PersistentExactTypeId,
    descriptors: &CanonicalExactDescriptorExportsV1,
) -> Result<(), ParamFreeShapeSupportExportError> {
    let descriptor = descriptor(exact, descriptors)?;
    let representation = descriptor.value_layout().representation().kind();
    let valid = match kind {
        SourceDeclarationKind::Struct => {
            matches!(
                representation,
                ExactRepresentationKindV1::Struct(_)
                    | ExactRepresentationKindV1::Scalar(_)
                    | ExactRepresentationKindV1::IntrinsicValue(_)
            )
        }
        SourceDeclarationKind::Enum => matches!(
            representation,
            ExactRepresentationKindV1::TaggedEnum(_) | ExactRepresentationKindV1::NicheEnum(_)
        ),
        SourceDeclarationKind::Class
        | SourceDeclarationKind::Interface
        | SourceDeclarationKind::Object
        | SourceDeclarationKind::AnnotationClass => matches!(
            representation,
            ExactRepresentationKindV1::QualifiedPointer(NichePointerKind::Managed)
        ),
        _ => false,
    };
    valid
        .then_some(())
        .ok_or(ParamFreeShapeSupportExportError::SourceRepresentation(
            exact,
        ))
}

fn exact(key: ExactTypeKey) -> Result<PersistentExactTypeId, ParamFreeShapeSupportExportError> {
    Ok(PersistentExactTypeId::from_key(&key)?)
}

fn descriptor(
    exact: PersistentExactTypeId,
    descriptors: &CanonicalExactDescriptorExportsV1,
) -> Result<&crate::ExactDescriptorExportV1, ParamFreeShapeSupportExportError> {
    descriptors
        .get(exact)
        .ok_or(ParamFreeShapeSupportExportError::MissingDescriptor(exact))
}
