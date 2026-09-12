use super::*;

impl<'a> NativeBoundaryNormalizer<'a> {
    pub(super) fn c_storage(
        &mut self,
        exact: PersistentExactTypeId,
    ) -> Result<CanonicalCStorageType, NativeBoundaryCompileError> {
        enum Shape {
            Boolean,
            DataPointer(PersistentExactTypeId),
            CodePointer,
            NullableDataPointer(PersistentExactTypeId),
            NullableCodePointer,
            CLayout,
            Unsupported,
        }
        if self.is_unit(exact) {
            return Err(NativeBoundaryTargetError::NotCAbiSafe { exact }.into());
        }
        if let Some((signedness, bit_width)) = core_integer(self.exact(exact)?) {
            return Ok(CanonicalCStorageType::Integer {
                exact_type: exact,
                signedness,
                bit_width,
            });
        }
        if is_core_application(self.exact(exact)?, CoreNativeBoundaryNominal::PinnedPtr)
            || is_core_application(self.exact(exact)?, CoreNativeBoundaryNominal::GcHandle)
        {
            return Ok(CanonicalCStorageType::Integer {
                exact_type: exact,
                signedness: scoop_identity::Signedness::Unsigned,
                bit_width: scoop_identity::IntegerBitWidth::Bits64,
            });
        }
        let shape = match self.exact(exact)? {
            ExactTypeKey::Nominal(owner)
                if Some(*owner) == CoreNativeBoundaryNominal::Boolean.concrete_id() =>
            {
                Shape::Boolean
            }
            ExactTypeKey::RawPointer(pointee) => Shape::DataPointer(*pointee),
            ExactTypeKey::NativeFunctionPointer { .. } => Shape::CodePointer,
            ExactTypeKey::NominalApplication { origin, arguments }
                if Some(*origin) == CoreNativeBoundaryNominal::Option.generic_id() =>
            {
                match self.exact(arguments.as_slice()[0])? {
                    ExactTypeKey::RawPointer(pointee) => Shape::NullableDataPointer(*pointee),
                    ExactTypeKey::NativeFunctionPointer { .. } => Shape::NullableCodePointer,
                    _ => Shape::Unsupported,
                }
            }
            ExactTypeKey::Nominal(_) | ExactTypeKey::NominalApplication { .. } => Shape::CLayout,
            ExactTypeKey::Tuple(_) | ExactTypeKey::Function { .. } => Shape::Unsupported,
        };
        match shape {
            Shape::Boolean => Ok(CanonicalCStorageType::Boolean { exact_type: exact }),
            Shape::DataPointer(pointee) => Ok(CanonicalCStorageType::DataPointer {
                exact_type: exact,
                pointee: if self.is_unit(pointee) {
                    CDataPointee::OpaqueUnit
                } else {
                    CDataPointee::ExactObject(pointee)
                },
                storage: CPointerStorage::Direct,
            }),
            Shape::CodePointer => Ok(CanonicalCStorageType::CodePointer {
                exact_type: exact,
                storage: CPointerStorage::Direct,
            }),
            Shape::NullableDataPointer(pointee) => Ok(CanonicalCStorageType::DataPointer {
                exact_type: exact,
                pointee: if self.is_unit(pointee) {
                    CDataPointee::OpaqueUnit
                } else {
                    CDataPointee::ExactObject(pointee)
                },
                storage: CPointerStorage::NullableWrapper(exact),
            }),
            Shape::NullableCodePointer => Ok(CanonicalCStorageType::CodePointer {
                exact_type: exact,
                storage: CPointerStorage::NullableWrapper(exact),
            }),
            Shape::CLayout => {
                let layout = self.c_layout(exact)?;
                Ok(CanonicalCStorageType::Struct {
                    exact_type: exact,
                    layout,
                })
            }
            Shape::Unsupported => Err(NativeBoundaryTargetError::NotCAbiSafe { exact }.into()),
        }
    }

    fn c_layout(
        &mut self,
        exact: PersistentExactTypeId,
    ) -> Result<CanonicalCAbiLayoutFingerprint, NativeBoundaryCompileError> {
        if let Some(fingerprint) = self.layouts_by_type.get(&exact) {
            return Ok(*fingerprint);
        }
        if !self
            .visiting_c_layouts
            .push(exact, self.meter, &WirePath::root().field(16))?
        {
            return Err(NativeBoundaryTargetError::CLayoutCycle { exact }.into());
        }
        let (definition, binders) = self.definition(exact)?;
        let NativeBoundaryNominalShape::Struct { c_layout, fields } = definition.shape() else {
            return Err(NativeBoundaryTargetError::NotCAbiSafe { exact }.into());
        };
        let NativeBoundaryCLayoutPolicy::CLayout { aligned, packed } = c_layout else {
            return Err(NativeBoundaryTargetError::NotCAbiSafe { exact }.into());
        };
        let aligned = *aligned;
        let packed = *packed;
        let mut normalized = metered_vec(self.meter, fields.len(), &WirePath::root().field(16))?;
        let mut size = 0_u64;
        let mut alignment = override_bytes(aligned).unwrap_or(1);
        for field in fields {
            let field_exact = self.signature_exact(field.ty(), &binders)?;
            let storage = self.c_storage(field_exact)?;
            let (field_size, natural_alignment) = self.c_storage_layout(storage)?;
            let access_alignment = override_bytes(packed)
                .map_or(natural_alignment, |packed| natural_alignment.min(packed));
            let offset = align_up(size, access_alignment)?;
            size = offset
                .checked_add(field_size)
                .ok_or(NativeBoundaryTargetError::LayoutOverflow { exact })?;
            alignment = alignment.max(access_alignment);
            normalized.push(CanonicalCAbiLayoutField::new(
                field.field(),
                offset,
                storage,
            ));
        }
        size = align_up(size, alignment)?;
        let alignment = NonZeroU64::new(alignment)
            .ok_or(NativeBoundaryTargetError::LayoutOverflow { exact })?;
        let layout = CanonicalCAbiLayout::new(exact, size, alignment, aligned, packed, normalized);
        let record = CanonicalCAbiLayoutFingerprintRecord::new(layout)
            .map_err(NativeBoundaryTargetError::Hash)?;
        let fingerprint = record.fingerprint();
        insert_metered(
            self.meter,
            &mut self.expected_layouts,
            fingerprint,
            record,
            &WirePath::root().field(16),
        )?;
        insert_metered(
            self.meter,
            &mut self.layouts_by_type,
            exact,
            fingerprint,
            &WirePath::root().field(16),
        )?;
        self.visiting_c_layouts.remove(&exact);
        Ok(fingerprint)
    }

    fn c_storage_layout(
        &self,
        storage: CanonicalCStorageType,
    ) -> Result<(u64, u64), NativeBoundaryCompileError> {
        match storage {
            CanonicalCStorageType::Integer { bit_width, .. } => {
                let layout = self.target.scalar_layout(integer_scalar_kind(bit_width));
                Ok((layout.size_bytes(), layout.alignment_bytes()))
            }
            CanonicalCStorageType::Boolean { .. } => {
                let layout = self.target.scalar_layout(scoop_lir::BackendScalarKind::I1);
                Ok((layout.size_bytes(), layout.alignment_bytes()))
            }
            CanonicalCStorageType::DataPointer { .. } => {
                let layout = self.target.data_pointer().layout();
                Ok((layout.size_bytes(), layout.alignment_bytes()))
            }
            CanonicalCStorageType::CodePointer { .. } => {
                let layout = self.target.code_pointer().layout();
                Ok((layout.size_bytes(), layout.alignment_bytes()))
            }
            CanonicalCStorageType::Struct { layout, .. } => {
                let record = self
                    .expected_layouts
                    .get(&layout)
                    .ok_or(NativeBoundaryTargetError::MissingComputedCLayout { layout })?;
                Ok((
                    record.layout().byte_size(),
                    record.layout().alignment().get(),
                ))
            }
        }
    }

    pub(super) fn scoop_argument(
        &mut self,
        exact: PersistentExactTypeId,
    ) -> Result<ScoopAbiArgument, NativeBoundaryCompileError> {
        let storage = self.scoop_storage(exact)?;
        if storage.byte_size() == 0 {
            ScoopAbiArgument::elided_zst(storage)
        } else if storage.shape() == ScoopAbiValueShape::Scalar {
            ScoopAbiArgument::direct(storage)
        } else {
            ScoopAbiArgument::indirect(storage)
        }
        .map_err(NativeBoundaryTargetError::ScoopAbi)
        .map_err(Into::into)
    }

    pub(super) fn scoop_return(
        &mut self,
        exact: PersistentExactTypeId,
    ) -> Result<ScoopAbiReturn, NativeBoundaryCompileError> {
        let storage = self.scoop_storage(exact)?;
        if storage.byte_size() == 0 {
            ScoopAbiReturn::elided_zst(storage)
        } else if storage.shape() == ScoopAbiValueShape::Scalar {
            ScoopAbiReturn::direct(storage)
        } else {
            ScoopAbiReturn::indirect(storage)
        }
        .map_err(NativeBoundaryTargetError::ScoopAbi)
        .map_err(Into::into)
    }

    fn scoop_storage(
        &mut self,
        exact: PersistentExactTypeId,
    ) -> Result<CanonicalScoopStorage, NativeBoundaryCompileError> {
        let physical = self.scoop_layout(exact)?;
        let alignment = NonZeroU64::new(physical.alignment)
            .ok_or(NativeBoundaryTargetError::LayoutOverflow { exact })?;
        Ok(CanonicalScoopStorage::new(
            exact,
            physical.size,
            alignment,
            physical.shape,
        ))
    }

    fn scoop_layout(
        &mut self,
        exact: PersistentExactTypeId,
    ) -> Result<PhysicalType, NativeBoundaryCompileError> {
        if let Some(layout) = self.scoop_layouts.get(&exact) {
            return Ok(*layout);
        }
        if !self
            .visiting_scoop_layouts
            .push(exact, self.meter, &WirePath::root().field(1))?
        {
            return Err(NativeBoundaryTargetError::ScoopLayoutCycle { exact }.into());
        }
        let layout = if let Some((_, bit_width)) = core_integer(self.exact(exact)?) {
            scalar(
                self.target.scalar_layout(integer_scalar_kind(bit_width)),
                true,
            )
        } else if is_core_application(self.exact(exact)?, CoreNativeBoundaryNominal::PinnedPtr)
            || is_core_application(self.exact(exact)?, CoreNativeBoundaryNominal::GcHandle)
        {
            scalar(
                self.target.scalar_layout(scoop_lir::BackendScalarKind::I64),
                true,
            )
        } else if self.is_unit(exact) {
            PhysicalType {
                size: 0,
                alignment: 1,
                shape: ScoopAbiValueShape::Aggregate,
                gc_free: true,
            }
        } else if matches!(
            self.exact(exact)?,
            ExactTypeKey::Nominal(owner)
                if Some(*owner) == CoreNativeBoundaryNominal::Boolean.concrete_id()
        ) {
            scalar(
                self.target.scalar_layout(scoop_lir::BackendScalarKind::I1),
                true,
            )
        } else if matches!(
            self.exact(exact)?,
            ExactTypeKey::Nominal(_) | ExactTypeKey::NominalApplication { .. }
        ) {
            self.nominal_layout(exact)?
        } else if let Some(element_count) = match self.exact(exact)? {
            ExactTypeKey::Tuple(elements) => Some(elements.as_slice().len()),
            _ => None,
        } {
            let path = WirePath::root().field(1);
            let mut fields = metered_vec(self.meter, element_count, &path)?;
            for index in 0..element_count {
                let field = match self.exact(exact)? {
                    ExactTypeKey::Tuple(elements) => elements.as_slice()[index],
                    _ => return Err(NativeBoundaryTargetError::InvalidSignatureShape.into()),
                };
                fields.push(self.scoop_layout(field)?);
            }
            aggregate(&fields, ScoopAbiValueShape::Aggregate, exact)?
        } else {
            match self.exact(exact)? {
                ExactTypeKey::Function { .. } => {
                    pointer(self.target, scoop_lir::PointerKind::Managed, false)
                }
                ExactTypeKey::RawPointer(_) => {
                    pointer(self.target, scoop_lir::PointerKind::Raw, true)
                }
                ExactTypeKey::NativeFunctionPointer { .. } => {
                    pointer(self.target, scoop_lir::PointerKind::Code, true)
                }
                _ => return Err(NativeBoundaryTargetError::InvalidSignatureShape.into()),
            }
        };
        self.visiting_scoop_layouts.remove(&exact);
        insert_metered(
            self.meter,
            &mut self.scoop_layouts,
            exact,
            layout,
            &WirePath::root().field(1),
        )?;
        Ok(layout)
    }

    fn nominal_layout(
        &mut self,
        exact: PersistentExactTypeId,
    ) -> Result<PhysicalType, NativeBoundaryCompileError> {
        let (definition, binders) = self.definition(exact)?;
        match definition.shape() {
            NativeBoundaryNominalShape::Reference => {
                Ok(pointer(self.target, scoop_lir::PointerKind::Managed, false))
            }
            NativeBoundaryNominalShape::Struct { c_layout, fields } => {
                let mut normalized =
                    metered_vec(self.meter, fields.len(), &WirePath::root().field(1))?;
                for field in fields {
                    let exact = self.signature_exact(field.ty(), &binders)?;
                    normalized.push(self.scoop_layout(exact)?);
                }
                aggregate_with_policy(&normalized, *c_layout, exact)
            }
            NativeBoundaryNominalShape::Enum { variants } => {
                let path = WirePath::root().field(1);
                let mut normalized = metered_vec(self.meter, variants.len(), &path)?;
                for variant in variants {
                    let mut fields = metered_vec(self.meter, variant.fields().len(), &path)?;
                    for field in variant.fields() {
                        let exact = self.signature_exact(field.ty(), &binders)?;
                        fields.push((exact, self.scoop_layout(exact)?));
                    }
                    normalized.push(fields);
                }
                self.enum_layout(exact, &normalized)
            }
        }
    }

    fn enum_layout(
        &mut self,
        exact: PersistentExactTypeId,
        variants: &[Vec<(PersistentExactTypeId, PhysicalType)>],
    ) -> Result<PhysicalType, NativeBoundaryCompileError> {
        let niche_pointer_kind = variants
            .iter()
            .find(|variant| !variant.is_empty())
            .filter(|variant| variant.len() == 1)
            .and_then(|variant| self.niche_pointer_kind(variant[0].0));
        if variants.len() == 2
            && variants.iter().any(Vec::is_empty)
            && let Some(niche_pointer_kind) = niche_pointer_kind
        {
            let gc_free = variants.iter().flatten().all(|(_, field)| field.gc_free);
            let mut layout = pointer(self.target, niche_pointer_kind, gc_free);
            layout.gc_free = gc_free;
            return Ok(layout);
        }

        let mut payloads = metered_vec(self.meter, variants.len(), &WirePath::root().field(1))?;
        for variant in variants {
            payloads.push(aggregate_values(
                variant.iter().map(|(_, field)| *field),
                None,
                None,
                ScoopAbiValueShape::Aggregate,
                exact,
            )?);
        }
        let pure_size = payloads
            .iter()
            .filter(|variant| variant.gc_free)
            .map(|variant| variant.size)
            .max()
            .unwrap_or(0);
        let pure_alignment = payloads
            .iter()
            .filter(|variant| variant.gc_free)
            .map(|variant| variant.alignment)
            .max()
            .unwrap_or(1);
        let tag = self.target.scalar_layout(scoop_lir::BackendScalarKind::I64);
        let pure_offset = align_up(tag.size_bytes(), pure_alignment)?;
        let mut cursor = pure_offset
            .checked_add(pure_size)
            .ok_or(NativeBoundaryTargetError::LayoutOverflow { exact })?;
        let mut alignment = tag.alignment_bytes().max(pure_alignment);
        for variant in payloads.iter().filter(|variant| !variant.gc_free) {
            cursor = align_up(cursor, variant.alignment)?;
            cursor = cursor
                .checked_add(variant.size)
                .ok_or(NativeBoundaryTargetError::LayoutOverflow { exact })?;
            alignment = alignment.max(variant.alignment);
        }
        Ok(PhysicalType {
            size: align_up(cursor, alignment)?,
            alignment,
            shape: ScoopAbiValueShape::Aggregate,
            gc_free: payloads.iter().all(|variant| variant.gc_free),
        })
    }

    fn niche_pointer_kind(&self, exact: PersistentExactTypeId) -> Option<scoop_lir::PointerKind> {
        match self.exact_types.get(&exact) {
            Some(ExactTypeKey::Function { .. }) => Some(scoop_lir::PointerKind::Managed),
            Some(ExactTypeKey::RawPointer(_)) => Some(scoop_lir::PointerKind::Raw),
            Some(ExactTypeKey::NativeFunctionPointer { .. }) => Some(scoop_lir::PointerKind::Code),
            Some(ExactTypeKey::Nominal(owner)) => self
                .definitions
                .get(&NativeBoundaryNominalOwner::Concrete(*owner))
                .filter(|definition| {
                    matches!(definition.shape(), NativeBoundaryNominalShape::Reference)
                })
                .map(|_| scoop_lir::PointerKind::Managed),
            Some(ExactTypeKey::NominalApplication { origin, .. }) => self
                .definitions
                .get(&NativeBoundaryNominalOwner::GenericTemplate(*origin))
                .filter(|definition| {
                    matches!(definition.shape(), NativeBoundaryNominalShape::Reference)
                })
                .map(|_| scoop_lir::PointerKind::Managed),
            Some(ExactTypeKey::Tuple(_)) | None => None,
        }
    }

    fn definition(
        &mut self,
        exact: PersistentExactTypeId,
    ) -> Result<
        (
            &'a NativeBoundaryTypeDefinitionRecord,
            Vec<Vec<PersistentExactTypeId>>,
        ),
        NativeBoundaryCompileError,
    > {
        let mut binders = Vec::new();
        let key = self
            .exact_types
            .get(&exact)
            .ok_or(NativeBoundaryTargetError::MissingExactType { exact })?;
        let owner = match key {
            ExactTypeKey::Nominal(owner) => NativeBoundaryNominalOwner::Concrete(*owner),
            ExactTypeKey::NominalApplication { origin, arguments } => {
                let owner = NativeBoundaryNominalOwner::GenericTemplate(*origin);
                push_binder_group(
                    self.meter,
                    &mut binders,
                    arguments.as_slice(),
                    &WirePath::root().field(1),
                )?;
                owner
            }
            _ => return Err(NativeBoundaryTargetError::ExpectedNominal { exact }.into()),
        };
        self.definitions
            .get(&owner)
            .copied()
            .map(|definition| (definition, binders))
            .ok_or(NativeBoundaryCompileError::ClosureRequired { owner })
    }

    pub(super) fn exact(
        &self,
        exact: PersistentExactTypeId,
    ) -> Result<&ExactTypeKey, NativeBoundaryCompileError> {
        self.exact_types
            .get(&exact)
            .ok_or(NativeBoundaryTargetError::MissingExactType { exact }.into())
    }

    pub(super) fn is_unit(&self, exact: PersistentExactTypeId) -> bool {
        matches!(
            self.exact_types.get(&exact),
            Some(ExactTypeKey::Nominal(owner))
                if Some(*owner) == CoreNativeBoundaryNominal::Unit.concrete_id()
        )
    }
}

#[cfg(test)]
mod tests {
    use scoop_hir::NativeBoundaryFieldDefinition;
    use scoop_identity::{
        CLayoutByteAlignment, CLayoutOverride, CanonicalIdentifier, ConeIdentity, DeclarationScope,
        DefinitionOwnerChain, FieldIdentityKey, PackagePath, PersistentTypeId,
        SourceDeclarationKey, SourceDeclarationSite, SourceNominalKind,
    };

    use super::*;

    #[test]
    fn rejects_unit_as_a_c_object_even_without_consulting_a_witness() {
        let owner = CoreNativeBoundaryNominal::Unit.concrete_id().unwrap();
        let record = CborIdentityRecord::from_key(ExactTypeKey::Nominal(owner)).unwrap();
        let exact = record.id();
        let exact_types = HashMap::from([(exact, record.into_key())]);
        let callable_applications = HashMap::new();
        let initialization_units = HashMap::new();
        let definitions = HashMap::new();
        let mut meter = BudgetMeter::new(scoop_wire::DecodeLimits::default());
        let mut normalizer = NativeBoundaryNormalizer::new(
            scoop_lir::LirTargetProfile::DARWIN_AARCH64,
            &mut meter,
            &exact_types,
            &callable_applications,
            &initialization_units,
            &definitions,
        );

        assert!(matches!(
            normalizer.c_storage(exact),
            Err(NativeBoundaryCompileError::Target(
                NativeBoundaryTargetError::NotCAbiSafe { exact: actual }
            )) if actual == exact
        ));
    }

    #[test]
    fn recomputes_packed_and_overaligned_c_struct_layout() {
        let declaration = SourceDeclarationKey::nominal(
            SourceDeclarationSite::new(
                ConeIdentity::SINGLE_FILE,
                PackagePath::root(),
                DefinitionOwnerChain::top_level(),
                DeclarationScope::ConeWide,
            )
            .unwrap(),
            CanonicalIdentifier::new("PackedPair").unwrap(),
            SourceNominalKind::Struct,
            0,
        );
        let owner = PersistentTypeId::from_source_declaration(&declaration).unwrap();
        let first = FieldIdentityKey::source_declared(
            &declaration,
            CanonicalIdentifier::new("first").unwrap(),
        )
        .unwrap();
        let second = FieldIdentityKey::source_declared(
            &declaration,
            CanonicalIdentifier::new("second").unwrap(),
        )
        .unwrap();
        let u8_owner = CoreNativeBoundaryNominal::Unsigned8.concrete_id().unwrap();
        let u64_owner = CoreNativeBoundaryNominal::Unsigned64.concrete_id().unwrap();
        let definition = NativeBoundaryTypeDefinitionRecord::new(
            &declaration,
            &[0],
            NativeBoundaryNominalShape::Struct {
                c_layout: NativeBoundaryCLayoutPolicy::CLayout {
                    aligned: CLayoutOverride::Bytes(CLayoutByteAlignment::Bytes8),
                    packed: CLayoutOverride::Bytes(CLayoutByteAlignment::Bytes1),
                },
                fields: vec![
                    NativeBoundaryFieldDefinition::new(&first, SignatureTypeKey::Nominal(u8_owner))
                        .unwrap(),
                    NativeBoundaryFieldDefinition::new(
                        &second,
                        SignatureTypeKey::Nominal(u64_owner),
                    )
                    .unwrap(),
                ],
            },
        )
        .unwrap();
        let exact = |owner| {
            CborIdentityRecord::from_key(ExactTypeKey::Nominal(owner))
                .unwrap()
                .id()
        };
        let owner_exact = exact(owner);
        let exact_types = HashMap::from([
            (owner_exact, ExactTypeKey::Nominal(owner)),
            (exact(u8_owner), ExactTypeKey::Nominal(u8_owner)),
            (exact(u64_owner), ExactTypeKey::Nominal(u64_owner)),
        ]);
        let callable_applications = HashMap::new();
        let initialization_units = HashMap::new();
        let definitions = HashMap::from([(definition.owner(), &definition)]);
        let mut meter = BudgetMeter::new(scoop_wire::DecodeLimits::default());
        let mut normalizer = NativeBoundaryNormalizer::new(
            scoop_lir::LirTargetProfile::DARWIN_AARCH64,
            &mut meter,
            &exact_types,
            &callable_applications,
            &initialization_units,
            &definitions,
        );

        let CanonicalCStorageType::Struct { layout, .. } =
            normalizer.c_storage(owner_exact).unwrap()
        else {
            panic!("a C-layout struct must normalize to struct storage");
        };
        let layout = normalizer.expected_layouts.get(&layout).unwrap().layout();

        assert_eq!(layout.byte_size(), 16);
        assert_eq!(layout.alignment().get(), 8);
        assert_eq!(
            layout
                .fields()
                .iter()
                .copied()
                .map(CanonicalCAbiLayoutField::offset)
                .collect::<Vec<_>>(),
            [0, 1]
        );
    }

    #[test]
    fn scoop_layout_walk_has_inclusive_semantic_depth_boundaries() {
        let unit = CoreNativeBoundaryNominal::Unit.concrete_id().unwrap();
        let leaf = CborIdentityRecord::from_key(ExactTypeKey::Nominal(unit)).unwrap();
        let middle = CborIdentityRecord::from_key(ExactTypeKey::Tuple(NonEmptyVec::from_first(
            leaf.id(),
            [],
        )))
        .unwrap();
        let root = CborIdentityRecord::from_key(ExactTypeKey::Tuple(NonEmptyVec::from_first(
            middle.id(),
            [],
        )))
        .unwrap();
        let root_id = root.id();
        let exact_types = HashMap::from([
            (leaf.id(), leaf.into_key()),
            (middle.id(), middle.into_key()),
            (root_id, root.into_key()),
        ]);
        let callable_applications = HashMap::new();
        let initialization_units = HashMap::new();
        let definitions = HashMap::new();

        for (limit, accepted) in [(2, false), (3, true), (4, true)] {
            let mut meter = BudgetMeter::new(scoop_wire::DecodeLimits {
                semantic_recursion: limit,
                ..scoop_wire::DecodeLimits::default()
            });
            let result = {
                let mut normalizer = NativeBoundaryNormalizer::new(
                    scoop_lir::LirTargetProfile::DARWIN_AARCH64,
                    &mut meter,
                    &exact_types,
                    &callable_applications,
                    &initialization_units,
                    &definitions,
                );
                normalizer.scoop_layout(root_id)
            };
            assert_eq!(result.is_ok(), accepted);
            if !accepted {
                assert!(matches!(
                    result,
                    Err(NativeBoundaryCompileError::Resource(ref error))
                        if error.kind() == &scoop_wire::WireErrorKind::LimitExceeded {
                            resource: scoop_wire::ResourceKind::SemanticRecursion,
                            limit: 2,
                            observed: 3,
                        }
                ));
            }
        }
    }
}
