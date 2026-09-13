use super::*;
pub(super) fn validate_order(
    targets: &[DecodedCoreValueTargetV1],
) -> Result<(), CoreValueTargetSurfaceValidationError> {
    for (index, pair) in targets.windows(2).enumerate() {
        match pair[0].binding.as_array().cmp(pair[1].binding.as_array()) {
            std::cmp::Ordering::Equal => {
                return Err(CoreValueTargetSurfaceValidationError::DuplicateBinding {
                    index: index + 1,
                    identity: *pair[1].binding.as_array(),
                });
            }
            std::cmp::Ordering::Greater => {
                return Err(CoreValueTargetSurfaceValidationError::NonCanonicalOrder {
                    index: index + 1,
                });
            }
            std::cmp::Ordering::Less => {}
        }
    }
    Ok(())
}

pub(super) fn validate_target(
    foundation: &CanonicalHirFoundation,
    direct_surface: &CanonicalDirectPublicSurfaceV1,
    binding: PersistentExportBindingId,
    decoded: DecodedCoreValueTargetV1,
) -> Result<CoreValueTargetV1, CoreValueTargetSurfaceValidationError> {
    let binding_key = foundation.export_binding_key(binding).ok_or(
        CoreValueTargetSurfaceValidationError::UnknownBinding {
            index: 0,
            identity: *binding.as_array(),
        },
    )?;
    if binding_key.exporter() != ConeIdentity::CORE {
        return Err(CoreValueTargetSurfaceValidationError::NonCoreBinding(
            binding,
        ));
    }
    let (definition, source) = resolve_definition(
        foundation,
        binding,
        binding_key.target(),
        decoded.definition,
    )?;
    if !super::super::direct_binding_matches_source(binding_key, source) {
        return Err(CoreValueTargetSurfaceValidationError::BindingSourceMismatch(binding));
    }
    if source.origin() != ConeIdentity::CORE {
        return Err(CoreValueTargetSurfaceValidationError::NonCoreDefinition(
            definition,
        ));
    }
    match definition {
        CoreValueDefinitionV1::ObjectValue(_) => validate_object_target(
            foundation,
            direct_surface,
            binding,
            definition,
            source,
            decoded.source_interface,
            decoded.capability,
        ),
        CoreValueDefinitionV1::Property(_) | CoreValueDefinitionV1::ExtensionProperty(_) => {
            validate_property_target(
                foundation,
                binding,
                definition,
                source,
                decoded.source_interface,
                decoded.capability,
            )
        }
    }
}

fn resolve_definition(
    foundation: &CanonicalHirFoundation,
    binding: PersistentExportBindingId,
    target: BindableEntity,
    decoded: DecodedCoreValueDefinitionV1,
) -> Result<
    (CoreValueDefinitionV1, &scoop_identity::SourceDeclarationKey),
    CoreValueTargetSurfaceValidationError,
> {
    match decoded {
        DecodedCoreValueDefinitionV1::ObjectValue(decoded) => {
            let (id, source) = foundation.object_value_by_bytes(decoded.as_array()).ok_or(
                CoreValueTargetSurfaceValidationError::UnknownObjectValue(*decoded.as_array()),
            )?;
            if target != BindableEntity::ObjectValue(id) {
                return Err(CoreValueTargetSurfaceValidationError::DefinitionMismatch(
                    binding,
                ));
            }
            Ok((CoreValueDefinitionV1::ObjectValue(id), source))
        }
        DecodedCoreValueDefinitionV1::Property(decoded) => {
            let (id, source) = foundation.property_by_bytes(decoded.as_array()).ok_or(
                CoreValueTargetSurfaceValidationError::UnknownProperty(*decoded.as_array()),
            )?;
            if target != BindableEntity::Property(id) {
                return Err(CoreValueTargetSurfaceValidationError::DefinitionMismatch(
                    binding,
                ));
            }
            Ok((CoreValueDefinitionV1::Property(id), source))
        }
        DecodedCoreValueDefinitionV1::ExtensionProperty(decoded) => {
            let (id, source) = foundation
                .extension_property_by_bytes(decoded.as_array())
                .ok_or(
                    CoreValueTargetSurfaceValidationError::UnknownExtensionProperty(
                        *decoded.as_array(),
                    ),
                )?;
            if target != BindableEntity::ExtensionProperty(id) {
                return Err(CoreValueTargetSurfaceValidationError::DefinitionMismatch(
                    binding,
                ));
            }
            Ok((CoreValueDefinitionV1::ExtensionProperty(id), source))
        }
    }
}

fn validate_object_target(
    foundation: &CanonicalHirFoundation,
    direct_surface: &CanonicalDirectPublicSurfaceV1,
    binding: PersistentExportBindingId,
    definition: CoreValueDefinitionV1,
    source: &scoop_identity::SourceDeclarationKey,
    decoded_interface: DecodedCoreValueSourceInterfaceV1,
    decoded_capability: DecodedCoreHirValueCapabilityV1,
) -> Result<CoreValueTargetV1, CoreValueTargetSurfaceValidationError> {
    if source.declaration_kind() != SourceDeclarationKind::Object
        || source.duplicate_signature().type_parameter_count() != 0
    {
        return Err(CoreValueTargetSurfaceValidationError::InvalidObjectSource(
            binding,
        ));
    }
    let DecodedCoreValueSourceInterfaceV1::ObjectValue {
        source_type: decoded_source_type,
    } = decoded_interface
    else {
        return Err(CoreValueTargetSurfaceValidationError::InterfaceKindMismatch(binding));
    };
    let (source_type, source_type_key) = foundation
        .source_type_by_bytes(decoded_source_type.as_array())
        .ok_or(CoreValueTargetSurfaceValidationError::UnknownSourceType(
            *decoded_source_type.as_array(),
        ))?;
    if source_type_key != source {
        return Err(CoreValueTargetSurfaceValidationError::ObjectTypeMismatch(
            binding,
        ));
    }
    require_validation_origin(foundation, DefinitionOriginSubject::Type(source_type))?;
    let has_type_binding = direct_surface.bindings().iter().any(|candidate| {
        foundation
            .export_binding_key(*candidate)
            .is_some_and(|key| {
                key.exporter() == ConeIdentity::CORE
                    && key.target() == BindableEntity::Type(source_type)
            })
    });
    if !has_type_binding {
        return Err(
            CoreValueTargetSurfaceValidationError::MissingObjectTypeBinding {
                binding,
                source_type,
            },
        );
    }
    let DecodedCoreHirValueCapabilityV1::ParamFreeStrong(
        DecodedCoreExactValueInterfaceV1::ObjectValue(decoded_exact),
    ) = decoded_capability
    else {
        return Err(CoreValueTargetSurfaceValidationError::CapabilityMismatch(
            binding,
        ));
    };
    let (exact, key) = foundation
        .exact_type_by_bytes(decoded_exact.as_array())
        .ok_or(CoreValueTargetSurfaceValidationError::UnknownExactType(
            *decoded_exact.as_array(),
        ))?;
    if key != &ExactTypeKey::Nominal(source_type) {
        return Err(CoreValueTargetSurfaceValidationError::ExactInterfaceMismatch(binding));
    }
    Ok(CoreValueTargetV1 {
        binding,
        definition,
        source_interface: CoreValueSourceInterfaceV1::ObjectValue { source_type },
        capability: CoreHirValueCapabilityV1::ParamFreeStrong(
            CoreExactValueInterfaceV1::ObjectValue(exact),
        ),
    })
}

fn validate_property_target(
    foundation: &CanonicalHirFoundation,
    binding: PersistentExportBindingId,
    definition: CoreValueDefinitionV1,
    source: &scoop_identity::SourceDeclarationKey,
    decoded_interface: DecodedCoreValueSourceInterfaceV1,
    decoded_capability: DecodedCoreHirValueCapabilityV1,
) -> Result<CoreValueTargetV1, CoreValueTargetSurfaceValidationError> {
    let expected_kind = match definition {
        CoreValueDefinitionV1::Property(_) => SourceDeclarationKind::Property,
        CoreValueDefinitionV1::ExtensionProperty(_) => SourceDeclarationKind::ExtensionProperty,
        CoreValueDefinitionV1::ObjectValue(_) => {
            return Err(CoreValueTargetSurfaceValidationError::InterfaceKindMismatch(binding));
        }
    };
    if source.declaration_kind() != expected_kind {
        return Err(CoreValueTargetSurfaceValidationError::InvalidPropertySource(binding));
    }
    require_validation_origin(
        foundation,
        definition
            .definition_origin()
            .ok_or(CoreValueTargetSurfaceValidationError::InterfaceKindMismatch(binding))?,
    )?;
    let DecodedCoreValueSourceInterfaceV1::Property(decoded_property) = decoded_interface else {
        return Err(CoreValueTargetSurfaceValidationError::InterfaceKindMismatch(binding));
    };
    let mut resolver = ValueFoundationTypeResolver { foundation };
    let receiver = decoded_property
        .receiver
        .resolve(&mut resolver)
        .map_err(CoreValueTargetSurfaceValidationError::SignatureReference)?;
    let value = decoded_property
        .value
        .resolve(&mut resolver)
        .map_err(CoreValueTargetSurfaceValidationError::SignatureReference)?;
    let (type_parameter_count, expected_receiver) = match source.duplicate_signature() {
        DuplicateSignatureKey::Property {
            type_parameter_count,
            receiver,
        } => (*type_parameter_count, receiver),
        DuplicateSignatureKey::Nominal { .. }
        | DuplicateSignatureKey::Function { .. }
        | DuplicateSignatureKey::Constructor { .. }
        | DuplicateSignatureKey::TypeAlias => {
            return Err(CoreValueTargetSurfaceValidationError::InvalidPropertySource(binding));
        }
    };
    if &receiver != expected_receiver {
        return Err(CoreValueTargetSurfaceValidationError::ReceiverMismatch(
            binding,
        ));
    }
    validate_optional_binders(&receiver, type_parameter_count).map_err(|reason| {
        CoreValueTargetSurfaceValidationError::InvalidBinder { binding, reason }
    })?;
    validate_signature_binders(&value, type_parameter_count).map_err(|reason| {
        CoreValueTargetSurfaceValidationError::InvalidBinder { binding, reason }
    })?;
    let accessors = validate_accessors(foundation, definition, decoded_property.accessors)?;
    let source_interface = CoreValueSourceInterfaceV1::Property(CorePropertySourceInterfaceV1 {
        receiver,
        value,
        accessors,
    });
    let capability = validate_property_capability(
        foundation,
        binding,
        type_parameter_count,
        &source_interface,
        decoded_capability,
    )?;
    Ok(CoreValueTargetV1 {
        binding,
        definition,
        source_interface,
        capability,
    })
}

fn validate_accessors(
    foundation: &CanonicalHirFoundation,
    definition: CoreValueDefinitionV1,
    decoded: DecodedCorePropertyAccessorsV1,
) -> Result<CorePropertyAccessorsV1, CoreValueTargetSurfaceValidationError> {
    let owner = definition
        .property_owner()
        .ok_or(CoreValueTargetSurfaceValidationError::AccessorOwnerMismatch)?;
    match decoded {
        DecodedCorePropertyAccessorsV1::ReadOnly { getter } => {
            let getter = validate_accessor(foundation, owner, AccessorRole::Getter, getter)?;
            Ok(CorePropertyAccessorsV1::ReadOnly { getter })
        }
        DecodedCorePropertyAccessorsV1::ReadWrite { getter, setter } => {
            let getter = validate_accessor(foundation, owner, AccessorRole::Getter, getter)?;
            let setter = validate_accessor(foundation, owner, AccessorRole::Setter, setter)?;
            if getter == setter {
                return Err(CoreValueTargetSurfaceValidationError::DuplicateAccessor(
                    getter,
                ));
            }
            Ok(CorePropertyAccessorsV1::ReadWrite { getter, setter })
        }
    }
}

fn validate_accessor(
    foundation: &CanonicalHirFoundation,
    owner: PersistentPropertyOwner,
    role: AccessorRole,
    decoded: DecodedPersistentId<PersistentPropertyAccessorId>,
) -> Result<PersistentPropertyAccessorId, CoreValueTargetSurfaceValidationError> {
    let (accessor, key) = foundation
        .property_accessor_by_bytes(decoded.as_array())
        .ok_or(CoreValueTargetSurfaceValidationError::UnknownAccessor(
            *decoded.as_array(),
        ))?;
    if key.owner() != owner {
        return Err(CoreValueTargetSurfaceValidationError::AccessorOwnerMismatch);
    }
    if key.role() != role {
        return Err(CoreValueTargetSurfaceValidationError::AccessorRoleMismatch(
            accessor,
        ));
    }
    require_validation_origin(
        foundation,
        DefinitionOriginSubject::PropertyAccessor(accessor),
    )?;
    Ok(accessor)
}

fn require_validation_origin(
    foundation: &CanonicalHirFoundation,
    subject: DefinitionOriginSubject,
) -> Result<(), CoreValueTargetSurfaceValidationError> {
    if foundation.definition_origin(subject).is_some() {
        Ok(())
    } else {
        Err(CoreValueTargetSurfaceValidationError::MissingDefinitionOrigin(subject))
    }
}

fn validate_property_capability(
    foundation: &CanonicalHirFoundation,
    binding: PersistentExportBindingId,
    type_parameter_count: u32,
    source_interface: &CoreValueSourceInterfaceV1,
    decoded: DecodedCoreHirValueCapabilityV1,
) -> Result<CoreHirValueCapabilityV1, CoreValueTargetSurfaceValidationError> {
    match decoded {
        DecodedCoreHirValueCapabilityV1::GenericUnavailable {
            type_parameter_count: actual,
        } => {
            if actual == 0 || actual != type_parameter_count {
                return Err(CoreValueTargetSurfaceValidationError::CapabilityMismatch(
                    binding,
                ));
            }
            Ok(CoreHirValueCapabilityV1::GenericUnavailable {
                type_parameter_count: actual,
            })
        }
        DecodedCoreHirValueCapabilityV1::ParamFreeStrong(decoded) => {
            if type_parameter_count != 0 {
                return Err(CoreValueTargetSurfaceValidationError::CapabilityMismatch(
                    binding,
                ));
            }
            let exact =
                validate_exact_property_interface(foundation, binding, source_interface, decoded)?;
            if !exact_roots_are_nominal(foundation, &exact) {
                return Err(CoreValueTargetSurfaceValidationError::CapabilityMismatch(
                    binding,
                ));
            }
            Ok(CoreHirValueCapabilityV1::ParamFreeStrong(exact))
        }
        DecodedCoreHirValueCapabilityV1::StructuralUnavailable(decoded) => {
            if type_parameter_count != 0 {
                return Err(CoreValueTargetSurfaceValidationError::CapabilityMismatch(
                    binding,
                ));
            }
            let exact =
                validate_exact_property_interface(foundation, binding, source_interface, decoded)?;
            if exact_roots_are_nominal(foundation, &exact) {
                return Err(CoreValueTargetSurfaceValidationError::CapabilityMismatch(
                    binding,
                ));
            }
            Ok(CoreHirValueCapabilityV1::StructuralUnavailable(exact))
        }
    }
}

fn validate_exact_property_interface(
    foundation: &CanonicalHirFoundation,
    binding: PersistentExportBindingId,
    source_interface: &CoreValueSourceInterfaceV1,
    decoded: DecodedCoreExactValueInterfaceV1,
) -> Result<CoreExactValueInterfaceV1, CoreValueTargetSurfaceValidationError> {
    let CoreValueSourceInterfaceV1::Property(source) = source_interface else {
        return Err(CoreValueTargetSurfaceValidationError::InterfaceKindMismatch(binding));
    };
    let DecodedCoreExactValueInterfaceV1::Property {
        receiver: decoded_receiver,
        value: decoded_value,
    } = decoded
    else {
        return Err(CoreValueTargetSurfaceValidationError::ExactInterfaceMismatch(binding));
    };
    let mut resolver = ValueFoundationTypeResolver { foundation };
    let receiver = decoded_receiver
        .resolve(&mut resolver)
        .map_err(CoreValueTargetSurfaceValidationError::SignatureReference)?;
    let value = resolver
        .resolve(decoded_value)
        .map_err(CoreValueTargetSurfaceValidationError::SignatureReference)?;
    let replayed_receiver = receiver
        .into_option()
        .map(|exact| signature_type_for_exact(foundation, exact))
        .transpose()
        .map_err(
            |reason| CoreValueTargetSurfaceValidationError::ExactRelation { binding, reason },
        )?;
    let replayed_value = signature_type_for_exact(foundation, value).map_err(|reason| {
        CoreValueTargetSurfaceValidationError::ExactRelation { binding, reason }
    })?;
    if OptionalSignatureType::from_option(replayed_receiver) != source.receiver
        || replayed_value != source.value
    {
        return Err(CoreValueTargetSurfaceValidationError::ExactInterfaceMismatch(binding));
    }
    Ok(CoreExactValueInterfaceV1::Property { receiver, value })
}

fn exact_roots_are_nominal(
    foundation: &CanonicalHirFoundation,
    interface: &CoreExactValueInterfaceV1,
) -> bool {
    match interface {
        CoreExactValueInterfaceV1::ObjectValue(exact) => matches!(
            foundation.exact_type_key(*exact),
            Some(ExactTypeKey::Nominal(_))
        ),
        CoreExactValueInterfaceV1::Property { receiver, value } => receiver
            .into_option()
            .into_iter()
            .chain(std::iter::once(*value))
            .all(|exact| {
                matches!(
                    foundation.exact_type_key(exact),
                    Some(ExactTypeKey::Nominal(_))
                )
            }),
    }
}

pub(super) fn validate_property_duplicate(
    duplicate: &DuplicateSignatureKey,
    receiver: &Option<SignatureTypeKey>,
) -> Result<(), ()> {
    let DuplicateSignatureKey::Property {
        receiver: expected_receiver,
        ..
    } = duplicate
    else {
        return Err(());
    };
    match (receiver, expected_receiver) {
        (None, OptionalSignatureType::Absent) => Ok(()),
        (Some(actual), OptionalSignatureType::Present(expected)) if actual == expected.as_ref() => {
            Ok(())
        }
        _ => Err(()),
    }
}

fn validate_optional_binders(
    receiver: &OptionalSignatureType,
    type_parameter_count: u32,
) -> Result<(), super::super::CoreCallableBinderError> {
    match receiver {
        OptionalSignatureType::Absent => Ok(()),
        OptionalSignatureType::Present(receiver) => {
            validate_signature_binders(receiver, type_parameter_count)
        }
    }
}

struct ValueFoundationTypeResolver<'a> {
    foundation: &'a CanonicalHirFoundation,
}

impl PersistentIdResolver<PersistentTypeId> for ValueFoundationTypeResolver<'_> {
    type Error = CoreValueSignatureReferenceError;

    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentTypeId>,
    ) -> Result<PersistentTypeId, Self::Error> {
        self.foundation
            .source_type_by_bytes(id.as_array())
            .map(|(id, _)| id)
            .ok_or(CoreValueSignatureReferenceError::UnknownSourceType(
                *id.as_array(),
            ))
    }
}

impl PersistentIdResolver<PersistentGenericTypeId> for ValueFoundationTypeResolver<'_> {
    type Error = CoreValueSignatureReferenceError;

    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentGenericTypeId>,
    ) -> Result<PersistentGenericTypeId, Self::Error> {
        self.foundation
            .generic_type_by_bytes(id.as_array())
            .map(|(id, _)| id)
            .ok_or(CoreValueSignatureReferenceError::UnknownGenericType(
                *id.as_array(),
            ))
    }
}

impl PersistentIdResolver<PersistentExactTypeId> for ValueFoundationTypeResolver<'_> {
    type Error = CoreValueSignatureReferenceError;

    fn resolve(
        &mut self,
        id: DecodedPersistentId<PersistentExactTypeId>,
    ) -> Result<PersistentExactTypeId, Self::Error> {
        self.foundation
            .exact_type_by_bytes(id.as_array())
            .map(|(id, _)| id)
            .ok_or(CoreValueSignatureReferenceError::UnknownExactType(
                *id.as_array(),
            ))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoreValueSignatureReferenceError {
    UnknownSourceType([u8; 32]),
    UnknownGenericType([u8; 32]),
    UnknownExactType([u8; 32]),
}

impl fmt::Display for CoreValueSignatureReferenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "core value interface references an unknown identity: {self:?}"
        )
    }
}

impl std::error::Error for CoreValueSignatureReferenceError {}
