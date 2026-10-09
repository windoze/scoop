use super::*;

impl<'a> AbiMetadataValidator<'a> {
    pub(super) fn new(module: &'a Module) -> Self {
        Self {
            module,
            visiting_structs: HashSet::new(),
            visiting_enums: HashSet::new(),
        }
    }

    pub(super) fn validate_all(mut self) -> Result<(), CodegenError> {
        for function in self.module.callable_bodies() {
            self.validate_scoop_signature(
                &function.signature,
                &format!("function @{}", function.symbol()),
            )?;
            self.validate_call_signatures(function)?;
            self.validate_boxing(function)?;
            self.validate_pointer_storage(function)?;
            for (id, local) in function.locals.iter() {
                let owner = format!("function @{} local {}", function.symbol(), id.into_raw());
                match local.storage() {
                    scoop_lir::LocalStorage::LogicalZst(value) => {
                        self.validate_zst(value.representation(), &owner)?;
                    }
                    scoop_lir::LocalStorage::AddressableZst(place) => {
                        self.validate_zst(place.value().representation(), &owner)?;
                    }
                    scoop_lir::LocalStorage::NonZero(value) => {
                        self.validate_value(value, &owner)?;
                    }
                }
            }
        }
        for (_, function) in self.module.extern_functions.iter() {
            if let ExternFunctionKind::Scoop { signature, .. } = &function.kind {
                self.validate_scoop_signature(
                    signature,
                    &format!("Scoop extern `{}`", function.source_name),
                )?;
            }
        }
        Ok(())
    }

    pub(super) fn validate_call_signatures(
        &mut self,
        function: &Function,
    ) -> Result<(), CodegenError> {
        let targets = &function.call_targets;
        for (id, signature) in targets.void_signatures.iter() {
            self.validate_arguments(
                signature.arguments(),
                &format!(
                    "function @{} void call signature {}",
                    function.symbol(),
                    id.into_raw()
                ),
            )?;
        }
        for (id, signature) in targets.elided_zst_signatures.iter() {
            let owner = format!(
                "function @{} elided-ZST call signature {}",
                function.symbol(),
                id.into_raw()
            );
            self.validate_arguments(signature.arguments(), &owner)?;
            self.validate_zst(signature.result(), &format!("{owner} result"))?;
        }
        for (id, signature) in targets.direct_signatures.iter() {
            let owner = format!(
                "function @{} direct call signature {}",
                function.symbol(),
                id.into_raw()
            );
            self.validate_arguments(signature.arguments(), &owner)?;
            let result_owner = format!("{owner} result");
            self.validate_direct(signature.result(), &result_owner)?;
        }
        for (id, signature) in targets.indirect_result_signatures.iter() {
            let owner = format!(
                "function @{} indirect-result call signature {}",
                function.symbol(),
                id.into_raw()
            );
            self.validate_arguments(signature.arguments(), &owner)?;
            let result_owner = format!("{owner} result");
            self.validate_value(signature.result(), &result_owner)?;
            if signature.convention() == scoop_lir::IndirectResultConvention::ScoopSret {
                self.validate_passing(
                    signature.result(),
                    scoop_lir::ScoopAbiPassing::Indirect,
                    &result_owner,
                )?;
            }
        }
        Ok(())
    }

    pub(super) fn validate_scoop_signature(
        &mut self,
        signature: &scoop_lir::ScoopAbiSignature,
        owner: &str,
    ) -> Result<(), CodegenError> {
        self.validate_arguments(signature.arguments(), owner)?;
        match signature.result() {
            scoop_lir::AbiReturn::UnitVoid => Ok(()),
            scoop_lir::AbiReturn::ElidedZst(result) => {
                self.validate_zst(result, &format!("{owner} result"))
            }
            scoop_lir::AbiReturn::Direct(result) => {
                let result_owner = format!("{owner} result");
                self.validate_direct(result, &result_owner)
            }
            scoop_lir::AbiReturn::Indirect(result) => {
                let result_owner = format!("{owner} result");
                self.validate_value(result, &result_owner)?;
                self.validate_passing(result, scoop_lir::ScoopAbiPassing::Indirect, &result_owner)
            }
        }
    }

    pub(super) fn validate_arguments(
        &mut self,
        arguments: &[scoop_lir::AbiArgument],
        owner: &str,
    ) -> Result<(), CodegenError> {
        for (index, argument) in arguments.iter().enumerate() {
            let owner = format!("{owner} argument {index}");
            match argument {
                scoop_lir::AbiArgument::ElidedZst(value) => self.validate_zst(value, &owner)?,
                scoop_lir::AbiArgument::Direct(value) => {
                    self.validate_direct(value, &owner)?;
                }
                scoop_lir::AbiArgument::Indirect(value) => {
                    self.validate_value(value, &owner)?;
                    self.validate_passing(value, scoop_lir::ScoopAbiPassing::Indirect, &owner)?;
                }
            }
        }
        Ok(())
    }

    pub(super) fn validate_direct(
        &mut self,
        value: &scoop_lir::AbiDirectValue,
        owner: &str,
    ) -> Result<(), CodegenError> {
        self.validate_value(value.value(), owner)?;
        let passing = match value {
            scoop_lir::AbiDirectValue::Scalar(_) => scoop_lir::ScoopAbiPassing::Direct,
            scoop_lir::AbiDirectValue::DirectParts(_) => scoop_lir::ScoopAbiPassing::DirectParts,
        };
        self.validate_passing(value.value(), passing, owner)
    }

    pub(super) fn validate_passing(
        &self,
        value: &scoop_lir::AbiValue,
        actual: scoop_lir::ScoopAbiPassing,
        owner: &str,
    ) -> Result<(), CodegenError> {
        let shape = scoop_lir::scoop_abi_value_shape(&self.module.enums, value.storage_type())
            .map_err(|error| CodegenError(format!("{owner} has invalid ABI storage: {error:?}")))?;
        let valid = match shape {
            scoop_lir::ScoopAbiValueShape::Scalar => actual == scoop_lir::ScoopAbiPassing::Direct,
            scoop_lir::ScoopAbiValueShape::Interface => {
                actual == scoop_lir::ScoopAbiPassing::DirectParts
            }
            scoop_lir::ScoopAbiValueShape::Aggregate => {
                actual == scoop_lir::ScoopAbiPassing::Indirect
                    || (actual == scoop_lir::ScoopAbiPassing::DirectParts
                        && value.layout().size().get() <= 16
                        && value.layout().alignment().get() <= 8
                        && value.scan() == &scoop_lir::RefScan::None)
            }
        };
        if !valid {
            return Err(CodegenError(format!(
                "{owner} uses {} passing for incompatible {} storage",
                scoop_abi_passing_name(actual),
                value.storage_type().dump()
            )));
        }
        Ok(())
    }

    pub(super) fn validate_zst(
        &mut self,
        value: &scoop_lir::AbiZst,
        owner: &str,
    ) -> Result<(), CodegenError> {
        let expected = self.storage_facts(value.storage_type(), owner)?;
        let actual_align = value.layout().alignment().get();
        if expected.size != 0 || expected.align != actual_align || value.scan() != &expected.scan {
            return Err(abi_metadata_error(
                owner,
                value.storage_type(),
                0,
                actual_align,
                value.scan(),
                &expected,
            ));
        }
        Ok(())
    }

    pub(super) fn validate_value(
        &mut self,
        value: &scoop_lir::AbiValue,
        owner: &str,
    ) -> Result<(), CodegenError> {
        let expected = self.storage_facts(value.storage_type(), owner)?;
        let actual_size = value.layout().size().get();
        let actual_align = value.layout().alignment().get();
        if expected.size != actual_size
            || expected.align != actual_align
            || value.scan() != &expected.scan
        {
            return Err(abi_metadata_error(
                owner,
                value.storage_type(),
                actual_size,
                actual_align,
                value.scan(),
                &expected,
            ));
        }
        Ok(())
    }

    pub(super) fn storage_facts(
        &mut self,
        ty: &LirType,
        owner: &str,
    ) -> Result<StorageFacts, CodegenError> {
        let profile = self.module.meta.target_profile;
        let scalar = |kind| {
            let layout = profile.scalar_layout(kind);
            StorageFacts {
                size: layout.size_bytes(),
                align: layout.alignment_bytes(),
                scan: RefScan::None,
            }
        };
        Ok(match ty {
            LirType::Void => {
                return Err(CodegenError(format!(
                    "{owner} uses void inside an ABI storage type"
                )));
            }
            LirType::I1 => scalar(scoop_lir::BackendScalarKind::I1),
            LirType::I8 => scalar(scoop_lir::BackendScalarKind::I8),
            LirType::I16 => scalar(scoop_lir::BackendScalarKind::I16),
            LirType::I32 => scalar(scoop_lir::BackendScalarKind::I32),
            LirType::F32 => scalar(scoop_lir::BackendScalarKind::F32),
            LirType::F64 => scalar(scoop_lir::BackendScalarKind::F64),
            LirType::I64 | LirType::MachineScalar(_) => scalar(scoop_lir::BackendScalarKind::I64),
            LirType::Interface => StorageFacts {
                size: 16,
                align: 8,
                scan: RefScan::References(vec![0]),
            },
            LirType::Ptr(kind) => {
                let layout = profile.pointer_layout(*kind);
                StorageFacts {
                    size: layout.size_bytes(),
                    align: layout.alignment_bytes(),
                    scan: if *kind == PointerKind::Managed {
                        RefScan::References(vec![0])
                    } else {
                        RefScan::None
                    },
                }
            }
            LirType::ExceptionRecord => {
                let pointer = profile.pointer_layout(PointerKind::Raw);
                let selector = profile.scalar_layout(scoop_lir::BackendScalarKind::I32);
                let (_, size, align) = aggregate_layout(
                    [
                        (pointer.size_bytes(), pointer.alignment_bytes()),
                        (selector.size_bytes(), selector.alignment_bytes()),
                    ],
                    owner,
                )?;
                StorageFacts {
                    size,
                    align,
                    scan: RefScan::None,
                }
            }
            LirType::Aggregate(fields) => self.aggregate_facts(fields, owner)?,
            LirType::Struct(id) => self.struct_facts(*id, owner)?,
            LirType::Enum(id) => self.enum_facts(*id, owner)?,
        })
    }

    pub(super) fn aggregate_facts(
        &mut self,
        fields: &[LirType],
        owner: &str,
    ) -> Result<StorageFacts, CodegenError> {
        let mut facts = Vec::with_capacity(fields.len());
        for (index, field) in fields.iter().enumerate() {
            facts.push(self.storage_facts(field, &format!("{owner} field {index}"))?);
        }
        let (offsets, size, align) =
            aggregate_layout(facts.iter().map(|field| (field.size, field.align)), owner)?;
        let scan = sequence_scans(
            facts
                .iter()
                .zip(offsets)
                .map(|(field, offset)| shift_scan(&field.scan, offset, owner))
                .collect::<Result<Vec<_>, _>>()?,
        );
        Ok(StorageFacts { size, align, scan })
    }
}

pub(super) fn checked_align_up(value: u64, align: u64, owner: &str) -> Result<u64, CodegenError> {
    if align == 0 || !align.is_power_of_two() {
        return Err(CodegenError(format!(
            "{owner} has invalid target storage alignment {align}"
        )));
    }
    value
        .checked_add(align - 1)
        .map(|rounded| rounded & !(align - 1))
        .ok_or_else(|| CodegenError(format!("{owner} layout rounding overflows u64")))
}

pub(super) fn aggregate_layout(
    fields: impl IntoIterator<Item = (u64, u64)>,
    owner: &str,
) -> Result<(Vec<u64>, u64, u64), CodegenError> {
    let mut offsets = Vec::new();
    let mut size = 0;
    let mut align = 1;
    for (field_size, field_align) in fields {
        if field_size == 0 {
            offsets.push(0);
        } else {
            size = checked_align_up(size, field_align, owner)?;
            offsets.push(size);
            size = size
                .checked_add(field_size)
                .ok_or_else(|| CodegenError(format!("{owner} aggregate layout overflows u64")))?;
        }
        align = align.max(field_align);
    }
    Ok((offsets, checked_align_up(size, align, owner)?, align))
}

pub(super) fn shift_scan(scan: &RefScan, base: u64, owner: &str) -> Result<RefScan, CodegenError> {
    Ok(match scan {
        RefScan::None => RefScan::None,
        RefScan::References(offsets) => RefScan::References(
            offsets
                .iter()
                .map(|offset| {
                    base.checked_add(*offset).ok_or_else(|| {
                        CodegenError(format!("{owner} reference scan offset overflows u64"))
                    })
                })
                .collect::<Result<Vec<_>, _>>()?,
        ),
        RefScan::Sequence(parts) => RefScan::Sequence(
            parts
                .iter()
                .map(|part| shift_scan(part, base, owner))
                .collect::<Result<Vec<_>, _>>()?,
        ),
        RefScan::Array { .. } => {
            return Err(CodegenError(format!(
                "{owner} value scan cannot contain a variable object scan"
            )));
        }
    })
}

pub(super) fn sequence_scans(scans: impl IntoIterator<Item = RefScan>) -> RefScan {
    fn collect(scan: RefScan, references: &mut Vec<u64>) {
        match scan {
            RefScan::None => {}
            RefScan::References(offsets) => references.extend(offsets),
            RefScan::Sequence(parts) => {
                for part in parts {
                    collect(part, references);
                }
            }
            RefScan::Array { .. } => {
                unreachable!("Scoop ABI values cannot contain variable object scans")
            }
        }
    }

    let mut references = Vec::new();
    for scan in scans {
        collect(scan, &mut references);
    }
    if references.is_empty() {
        RefScan::None
    } else {
        RefScan::References(references)
    }
}

pub(super) fn abi_metadata_error(
    owner: &str,
    ty: &LirType,
    actual_size: u64,
    actual_align: u64,
    actual_scan: &RefScan,
    expected: &StorageFacts,
) -> CodegenError {
    CodegenError(format!(
        "{owner} ABI metadata for {} is size/alignment {actual_size}/{actual_align} with scan {}, expected {}/{} with scan {} from the module target layout",
        ty.dump(),
        actual_scan.dump(),
        expected.size,
        expected.align,
        expected.scan.dump()
    ))
}

pub(super) const fn scoop_abi_passing_name(passing: scoop_lir::ScoopAbiPassing) -> &'static str {
    match passing {
        scoop_lir::ScoopAbiPassing::Direct => "direct",
        scoop_lir::ScoopAbiPassing::DirectParts => "direct-parts",
        scoop_lir::ScoopAbiPassing::Indirect => "indirect",
    }
}
