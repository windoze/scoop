use super::*;

mod enumeration;
mod scan;

impl Projection<'_> {
    pub(super) fn validate_physical(
        &mut self,
        record: &lir::ExactLayoutExportV1,
        source: &mir::ParamFreeMirTypeExportV1,
        ty: &mir::Type,
    ) -> Result<()> {
        let id = record.identity().layout();

        let actual = physical_layout(id, self.output)?;
        if actual.is_some_and(|layout| layout.identity.scan_record().id() != record.scan()) {
            return Err(ExactLayoutLoweringError::PhysicalLayout(id));
        }
        match record.kind() {
            lir::ExactLayoutBodyKindV1::Value(value) => {
                let actual = actual.ok_or(ExactLayoutLoweringError::PhysicalLayout(id))?;
                self.validate_value(value, source, actual)?;
                if let mir::Type::Enum(enum_id, _) = ty {
                    self.validate_enum(value, *enum_id)?;
                }
            }
            lir::ExactLayoutBodyKindV1::Instance(instance) => {
                if let Some(actual) = actual {
                    self.validate_instance_layout(instance, actual)?;
                }

                let mut descriptors =
                    self.output
                        .module()
                        .meta
                        .type_descriptors
                        .iter()
                        .filter(|(_, descriptor)| {
                            descriptor.instance_layout.layout_record().id() == id
                        });
                let (_, descriptor) = descriptors
                    .next()
                    .ok_or(ExactLayoutLoweringError::PhysicalDescriptor(source.exact()))?;
                if descriptors.next().is_some()
                    || descriptor.identity.exact_type() != source.exact()
                    || descriptor.instance_layout.scan_record().id() != record.scan()
                    || !scan::shape_matches(instance.shape(), &descriptor.instance_shape)?
                {
                    return Err(ExactLayoutLoweringError::PhysicalDescriptor(source.exact()));
                }
            }
        }
        Ok(())
    }

    pub(super) fn validate_structural_value(
        &mut self,
        value: &lir::ExactValueLayoutV1,
    ) -> Result<()> {
        let id = value.identity().layout();
        let actual = physical_layout(id, self.output)?
            .ok_or(ExactLayoutLoweringError::PhysicalLayout(id))?;
        if !matches!(actual.kind, lir::LayoutKind::Plain { .. })
            || !actual.fields.is_empty()
            || actual.c_layout.is_some()
            || actual.interior_mutable
            || actual.identity.scan_record().id() != value.scan()
        {
            return Err(ExactLayoutLoweringError::PhysicalLayout(id));
        }
        self.validate_value_storage(value, actual)
    }

    fn validate_value(
        &mut self,
        value: &lir::ExactValueLayoutV1,
        source: &mir::ParamFreeMirTypeExportV1,
        actual: &lir::Layout,
    ) -> Result<()> {
        use lir::ExactRepresentationKindV1 as Kind;
        let storage = value.value().storage();
        let scan = match storage.kind() {
            lir::ValueStorageKindV1::ZeroSized { .. } => &lir::RefScan::None,
            lir::ValueStorageKindV1::Inline { scan, .. } => scan.as_ref_scan(),
        };
        let expected_kind = if source.facts().kind() == mir::MirValueKindV1::Reference {
            mir::MirValueKindV1::Reference
        } else if storage.byte_size() == 0 {
            mir::MirValueKindV1::ZeroSizedValue
        } else {
            mir::MirValueKindV1::NonZeroValue
        };
        if expected_kind != source.facts().kind()
            || !matches!(scan, lir::RefScan::None)
                != (source.facts().gc() == mir::MirGcKindV1::ContainsManagedReferences)
        {
            return Err(ExactLayoutLoweringError::SourceFacts(source.exact()));
        }
        let policy = match source.representation() {
            mir::MirTypeRepresentationV1::Struct {
                c_layout: mir::MirTypeCLayoutPolicyV1::CLayout(contract),
                ..
            } => Some(crate::metadata::lower_c_layout(*contract)),
            _ => None,
        };
        let mutable = matches!(
            source.representation(),
            mir::MirTypeRepresentationV1::Struct {
                interior_mutable: true,
                ..
            }
        );
        let family = match (value.representation().kind(), &actual.kind) {
            (
                Kind::Scalar(lir::ScalarRepresentationKindV1::Float(expected)),
                lir::LayoutKind::Intrinsic(lir::IntrinsicTypeRepresentation::Float(actual)),
            ) => expected == *actual,
            (
                Kind::IntrinsicValue(lir::IntrinsicValueFamilyV1::Unit),
                lir::LayoutKind::Intrinsic(lir::IntrinsicTypeRepresentation::Unit),
            ) => true,
            (
                Kind::Scalar(lir::ScalarRepresentationKindV1::Char),
                lir::LayoutKind::Intrinsic(lir::IntrinsicTypeRepresentation::Char),
            ) => true,
            (
                Kind::Scalar(lir::ScalarRepresentationKindV1::Integer(expected)),
                lir::LayoutKind::Intrinsic(lir::IntrinsicTypeRepresentation::Integer(actual)),
            ) => expected == *actual,
            (
                Kind::Scalar(lir::ScalarRepresentationKindV1::Boolean),
                lir::LayoutKind::Intrinsic(lir::IntrinsicTypeRepresentation::Boolean),
            ) => true,
            (Kind::NicheEnum(_) | Kind::TaggedEnum(_), lir::LayoutKind::Enum { .. }) => true,
            (
                Kind::Scalar(_)
                | Kind::QualifiedPointer(_)
                | Kind::Interface
                | Kind::Struct(_)
                | Kind::Tuple(_)
                | Kind::IntrinsicValue(_),
                lir::LayoutKind::Plain { .. },
            ) => true,
            _ => false,
        };
        if !family || actual.c_layout != policy || actual.interior_mutable != mutable {
            return Err(ExactLayoutLoweringError::PhysicalLayout(
                value.identity().layout(),
            ));
        }
        self.validate_value_storage(value, actual)?;
        match value.representation().kind() {
            Kind::Struct(layout) => {
                self.validate_fields(value.identity().layout(), layout.fields(), &actual.fields)
            }
            _ if actual.fields.is_empty() => Ok(()),
            _ => Err(ExactLayoutLoweringError::PhysicalLayout(
                value.identity().layout(),
            )),
        }
    }

    fn validate_value_storage(
        &mut self,
        value: &lir::ExactValueLayoutV1,
        actual: &lir::Layout,
    ) -> Result<()> {
        let storage = value.value().storage();
        if actual.size != storage.byte_size() || actual.align != storage.alignment().get() {
            return Err(ExactLayoutLoweringError::PhysicalLayout(
                value.identity().layout(),
            ));
        }
        let scan = match storage.kind() {
            lir::ValueStorageKindV1::ZeroSized { .. } => &lir::RefScan::None,
            lir::ValueStorageKindV1::Inline { scan, .. } => scan.as_ref_scan(),
        };
        self.validate_scan(value.identity(), scan, actual)
    }

    fn validate_instance_layout(
        &mut self,
        instance: &lir::ExactInstanceLayoutV1,
        actual: &lir::Layout,
    ) -> Result<()> {
        let id = instance.identity().layout();
        if actual.size != instance.shape().minimum_size()
            || actual.align != instance.shape().instance_alignment()
            || actual.c_layout.is_some()
            || actual.interior_mutable
        {
            return Err(ExactLayoutLoweringError::PhysicalLayout(id));
        }
        self.validate_scan(instance.identity(), instance.shape().object_scan(), actual)?;
        match instance.representation().kind() {
            lir::InstanceRepresentationKindV1::Atomic(_)
                if actual.fields.is_empty()
                    && matches!(actual.kind, lir::LayoutKind::Plain { .. }) =>
            {
                Ok(())
            }
            lir::InstanceRepresentationKindV1::ClassObject(class)
                if matches!(actual.kind, lir::LayoutKind::Plain { .. }) =>
            {
                self.validate_fields(id, class.complete_fields(), &actual.fields)
            }
            lir::InstanceRepresentationKindV1::BoxedPayload(payload)
                if matches!(actual.kind, lir::LayoutKind::Plain { .. }) =>
            {
                let [field] = actual.fields.as_slice() else {
                    return Err(ExactLayoutLoweringError::PhysicalLayout(id));
                };
                let expected_offset = if payload.storage().byte_size() == 0 {
                    0
                } else {
                    instance.shape().inline_offset()
                };
                if field.offset != expected_offset
                    || field.access_align != payload.storage().alignment().get()
                {
                    return Err(ExactLayoutLoweringError::PhysicalLayout(id));
                }
                Ok(())
            }
            lir::InstanceRepresentationKindV1::InlineBytes
                if actual.fields.is_empty()
                    && matches!(
                        actual.kind,
                        lir::LayoutKind::Intrinsic(lir::IntrinsicTypeRepresentation::String)
                    ) =>
            {
                Ok(())
            }
            _ => Err(ExactLayoutLoweringError::PhysicalLayout(id)),
        }
    }

    fn validate_fields(
        &mut self,
        layout: PersistentLayoutId,
        fields: &[lir::PlacedFieldStorageV1],
        actual: &[lir::FieldLayout],
    ) -> Result<()> {
        if fields.len() != actual.len() {
            return Err(ExactLayoutLoweringError::PhysicalLayout(layout));
        }

        if fields.iter().zip(actual).any(|(field, actual)| {
            field.storage().offset().get() != actual.offset
                || field.access_alignment().get() != actual.access_align
        }) {
            return Err(ExactLayoutLoweringError::PhysicalLayout(layout));
        }
        Ok(())
    }

    fn validate_scan(
        &mut self,
        identity: &lir::ExactLayoutIdentityV1,
        expected: &lir::RefScan,
        actual: &lir::Layout,
    ) -> Result<()> {
        let actual_scan = match &actual.kind {
            lir::LayoutKind::Plain { scan } | lir::LayoutKind::Enum { scan } => scan,
            lir::LayoutKind::Intrinsic(_) => &lir::RefScan::None,
        };
        if actual.identity.layout_record().key() != identity.layout_key()
            || !scan::matches(expected, actual_scan)?
        {
            return Err(ExactLayoutLoweringError::PhysicalLayout(identity.layout()));
        }
        Ok(())
    }
}

fn physical_layout(
    id: PersistentLayoutId,
    output: &lir::ConeLirOutput,
) -> Result<Option<&lir::Layout>> {
    let mut matching = output
        .module()
        .meta
        .layouts
        .iter()
        .filter(|(_, layout)| layout.identity.layout_record().id() == id);
    let actual = matching.next().map(|(_, layout)| layout);
    if matching.next().is_some() {
        return Err(ExactLayoutLoweringError::PhysicalLayout(id));
    }
    Ok(actual)
}
