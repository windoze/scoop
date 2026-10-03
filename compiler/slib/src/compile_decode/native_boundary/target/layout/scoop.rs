use super::*;

impl NativeBoundaryNormalizer<'_> {
    pub(in super::super) fn scoop_argument(
        &mut self,
        exact: PersistentExactTypeId,
    ) -> Result<ScoopAbiArgument, NativeBoundaryCompileError> {
        let storage = self.scoop_storage(exact)?;
        scoop_lir::canonical_scoop_abi_argument(self.target, storage)
            .map_err(NativeBoundaryTargetError::ScoopAbi)
            .map_err(Into::into)
    }

    pub(in super::super) fn scoop_return(
        &mut self,
        exact: PersistentExactTypeId,
    ) -> Result<ScoopAbiReturn, NativeBoundaryCompileError> {
        let storage = self.scoop_storage(exact)?;
        scoop_lir::canonical_scoop_abi_value_return(self.target, storage)
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

    pub(super) fn scoop_layout(
        &mut self,
        exact: PersistentExactTypeId,
    ) -> Result<PhysicalType, NativeBoundaryCompileError> {
        if let Some(layout) = self.scoop_layouts.get(&exact) {
            return Ok(*layout);
        }
        if !self
            .visiting_scoop_layouts
            .push(exact, &WirePath::root().field(1))?
        {
            return Err(NativeBoundaryTargetError::ScoopLayoutCycle { exact }.into());
        }
        let layout = if self.is_unit(exact) {
            PhysicalType {
                size: 0,
                alignment: 1,
                shape: ScoopAbiValueShape::Aggregate,
                gc_free: true,
            }
        } else if self.is_any(exact) {
            pointer(self.target, scoop_lir::PointerKind::Managed, false)
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
            let mut fields = allocate_vec(element_count, &path)?;
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
        insert_entry(
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
            NativeBoundaryNominalShape::Intrinsic(representation) => {
                intrinsic_layout(self.target, representation.family())
            }
            NativeBoundaryNominalShape::Reference => {
                Ok(pointer(self.target, scoop_lir::PointerKind::Managed, false))
            }
            NativeBoundaryNominalShape::Struct { c_layout, fields } => {
                let mut normalized = allocate_vec(fields.len(), &WirePath::root().field(1))?;
                for field in fields {
                    let exact = self.signature_exact(field.ty(), &binders)?;
                    normalized.push(self.scoop_layout(exact)?);
                }
                aggregate_with_policy(&normalized, *c_layout, exact)
            }
            NativeBoundaryNominalShape::Enum { variants } => {
                let path = WirePath::root().field(1);
                let mut normalized = allocate_vec(variants.len(), &path)?;
                for variant in variants {
                    let mut fields = allocate_vec(variant.fields().len(), &path)?;
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

        let mut payloads = allocate_vec(variants.len(), &WirePath::root().field(1))?;
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

    pub(super) fn niche_pointer_kind(
        &self,
        exact: PersistentExactTypeId,
    ) -> Option<scoop_lir::PointerKind> {
        if self.is_any(exact) {
            return Some(scoop_lir::PointerKind::Managed);
        }
        match self.exact_types.get(&exact).map(Arc::as_ref) {
            Some(ExactTypeKey::Function { .. }) => Some(scoop_lir::PointerKind::Managed),
            Some(ExactTypeKey::RawPointer(_)) => Some(scoop_lir::PointerKind::Raw),
            Some(ExactTypeKey::NativeFunctionPointer { .. }) => Some(scoop_lir::PointerKind::Code),
            Some(ExactTypeKey::Nominal(owner)) => self
                .definitions
                .get(&NativeBoundaryNominalOwner::Concrete(*owner))
                .filter(|definition| is_reference(definition.shape()))
                .map(|_| scoop_lir::PointerKind::Managed),
            Some(ExactTypeKey::NominalApplication { origin, .. }) => self
                .definitions
                .get(&NativeBoundaryNominalOwner::GenericTemplate(*origin))
                .filter(|definition| is_reference(definition.shape()))
                .map(|_| scoop_lir::PointerKind::Managed),
            Some(ExactTypeKey::Tuple(_)) | None => None,
        }
    }
}
