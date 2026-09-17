//! Defensive validation of the fully classified Scoop call ABI.
//!
//! Classification belongs to MIR -> LIR lowering.  This module only checks
//! that definitions, declarations and call sites preserve the exact typed
//! conventions already present in LIR.

use super::super::*;
use scoop_lir::{EnumDefId, StructDefId};
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq)]
struct StorageFacts {
    size: u64,
    align: u64,
    scan: RefScan,
}

pub(super) fn canonical_storage_scan(
    module: &Module,
    ty: &LirType,
    owner: &str,
) -> Result<RefScan, CodegenError> {
    AbiMetadataValidator::new(module)
        .storage_facts(ty, owner)
        .map(|facts| facts.scan)
}

struct AbiMetadataValidator<'a> {
    module: &'a Module,
    visiting_structs: HashSet<StructDefId>,
    visiting_enums: HashSet<EnumDefId>,
}

impl<'a> AbiMetadataValidator<'a> {
    fn new(module: &'a Module) -> Self {
        Self {
            module,
            visiting_structs: HashSet::new(),
            visiting_enums: HashSet::new(),
        }
    }

    fn validate_all(mut self) -> Result<(), CodegenError> {
        for function in &self.module.functions {
            self.validate_scoop_signature(
                &function.signature,
                &format!("function @{}", function.symbol()),
            )?;
            self.validate_call_signatures(function)?;
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

    fn validate_call_signatures(&mut self, function: &Function) -> Result<(), CodegenError> {
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
            self.validate_value(signature.result(), &result_owner)?;
            self.validate_passing(
                signature.result(),
                scoop_lir::ScoopAbiPassing::Direct,
                &result_owner,
            )?;
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

    fn validate_scoop_signature(
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
                self.validate_value(result, &result_owner)?;
                self.validate_passing(result, scoop_lir::ScoopAbiPassing::Direct, &result_owner)
            }
            scoop_lir::AbiReturn::Indirect(result) => {
                let result_owner = format!("{owner} result");
                self.validate_value(result, &result_owner)?;
                self.validate_passing(result, scoop_lir::ScoopAbiPassing::Indirect, &result_owner)
            }
        }
    }

    fn validate_arguments(
        &mut self,
        arguments: &[scoop_lir::AbiArgument],
        owner: &str,
    ) -> Result<(), CodegenError> {
        for (index, argument) in arguments.iter().enumerate() {
            let owner = format!("{owner} argument {index}");
            match argument {
                scoop_lir::AbiArgument::ElidedZst(value) => self.validate_zst(value, &owner)?,
                scoop_lir::AbiArgument::Direct(value) => {
                    self.validate_value(value, &owner)?;
                    self.validate_passing(value, scoop_lir::ScoopAbiPassing::Direct, &owner)?;
                }
                scoop_lir::AbiArgument::Indirect(value) => {
                    self.validate_value(value, &owner)?;
                    self.validate_passing(value, scoop_lir::ScoopAbiPassing::Indirect, &owner)?;
                }
            }
        }
        Ok(())
    }

    fn validate_passing(
        &self,
        value: &scoop_lir::AbiValue,
        actual: scoop_lir::ScoopAbiPassing,
        owner: &str,
    ) -> Result<(), CodegenError> {
        let expected = scoop_lir::classify_non_zero_scoop_abi_value(
            self.module.meta.target_profile,
            &self.module.enums,
            value.storage_type(),
        )
        .map_err(|error| {
            CodegenError(format!(
                "{owner} cannot be classified by the module target profile: {error:?}"
            ))
        })?;
        if actual != expected {
            return Err(CodegenError(format!(
                "{owner} uses {} passing for {}, but the module target profile requires {} passing",
                scoop_abi_passing_name(actual),
                value.storage_type().dump(),
                scoop_abi_passing_name(expected),
            )));
        }
        Ok(())
    }

    fn validate_zst(&mut self, value: &scoop_lir::AbiZst, owner: &str) -> Result<(), CodegenError> {
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

    fn validate_value(
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

    fn storage_facts(&mut self, ty: &LirType, owner: &str) -> Result<StorageFacts, CodegenError> {
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
            LirType::I64 | LirType::MachineScalar(_) => scalar(scoop_lir::BackendScalarKind::I64),
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

    fn aggregate_facts(
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

    fn struct_facts(&mut self, id: StructDefId, owner: &str) -> Result<StorageFacts, CodegenError> {
        let index = arena_index(id);
        if index >= self.module.structs.len() {
            return Err(CodegenError(format!(
                "{owner} references invalid struct definition {index}"
            )));
        }
        if !self.visiting_structs.insert(id) {
            return Err(CodegenError(format!(
                "{owner} reaches a recursive by-value struct definition {index}"
            )));
        }
        let result = self.compute_struct_facts(id, owner);
        self.visiting_structs.remove(&id);
        result
    }

    fn compute_struct_facts(
        &mut self,
        id: StructDefId,
        owner: &str,
    ) -> Result<StorageFacts, CodegenError> {
        enum StructShape {
            Fields {
                fields: Vec<(LirType, scoop_lir::FieldLayout)>,
                packed: Option<u64>,
                explicit_align: Option<u64>,
            },
            Intrinsic(scoop_lir::IntrinsicTypeRepresentation),
        }

        let (name, stored_size, stored_align, shape) = {
            let definition = &self.module.structs[id];
            let shape = match &definition.representation {
                StructRepresentation::Scoop { fields } => StructShape::Fields {
                    fields: fields
                        .iter()
                        .map(|field| (field.ty.clone(), field.layout))
                        .collect(),
                    packed: None,
                    explicit_align: None,
                },
                StructRepresentation::C { contract, fields } => StructShape::Fields {
                    fields: fields
                        .iter()
                        .map(|field| (field.ty.storage_type(), field.layout))
                        .collect(),
                    packed: contract.packed.bytes(),
                    explicit_align: contract.aligned.bytes(),
                },
                StructRepresentation::Intrinsic(representation) => {
                    StructShape::Intrinsic(representation.clone())
                }
            };
            (
                definition.name.clone(),
                definition.size,
                definition.align,
                shape,
            )
        };

        let expected = match shape {
            StructShape::Fields {
                fields,
                packed,
                explicit_align,
            } => {
                let mut field_facts = Vec::with_capacity(fields.len());
                for (index, (ty, _)) in fields.iter().enumerate() {
                    field_facts.push(
                        self.storage_facts(ty, &format!("{owner} struct `{name}` field {index}"))?,
                    );
                }
                let mut cursor = 0;
                let mut align = explicit_align.unwrap_or(1);
                let mut scans = Vec::new();
                for (index, ((_, stored), facts)) in fields.iter().zip(&field_facts).enumerate() {
                    let access_align = packed.map_or(facts.align, |cap| facts.align.min(cap));
                    let offset = checked_align_up(cursor, access_align, owner)?;
                    if stored.offset != offset || stored.access_align != access_align {
                        return Err(CodegenError(format!(
                            "{owner} struct `{name}` field {index} layout {}/{} disagrees with target layout {offset}/{access_align}",
                            stored.offset, stored.access_align
                        )));
                    }
                    scans.push(shift_scan(&facts.scan, offset, owner)?);
                    cursor = offset.checked_add(facts.size).ok_or_else(|| {
                        CodegenError(format!("{owner} struct `{name}` layout overflows u64"))
                    })?;
                    align = align.max(access_align);
                }
                StorageFacts {
                    size: checked_align_up(cursor, align, owner)?,
                    align,
                    scan: sequence_scans(scans),
                }
            }
            StructShape::Intrinsic(representation) => {
                let profile = self.module.meta.target_profile;
                let layout = match representation {
                    scoop_lir::IntrinsicTypeRepresentation::Integer(kind) => {
                        profile.integer_layout(kind)
                    }
                    scoop_lir::IntrinsicTypeRepresentation::Boolean => {
                        profile.scalar_layout(scoop_lir::BackendScalarKind::I1)
                    }
                    scoop_lir::IntrinsicTypeRepresentation::Ptr { .. } => {
                        profile.pointer_layout(PointerKind::Raw)
                    }
                    scoop_lir::IntrinsicTypeRepresentation::FunPtr { .. } => {
                        profile.pointer_layout(PointerKind::Code)
                    }
                    scoop_lir::IntrinsicTypeRepresentation::String => {
                        return Err(CodegenError(format!(
                            "{owner} uses intrinsic String declaration `{name}` as value storage"
                        )));
                    }
                };
                StorageFacts {
                    size: layout.size_bytes(),
                    align: layout.alignment_bytes(),
                    scan: RefScan::None,
                }
            }
        };
        if (stored_size, stored_align) != (expected.size, expected.align) {
            return Err(CodegenError(format!(
                "{owner} struct `{name}` layout {stored_size}/{stored_align} disagrees with target layout {}/{}",
                expected.size, expected.align
            )));
        }
        Ok(expected)
    }

    fn enum_facts(&mut self, id: EnumDefId, owner: &str) -> Result<StorageFacts, CodegenError> {
        let index = arena_index(id);
        if index >= self.module.enums.len() {
            return Err(CodegenError(format!(
                "{owner} references invalid enum definition {index}"
            )));
        }
        if !self.visiting_enums.insert(id) {
            return Err(CodegenError(format!(
                "{owner} reaches a recursive by-value enum definition {index}"
            )));
        }
        let result = self.compute_enum_facts(id, owner);
        self.visiting_enums.remove(&id);
        result
    }

    fn compute_enum_facts(
        &mut self,
        id: EnumDefId,
        owner: &str,
    ) -> Result<StorageFacts, CodegenError> {
        struct VariantShape {
            fields: Vec<(LirType, u64)>,
            slot_offset: u64,
            slot_size: u64,
            slot_align: u64,
            gc_free: bool,
        }
        enum EnumShape {
            Niche(scoop_lir::NichePointerKind),
            Tagged {
                variants: Vec<VariantShape>,
                size: u64,
                align: u64,
            },
        }

        let (name, stored_scan, shape) = {
            let definition = &self.module.enums[id];
            let shape = match &definition.repr {
                EnumRepr::Niche { kind, .. } => EnumShape::Niche(*kind),
                EnumRepr::Tagged {
                    variants,
                    size,
                    align,
                } => EnumShape::Tagged {
                    variants: variants
                        .iter()
                        .map(|variant| VariantShape {
                            fields: variant
                                .fields
                                .iter()
                                .map(|field| (field.ty.clone(), field.offset))
                                .collect(),
                            slot_offset: variant.slot_offset,
                            slot_size: variant.slot_size,
                            slot_align: variant.slot_align,
                            gc_free: variant.gc_free,
                        })
                        .collect(),
                    size: *size,
                    align: *align,
                },
            };
            (definition.name.clone(), definition.scan.clone(), shape)
        };

        let expected = match shape {
            EnumShape::Niche(kind) => {
                let layout = self
                    .module
                    .meta
                    .target_profile
                    .pointer_layout(kind.pointer_kind());
                StorageFacts {
                    size: layout.size_bytes(),
                    align: layout.alignment_bytes(),
                    scan: if kind == scoop_lir::NichePointerKind::Managed {
                        RefScan::References(vec![0])
                    } else {
                        RefScan::None
                    },
                }
            }
            EnumShape::Tagged {
                variants,
                size: stored_size,
                align: stored_align,
            } => {
                struct ComputedVariant {
                    fields: Vec<StorageFacts>,
                    offsets: Vec<u64>,
                    size: u64,
                    align: u64,
                }
                let mut computed = Vec::with_capacity(variants.len());
                for (variant_index, variant) in variants.iter().enumerate() {
                    let mut fields = Vec::with_capacity(variant.fields.len());
                    for (field_index, (ty, _)) in variant.fields.iter().enumerate() {
                        fields.push(self.storage_facts(
                            ty,
                            &format!(
                                "{owner} enum `{name}` variant {variant_index} field {field_index}"
                            ),
                        )?);
                    }
                    let (offsets, size, align) = aggregate_layout(
                        fields.iter().map(|field| (field.size, field.align)),
                        owner,
                    )?;
                    if (variant.slot_size, variant.slot_align) != (size, align) {
                        return Err(CodegenError(format!(
                            "{owner} enum `{name}` variant {variant_index} slot layout {}/{} disagrees with target layout {size}/{align}",
                            variant.slot_size, variant.slot_align
                        )));
                    }
                    let expected_gc_free = fields.iter().all(|field| field.scan == RefScan::None);
                    if variant.gc_free != expected_gc_free {
                        return Err(CodegenError(format!(
                            "{owner} enum `{name}` variant {variant_index} gc_free metadata disagrees with its fields"
                        )));
                    }
                    computed.push(ComputedVariant {
                        fields,
                        offsets,
                        size,
                        align,
                    });
                }

                let pure_size = variants
                    .iter()
                    .zip(&computed)
                    .filter(|(variant, _)| variant.gc_free)
                    .map(|(_, variant)| variant.size)
                    .max()
                    .unwrap_or(0);
                let pure_align = variants
                    .iter()
                    .zip(&computed)
                    .filter(|(variant, _)| variant.gc_free)
                    .map(|(_, variant)| variant.align)
                    .max()
                    .unwrap_or(1);
                let tag = self
                    .module
                    .meta
                    .target_profile
                    .scalar_layout(scoop_lir::BackendScalarKind::I64);
                let pure_offset = checked_align_up(tag.size_bytes(), pure_align, owner)?;
                let mut cursor = pure_offset.checked_add(pure_size).ok_or_else(|| {
                    CodegenError(format!("{owner} enum `{name}` layout overflows u64"))
                })?;
                let mut align = tag.alignment_bytes().max(pure_align);
                let mut scans = Vec::new();
                for (variant_index, (variant, computed)) in
                    variants.iter().zip(&computed).enumerate()
                {
                    let slot_offset = if variant.gc_free {
                        pure_offset
                    } else {
                        cursor = checked_align_up(cursor, computed.align, owner)?;
                        let offset = cursor;
                        cursor = cursor.checked_add(computed.size).ok_or_else(|| {
                            CodegenError(format!("{owner} enum `{name}` layout overflows u64"))
                        })?;
                        offset
                    };
                    if variant.slot_offset != slot_offset {
                        return Err(CodegenError(format!(
                            "{owner} enum `{name}` variant {variant_index} slot offset {} disagrees with target offset {slot_offset}",
                            variant.slot_offset
                        )));
                    }
                    align = align.max(computed.align);
                    for (field_index, (((_, stored_offset), facts), relative_offset)) in variant
                        .fields
                        .iter()
                        .zip(&computed.fields)
                        .zip(&computed.offsets)
                        .enumerate()
                    {
                        let offset =
                            slot_offset.checked_add(*relative_offset).ok_or_else(|| {
                                CodegenError(format!(
                                    "{owner} enum `{name}` field offset overflows u64"
                                ))
                            })?;
                        if *stored_offset != offset {
                            return Err(CodegenError(format!(
                                "{owner} enum `{name}` variant {variant_index} field {field_index} offset {stored_offset} disagrees with target offset {offset}"
                            )));
                        }
                        scans.push(shift_scan(&facts.scan, offset, owner)?);
                    }
                }
                let size = checked_align_up(cursor, align, owner)?;
                if (stored_size, stored_align) != (size, align) {
                    return Err(CodegenError(format!(
                        "{owner} enum `{name}` layout {stored_size}/{stored_align} disagrees with target layout {size}/{align}"
                    )));
                }
                StorageFacts {
                    size,
                    align,
                    scan: sequence_scans(scans),
                }
            }
        };
        if stored_scan != expected.scan {
            return Err(CodegenError(format!(
                "{owner} enum `{name}` scan {} disagrees with field-derived scan {}",
                stored_scan.dump(),
                expected.scan.dump()
            )));
        }
        Ok(expected)
    }
}

fn checked_align_up(value: u64, align: u64, owner: &str) -> Result<u64, CodegenError> {
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

fn aggregate_layout(
    fields: impl IntoIterator<Item = (u64, u64)>,
    owner: &str,
) -> Result<(Vec<u64>, u64, u64), CodegenError> {
    let mut offsets = Vec::new();
    let mut size = 0;
    let mut align = 1;
    for (field_size, field_align) in fields {
        size = checked_align_up(size, field_align, owner)?;
        offsets.push(size);
        size = size
            .checked_add(field_size)
            .ok_or_else(|| CodegenError(format!("{owner} aggregate layout overflows u64")))?;
        align = align.max(field_align);
    }
    Ok((offsets, checked_align_up(size, align, owner)?, align))
}

fn shift_scan(scan: &RefScan, base: u64, owner: &str) -> Result<RefScan, CodegenError> {
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

fn sequence_scans(scans: impl IntoIterator<Item = RefScan>) -> RefScan {
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

fn abi_metadata_error(
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

const fn scoop_abi_passing_name(passing: scoop_lir::ScoopAbiPassing) -> &'static str {
    match passing {
        scoop_lir::ScoopAbiPassing::Direct => "direct",
        scoop_lir::ScoopAbiPassing::Indirect => "indirect",
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CallProtocol {
    Managed,
    NoGc,
    NativeSafe,
    NativeBorrowed,
}

impl CallProtocol {
    const fn name(self) -> &'static str {
        match self {
            Self::Managed => "managed",
            Self::NoGc => "no-gc",
            Self::NativeSafe => "native-safe",
            Self::NativeBorrowed => "native-borrowed",
        }
    }
}

pub(super) fn validate_scoop_abi(module: &Module) -> Result<(), CodegenError> {
    AbiMetadataValidator::new(module).validate_all()?;
    validate_scoop_extern_declarations(module)?;

    for function in &module.functions {
        validate_target_signature_references(function)?;
        for (_, block) in function.blocks.iter() {
            for instruction in &block.instructions {
                match instruction {
                    Instruction::ManagedPoll { site } => validate_managed_poll(function, site)?,
                    Instruction::Call { site } => validate_call_site(module, function, site)?,
                    Instruction::Invoke { site } => validate_invoke_site(module, function, site)?,
                    _ => {}
                }
            }
        }
    }
    Ok(())
}

fn validate_managed_poll(
    function: &Function,
    site: &scoop_lir::ManagedPollSite,
) -> Result<(), CodegenError> {
    if function.gc_effect != scoop_lir::GcEffect::Managed {
        return Err(managed_poll_error(
            function,
            "is only valid in a managed function",
        ));
    }

    let targets = &function.call_targets;
    let target_index = arena_index(site.target);
    if target_index >= targets.managed_targets.void.len() {
        return Err(managed_poll_error(
            function,
            format!("references invalid managed-void target {target_index}"),
        ));
    }
    let target = &targets.managed_targets.void[site.target];
    if target.destination
        != scoop_lir::ManagedCallDestination::runtime(scoop_lir::ManagedRuntimeFunction::Safepoint)
    {
        return Err(managed_poll_error(
            function,
            "target is not the managed safepoint runtime function",
        ));
    }

    let signature_index = arena_index(target.signature);
    if signature_index >= targets.void_signatures.len() {
        return Err(managed_poll_error(
            function,
            format!("target references invalid void signature {signature_index}"),
        ));
    }
    let signature = &targets.void_signatures[target.signature];
    if !signature.arguments().is_empty()
        || signature.calling_convention() != scoop_lir::CallingConvention::Cdecl
    {
        return Err(managed_poll_error(
            function,
            "target must use the exact `cdecl () -> void` safepoint ABI",
        ));
    }
    Ok(())
}

fn validate_target_signature_references(function: &Function) -> Result<(), CodegenError> {
    let targets = &function.call_targets;
    validate_protocol_target_signature_references(function, &targets.managed_targets)?;
    validate_protocol_target_signature_references(function, &targets.no_gc_targets)?;
    validate_protocol_target_signature_references(function, &targets.native_safe_targets)?;
    validate_protocol_target_signature_references(function, &targets.native_borrowed_targets)
}

fn validate_protocol_target_signature_references<Destination>(
    function: &Function,
    targets: &scoop_lir::ProtocolCallTargets<Destination>,
) -> Result<(), CodegenError> {
    let signatures = &function.call_targets;
    for (_, target) in targets.void.iter() {
        let index = arena_index(target.signature);
        if index >= signatures.void_signatures.len() {
            return Err(call_error(
                function,
                format!("target references invalid void signature {index}"),
            ));
        }
    }
    for (_, target) in targets.elided_zst.iter() {
        let index = arena_index(target.signature);
        if index >= signatures.elided_zst_signatures.len() {
            return Err(call_error(
                function,
                format!("target references invalid elided-ZST signature {index}"),
            ));
        }
    }
    for (_, target) in targets.direct.iter() {
        let index = arena_index(target.signature);
        if index >= signatures.direct_signatures.len() {
            return Err(call_error(
                function,
                format!("target references invalid direct-result signature {index}"),
            ));
        }
    }
    for (_, target) in targets.indirect_result.iter() {
        let index = arena_index(target.signature);
        if index >= signatures.indirect_result_signatures.len() {
            return Err(call_error(
                function,
                format!("target references invalid indirect-result signature {index}"),
            ));
        }
    }
    Ok(())
}

fn validate_scoop_extern_declarations(module: &Module) -> Result<(), CodegenError> {
    for (_, function) in module.extern_functions.iter() {
        let ExternFunctionKind::Scoop { signature, .. } = &function.kind else {
            continue;
        };
        if function.calling_convention != signature.calling_convention() {
            return Err(CodegenError(format!(
                "Scoop extern `{}` carries a calling convention that disagrees with its authoritative ABI signature",
                function.source_name
            )));
        }
    }
    Ok(())
}

fn validate_call_site(
    module: &Module,
    function: &Function,
    site: &scoop_lir::CallSite,
) -> Result<(), CodegenError> {
    let targets = &function.call_targets;
    match site {
        scoop_lir::CallSite::Managed(site) => {
            let call = checked_call_view(
                function,
                &site.call,
                &targets.managed_targets,
                scoop_lir::ManagedCallDestination::view,
            )?;
            validate_call(module, function, call, CallProtocol::Managed)
        }
        scoop_lir::CallSite::NoGc(site) => {
            let call = checked_call_view(
                function,
                &site.call,
                &targets.no_gc_targets,
                scoop_lir::NoGcCallDestination::view,
            )?;
            validate_call(module, function, call, CallProtocol::NoGc)
        }
        scoop_lir::CallSite::NativeSafe(site) => {
            let call = checked_call_view(
                function,
                &site.call,
                &targets.native_safe_targets,
                scoop_lir::NativeSafeCallDestination::view,
            )?;
            validate_call(module, function, call, CallProtocol::NativeSafe)
        }
        scoop_lir::CallSite::NativeBorrowed(site) => {
            // Native-borrowed calls are sealed by `CallTargets`: construction
            // already checked the target/signature ids and result publication.
            let call = site.call.view(targets);
            validate_native_borrowed_publication(function, &call)?;
            validate_call(module, function, call.call, CallProtocol::NativeBorrowed)
        }
    }
}

fn validate_invoke_site(
    module: &Module,
    function: &Function,
    site: &scoop_lir::InvokeSite,
) -> Result<(), CodegenError> {
    let targets = &function.call_targets;
    match site {
        scoop_lir::InvokeSite::Managed(site) => {
            let call = checked_call_view(
                function,
                &site.call,
                &targets.managed_targets,
                scoop_lir::ManagedCallDestination::view,
            )?;
            validate_call(module, function, call, CallProtocol::Managed)
        }
        scoop_lir::InvokeSite::NoGc(site) => {
            let call = checked_call_view(
                function,
                &site.call,
                &targets.no_gc_targets,
                scoop_lir::NoGcCallDestination::view,
            )?;
            validate_call(module, function, call, CallProtocol::NoGc)
        }
    }
}

fn checked_call_view<'a, Destination: Copy>(
    function: &'a Function,
    call: &'a scoop_lir::TypedCall<Destination>,
    targets: &'a scoop_lir::ProtocolCallTargets<Destination>,
    destination_view: fn(Destination) -> scoop_lir::CallDestination,
) -> Result<scoop_lir::TypedCallView<'a>, CodegenError> {
    let all = &function.call_targets;
    let invalid_target = |convention: &str, index: usize| {
        CodegenError(format!(
            "typed call @{} references invalid {convention} target {index}",
            function.symbol()
        ))
    };
    let invalid_signature = |convention: &str, index: usize| {
        CodegenError(format!(
            "typed call @{} references invalid {convention} signature {index}",
            function.symbol()
        ))
    };

    match call {
        scoop_lir::TypedCall::Void { target, .. } => {
            let index = arena_index(*target);
            if index >= targets.void.len() {
                return Err(invalid_target("void", index));
            }
            let signature = targets.void[*target].signature;
            let index = arena_index(signature);
            if index >= all.void_signatures.len() {
                return Err(invalid_signature("void", index));
            }
        }
        scoop_lir::TypedCall::ElidedZst { target, .. } => {
            let index = arena_index(*target);
            if index >= targets.elided_zst.len() {
                return Err(invalid_target("elided-ZST", index));
            }
            let signature = targets.elided_zst[*target].signature;
            let index = arena_index(signature);
            if index >= all.elided_zst_signatures.len() {
                return Err(invalid_signature("elided-ZST", index));
            }
        }
        scoop_lir::TypedCall::Direct { target, .. } => {
            let index = arena_index(*target);
            if index >= targets.direct.len() {
                return Err(invalid_target("direct-result", index));
            }
            let signature = targets.direct[*target].signature;
            let index = arena_index(signature);
            if index >= all.direct_signatures.len() {
                return Err(invalid_signature("direct-result", index));
            }
        }
        scoop_lir::TypedCall::IndirectResult { target, .. } => {
            let index = arena_index(*target);
            if index >= targets.indirect_result.len() {
                return Err(invalid_target("indirect-result", index));
            }
            let signature = targets.indirect_result[*target].signature;
            let index = arena_index(signature);
            if index >= all.indirect_result_signatures.len() {
                return Err(invalid_signature("indirect-result", index));
            }
        }
    }

    Ok(all.typed_call_view(call, targets, destination_view))
}

fn validate_call(
    module: &Module,
    function: &Function,
    call: scoop_lir::TypedCallView<'_>,
    protocol: CallProtocol,
) -> Result<(), CodegenError> {
    let expected_arguments = call.arguments();
    let actual_arguments = call.args();
    if actual_arguments.len() != expected_arguments.len() {
        return Err(call_error(
            function,
            format!(
                "signature has {} logical arguments but call has {}",
                expected_arguments.len(),
                actual_arguments.len()
            ),
        ));
    }

    for (index, (actual, expected)) in actual_arguments.iter().zip(expected_arguments).enumerate() {
        validate_argument(module, function, index, *actual, expected)?;
    }
    validate_result(function, &call)?;
    validate_destination(module, function, &call, protocol)?;

    let is_c_extern = matches!(
        call.destination(),
        scoop_lir::CallDestination::Extern(id)
            if extern_declaration(module, function, id).is_ok_and(|declaration| {
                matches!(declaration.kind, ExternFunctionKind::C { .. })
            })
    );
    for argument in actual_arguments {
        if matches!(
            argument,
            scoop_lir::AbiCallArgument::Direct(Value::CArgumentStorage(_))
        ) && !is_c_extern
        {
            return Err(call_error(
                function,
                "uses a C argument-storage address outside a C extern call",
            ));
        }
    }
    Ok(())
}

fn validate_argument(
    module: &Module,
    function: &Function,
    index: usize,
    actual: scoop_lir::AbiCallArgument,
    expected: &scoop_lir::AbiArgument,
) -> Result<(), CodegenError> {
    let (value, expected_type) = match (actual, expected) {
        (
            scoop_lir::AbiCallArgument::ElidedZst(value),
            scoop_lir::AbiArgument::ElidedZst(expected),
        ) => (value, expected.storage_type()),
        (scoop_lir::AbiCallArgument::Direct(value), scoop_lir::AbiArgument::Direct(expected)) => {
            (value, expected.storage_type())
        }
        (
            scoop_lir::AbiCallArgument::Indirect(storage),
            scoop_lir::AbiArgument::Indirect(expected),
        ) => {
            let local = storage.local();
            let local_index = arena_index(local);
            if local_index >= function.locals.len() {
                return Err(call_error(
                    function,
                    format!("indirect argument {index} references invalid local {local_index}"),
                ));
            }
            let actual_type = &function.locals[local].ty;
            if actual_type != expected.storage_type() {
                return Err(call_error(
                    function,
                    format!(
                        "indirect argument {index} storage local{local_index} has type {}, expected exact {}",
                        actual_type.dump(),
                        expected.storage_type().dump()
                    ),
                ));
            }
            return Ok(());
        }
        (actual, expected) => {
            return Err(call_error(
                function,
                format!(
                    "argument {index} uses {} passing, expected {}",
                    call_argument_convention(actual),
                    argument_convention(expected)
                ),
            ));
        }
    };

    let actual_type = super::checked_value_type(
        module,
        function,
        value,
        &format!("typed call argument {index}"),
    )?;
    if &actual_type != expected_type {
        return Err(call_error(
            function,
            format!(
                "argument {index} has type {}, expected exact {}",
                actual_type.dump(),
                expected_type.dump()
            ),
        ));
    }
    Ok(())
}

fn validate_result(
    function: &Function,
    call: &scoop_lir::TypedCallView<'_>,
) -> Result<(), CodegenError> {
    match call {
        scoop_lir::TypedCallView::Void { .. } => Ok(()),
        scoop_lir::TypedCallView::ElidedZst { signature, out, .. } => require_temp_type(
            function,
            *out,
            signature.result().storage_type(),
            "elided-ZST",
        ),
        scoop_lir::TypedCallView::Direct { signature, out, .. } => {
            require_temp_type(function, *out, signature.result().storage_type(), "direct")
        }
        scoop_lir::TypedCallView::IndirectResult {
            signature, storage, ..
        } => require_local_type(
            function,
            *storage,
            signature.result().storage_type(),
            "indirect result",
        ),
    }
}

fn require_temp_type(
    function: &Function,
    temp: TempId,
    expected: &LirType,
    convention: &str,
) -> Result<(), CodegenError> {
    let actual = super::checked_temp_type(function, temp, "typed call result")?;
    if actual != expected {
        return Err(call_error(
            function,
            format!(
                "{convention} result temporary has type {}, expected exact {}",
                actual.dump(),
                expected.dump()
            ),
        ));
    }
    Ok(())
}

fn require_local_type(
    function: &Function,
    local: scoop_lir::LocalId,
    expected: &LirType,
    owner: &str,
) -> Result<(), CodegenError> {
    let index = arena_index(local);
    if index >= function.locals.len() {
        return Err(call_error(
            function,
            format!("{owner} references invalid local {index}"),
        ));
    }
    let actual = &function.locals[local].ty;
    if actual != expected {
        return Err(call_error(
            function,
            format!(
                "{owner} local{index} has type {}, expected exact {}",
                actual.dump(),
                expected.dump()
            ),
        ));
    }
    Ok(())
}

fn validate_destination(
    module: &Module,
    function: &Function,
    call: &scoop_lir::TypedCallView<'_>,
    protocol: CallProtocol,
) -> Result<(), CodegenError> {
    let convention = indirect_result_convention(call);
    if convention == Some(scoop_lir::IndirectResultConvention::CStoragePointer)
        && protocol != CallProtocol::NativeSafe
    {
        return Err(call_error(
            function,
            "C storage result pointer is only valid for a native-safe C bridge call",
        ));
    }

    match call.destination() {
        scoop_lir::CallDestination::Local(id) => {
            let Some(declaration) = module.functions.get(id.into_u32() as usize) else {
                return Err(call_error(
                    function,
                    format!("references invalid local function id {}", id.into_u32()),
                ));
            };
            let expected_protocol = match declaration.gc_effect {
                scoop_lir::GcEffect::Managed => CallProtocol::Managed,
                scoop_lir::GcEffect::NoGc => CallProtocol::NoGc,
            };
            if protocol != expected_protocol {
                return Err(call_error(
                    function,
                    format!(
                        "{} protocol does not match @{}'s {:?} effect",
                        protocol.name(),
                        declaration.symbol(),
                        declaration.gc_effect
                    ),
                ));
            }
            require_scoop_signature(
                function,
                call,
                &declaration.signature,
                &format!("typed local call to @{}", declaration.symbol()),
            )
        }
        scoop_lir::CallDestination::CoreExternal(id) => {
            let index = arena_index(id);
            if index >= module.meta.core_external_callables.len() {
                return Err(call_error(
                    function,
                    format!("references invalid core external callable {index}"),
                ));
            }
            let declaration = &module.meta.core_external_callables[id];
            let expected_protocol = match declaration.gc_effect() {
                scoop_lir::GcEffect::Managed => CallProtocol::Managed,
                scoop_lir::GcEffect::NoGc => CallProtocol::NoGc,
            };
            if protocol != expected_protocol {
                return Err(call_error(
                    function,
                    format!(
                        "{} protocol does not match core external `{}`'s {:?} effect",
                        protocol.name(),
                        declaration.expected_symbol().symbol(),
                        declaration.gc_effect()
                    ),
                ));
            }
            require_scoop_signature(
                function,
                call,
                declaration.signature(),
                &format!(
                    "typed core external call to `{}`",
                    declaration.expected_symbol().symbol()
                ),
            )
        }
        scoop_lir::CallDestination::DependencyExternal(id) => {
            let index = arena_index(id);
            if index >= module.meta.dependency_external_callables.len() {
                return Err(call_error(
                    function,
                    format!("references invalid dependency external callable {index}"),
                ));
            }
            let declaration = &module.meta.dependency_external_callables[id];
            let expected_protocol = match declaration.gc_effect() {
                scoop_lir::GcEffect::Managed => CallProtocol::Managed,
                scoop_lir::GcEffect::NoGc => CallProtocol::NoGc,
            };
            if protocol != expected_protocol {
                return Err(call_error(
                    function,
                    format!(
                        "{} protocol does not match dependency external `{}`'s {:?} effect",
                        protocol.name(),
                        declaration.expected_symbol().symbol(),
                        declaration.gc_effect()
                    ),
                ));
            }
            require_scoop_signature(
                function,
                call,
                declaration.signature(),
                &format!(
                    "typed dependency external call to `{}`",
                    declaration.expected_symbol().symbol()
                ),
            )
        }
        scoop_lir::CallDestination::Extern(id) => {
            let declaration = extern_declaration(module, function, id)?;
            match &declaration.kind {
                ExternFunctionKind::C { signature, .. } => {
                    validate_c_extern_call(function, call, protocol, declaration, signature)
                }
                ExternFunctionKind::Scoop { signature, .. } => {
                    if protocol != CallProtocol::NativeBorrowed {
                        return Err(call_error(
                            function,
                            format!(
                                "Scoop extern `{}` requires the native-borrowed protocol",
                                declaration.source_name
                            ),
                        ));
                    }
                    require_scoop_signature(
                        function,
                        call,
                        signature,
                        &format!("Scoop extern `{}`", declaration.source_name),
                    )
                }
            }
        }
        scoop_lir::CallDestination::Runtime(runtime) => {
            validate_runtime_call(function, call, protocol, runtime)
        }
        scoop_lir::CallDestination::Dispatch { .. } => {
            if matches!(
                protocol,
                CallProtocol::NativeSafe | CallProtocol::NativeBorrowed
            ) {
                return Err(call_error(
                    function,
                    "dynamic dispatch cannot use a native transition protocol",
                ));
            }
            if convention == Some(scoop_lir::IndirectResultConvention::CStoragePointer) {
                return Err(call_error(
                    function,
                    "dynamic dispatch cannot use a C storage result pointer",
                ));
            }
            Ok(())
        }
    }
}

fn extern_declaration<'a>(
    module: &'a Module,
    function: &Function,
    id: scoop_lir::ExternFunctionId,
) -> Result<&'a scoop_lir::ExternFunction, CodegenError> {
    let index = arena_index(id);
    if index >= module.extern_functions.iter().count() {
        return Err(call_error(
            function,
            format!("references invalid extern function id {index}"),
        ));
    }
    Ok(&module.extern_functions[id])
}

fn require_scoop_signature(
    function: &Function,
    call: &scoop_lir::TypedCallView<'_>,
    expected: &scoop_lir::ScoopAbiSignature,
    callee: &str,
) -> Result<(), CodegenError> {
    let matches = call.arguments() == expected.arguments()
        && call_calling_convention(call) == expected.calling_convention()
        && match (call, expected.result()) {
            (scoop_lir::TypedCallView::Void { .. }, scoop_lir::AbiReturn::UnitVoid) => true,
            (
                scoop_lir::TypedCallView::ElidedZst { signature, .. },
                scoop_lir::AbiReturn::ElidedZst(result),
            ) => signature.result() == result,
            (
                scoop_lir::TypedCallView::Direct { signature, .. },
                scoop_lir::AbiReturn::Direct(result),
            ) => signature.result() == result,
            (
                scoop_lir::TypedCallView::IndirectResult { signature, .. },
                scoop_lir::AbiReturn::Indirect(result),
            ) => {
                signature.convention() == scoop_lir::IndirectResultConvention::ScoopSret
                    && signature.result() == result
            }
            _ => false,
        };
    if !matches {
        let result = expected
            .result()
            .logical_storage_type()
            .map(LirType::dump)
            .unwrap_or_else(|| "void".to_string());
        return Err(call_error(
            function,
            format!(
                "{callee} signature or physical convention does not match its authoritative Scoop declaration returning {result}"
            ),
        ));
    }
    Ok(())
}

fn validate_c_extern_call(
    function: &Function,
    call: &scoop_lir::TypedCallView<'_>,
    protocol: CallProtocol,
    declaration: &scoop_lir::ExternFunction,
    c_signature: &scoop_lir::CFunctionType,
) -> Result<(), CodegenError> {
    if protocol != CallProtocol::NativeSafe {
        return Err(call_error(
            function,
            format!(
                "C extern `{}` requires the native-safe protocol",
                declaration.source_name
            ),
        ));
    }
    if call_calling_convention(call) != declaration.calling_convention {
        return Err(call_error(
            function,
            format!(
                "C extern `{}` calling convention disagrees with its declaration",
                declaration.source_name
            ),
        ));
    }
    if call.arguments().len() != c_signature.params.len()
        || call.arguments().iter().any(|argument| {
            !matches!(argument, scoop_lir::AbiArgument::Direct(value)
                if value.storage_type() == &scoop_lir::RAW_PTR
                    && value.scan() == &RefScan::None)
        })
    {
        return Err(call_error(
            function,
            format!(
                "C extern `{}` bridge parameters must be direct raw storage pointers",
                declaration.source_name
            ),
        ));
    }

    for (index, (argument, parameter)) in call.args().iter().zip(&c_signature.params).enumerate() {
        let scoop_lir::AbiCallArgument::Direct(Value::CArgumentStorage(storage)) = argument else {
            return Err(call_error(
                function,
                format!(
                    "C extern `{}` argument {index} is not an exact C argument-storage address",
                    declaration.source_name
                ),
            ));
        };
        require_local_type(
            function,
            storage.local(),
            &parameter.storage_type(),
            &format!(
                "C extern `{}` argument {index} storage",
                declaration.source_name
            ),
        )?;
    }

    let result_matches = match (&c_signature.return_type, call) {
        (scoop_lir::CReturnType::Void, scoop_lir::TypedCallView::Void { .. }) => true,
        (
            scoop_lir::CReturnType::Value(_),
            scoop_lir::TypedCallView::IndirectResult { signature, .. },
        ) => {
            signature.convention() == scoop_lir::IndirectResultConvention::CStoragePointer
                && signature.result().storage_type() == &c_signature.storage_return_type()
                && signature.result().scan() == &RefScan::None
        }
        _ => false,
    };
    if !result_matches {
        return Err(call_error(
            function,
            format!(
                "C extern `{}` result does not use its exact void/storage-pointer bridge convention",
                declaration.source_name
            ),
        ));
    }
    Ok(())
}

fn validate_runtime_call(
    function: &Function,
    call: &scoop_lir::TypedCallView<'_>,
    protocol: CallProtocol,
    runtime: scoop_lir::RuntimeFunction,
) -> Result<(), CodegenError> {
    let (expected_arguments, expected_result, expected_protocol) = runtime_signature(runtime);
    let arguments_match = call.arguments().len() == expected_arguments.len()
        && call
            .arguments()
            .iter()
            .zip(&expected_arguments)
            .all(|(argument, expected)| {
                matches!(argument, scoop_lir::AbiArgument::Direct(value)
                    if value.storage_type() == expected)
            });
    let result_matches = match (expected_result.as_ref(), call) {
        (None, scoop_lir::TypedCallView::Void { .. }) => true,
        (Some(expected), scoop_lir::TypedCallView::Direct { signature, .. }) => {
            signature.result().storage_type() == expected
        }
        _ => false,
    };
    if !arguments_match || !result_matches || protocol != expected_protocol {
        return Err(call_error(
            function,
            format!(
                "has a signature or protocol outside the closed runtime ABI for `{}`",
                runtime.symbol()
            ),
        ));
    }
    Ok(())
}

fn runtime_signature(
    runtime: scoop_lir::RuntimeFunction,
) -> (Vec<LirType>, Option<LirType>, CallProtocol) {
    use scoop_lir::{ManagedRuntimeFunction as Managed, NoGcRuntimeFunction as NoGc};

    match runtime {
        scoop_lir::RuntimeFunction::Managed(function) => {
            let (arguments, result) = match function {
                Managed::Safepoint | Managed::GcCollect => (Vec::new(), None),
                Managed::Alloc => (
                    vec![
                        scoop_lir::METADATA_PTR,
                        LirType::MachineScalar(MachineScalarKind::ByteSize),
                    ],
                    Some(scoop_lir::MANAGED_PTR),
                ),
                Managed::Box => (
                    vec![
                        scoop_lir::METADATA_PTR,
                        scoop_lir::RAW_PTR,
                        LirType::MachineScalar(MachineScalarKind::ByteSize),
                        scoop_lir::METADATA_PTR,
                    ],
                    Some(scoop_lir::MANAGED_PTR),
                ),
                Managed::MaterializeException => {
                    (vec![scoop_lir::MANAGED_PTR], Some(scoop_lir::MANAGED_PTR))
                }
                Managed::StringConcat => (
                    vec![scoop_lir::MANAGED_PTR, scoop_lir::MANAGED_PTR],
                    Some(scoop_lir::MANAGED_PTR),
                ),
                Managed::InitializationEnter => (
                    vec![scoop_lir::METADATA_PTR],
                    Some(LirType::MachineScalar(
                        MachineScalarKind::InitializationOutcome,
                    )),
                ),
                Managed::InitializationSucceed => (vec![scoop_lir::METADATA_PTR], None),
                Managed::InitializationFail => {
                    (vec![scoop_lir::METADATA_PTR, scoop_lir::MANAGED_PTR], None)
                }
                Managed::InitializationFailure | Managed::InitializationCycleMessage => {
                    (vec![scoop_lir::METADATA_PTR], Some(scoop_lir::MANAGED_PTR))
                }
            };
            (arguments, result, CallProtocol::Managed)
        }
        scoop_lir::RuntimeFunction::NoGc(function) => {
            let (arguments, result) = match function {
                NoGc::IsInstance => (
                    vec![scoop_lir::MANAGED_PTR, scoop_lir::METADATA_PTR],
                    Some(LirType::I1),
                ),
                NoGc::ITableLookup => (
                    vec![scoop_lir::METADATA_PTR, scoop_lir::METADATA_PTR],
                    Some(scoop_lir::METADATA_PTR),
                ),
                NoGc::Pin | NoGc::GetHandle => (vec![scoop_lir::MANAGED_PTR], Some(LirType::I64)),
                NoGc::Unpin | NoGc::ReleaseHandle => {
                    (vec![LirType::I64], Some(scoop_lir::MANAGED_PTR))
                }
                NoGc::GcStats => (Vec::new(), Some(LirType::I64)),
                NoGc::StringCompare => (
                    vec![scoop_lir::MANAGED_PTR, scoop_lir::MANAGED_PTR],
                    Some(LirType::I64),
                ),
                NoGc::Trap => (vec![scoop_lir::RAW_PTR], None),
                NoGc::Throw => (vec![scoop_lir::MANAGED_PTR], None),
                NoGc::Rethrow => (Vec::new(), None),
            };
            (arguments, result, CallProtocol::NoGc)
        }
    }
}

fn validate_native_borrowed_publication(
    function: &Function,
    view: &scoop_lir::NativeBorrowedTypedCallView<'_>,
) -> Result<(), CodegenError> {
    match (view.result, &view.call) {
        (
            scoop_lir::NativeBorrowedResultPublication::Void,
            scoop_lir::TypedCallView::Void { .. },
        )
        | (
            scoop_lir::NativeBorrowedResultPublication::ElidedZst,
            scoop_lir::TypedCallView::ElidedZst { .. },
        ) => Ok(()),
        (
            scoop_lir::NativeBorrowedResultPublication::DirectGcFree,
            scoop_lir::TypedCallView::Direct { signature, .. },
        ) if signature.result().scan() == &RefScan::None => Ok(()),
        (
            scoop_lir::NativeBorrowedResultPublication::IndirectResultGcFree,
            scoop_lir::TypedCallView::IndirectResult { signature, .. },
        ) if signature.result().scan() == &RefScan::None => Ok(()),
        (
            scoop_lir::NativeBorrowedResultPublication::DirectRooted { storage, scan },
            scoop_lir::TypedCallView::Direct { signature, .. },
        ) if scan.as_ref_scan() == signature.result().scan() => require_local_type(
            function,
            storage,
            signature.result().storage_type(),
            "native-borrowed direct result root",
        ),
        (
            scoop_lir::NativeBorrowedResultPublication::IndirectResultRooted { storage, scan },
            scoop_lir::TypedCallView::IndirectResult {
                signature,
                storage: result_storage,
                ..
            },
        ) if storage == *result_storage && scan.as_ref_scan() == signature.result().scan() => {
            require_local_type(
                function,
                storage,
                signature.result().storage_type(),
                "native-borrowed indirect result root",
            )
        }
        _ => Err(call_error(
            function,
            "native-borrowed result publication disagrees with its typed result convention",
        )),
    }
}

fn call_calling_convention(call: &scoop_lir::TypedCallView<'_>) -> scoop_lir::CallingConvention {
    match call {
        scoop_lir::TypedCallView::Void { signature, .. } => signature.calling_convention(),
        scoop_lir::TypedCallView::ElidedZst { signature, .. } => signature.calling_convention(),
        scoop_lir::TypedCallView::Direct { signature, .. } => signature.calling_convention(),
        scoop_lir::TypedCallView::IndirectResult { signature, .. } => {
            signature.calling_convention()
        }
    }
}

fn indirect_result_convention(
    call: &scoop_lir::TypedCallView<'_>,
) -> Option<scoop_lir::IndirectResultConvention> {
    match call {
        scoop_lir::TypedCallView::IndirectResult { signature, .. } => Some(signature.convention()),
        scoop_lir::TypedCallView::Void { .. }
        | scoop_lir::TypedCallView::ElidedZst { .. }
        | scoop_lir::TypedCallView::Direct { .. } => None,
    }
}

const fn call_argument_convention(argument: scoop_lir::AbiCallArgument) -> &'static str {
    match argument {
        scoop_lir::AbiCallArgument::ElidedZst(_) => "elided-ZST",
        scoop_lir::AbiCallArgument::Direct(_) => "direct",
        scoop_lir::AbiCallArgument::Indirect(_) => "indirect",
    }
}

const fn argument_convention(argument: &scoop_lir::AbiArgument) -> &'static str {
    match argument {
        scoop_lir::AbiArgument::ElidedZst(_) => "elided-ZST",
        scoop_lir::AbiArgument::Direct(_) => "direct",
        scoop_lir::AbiArgument::Indirect(_) => "indirect",
    }
}

fn call_error(function: &Function, detail: impl std::fmt::Display) -> CodegenError {
    CodegenError(format!("typed call @{}: {detail}", function.symbol()))
}

fn managed_poll_error(function: &Function, detail: impl std::fmt::Display) -> CodegenError {
    CodegenError(format!("managed poll @{}: {detail}", function.symbol()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use la_arena::Arena;

    fn module_with_types(structs: StructDefs, enums: EnumDefs) -> Module {
        let mut local_functions = scoop_lir::LocalFunctionIdentities::default();
        Module {
            cone: scoop_identity::ConeIdentity::SINGLE_FILE,
            globals: Arena::new(),
            initialization_units: Arena::new(),
            structs,
            enums,
            functions: Vec::new(),
            extern_functions: scoop_lir::ExternFunctions::default(),
            native_globals: Arena::new(),
            native_global_bridges: scoop_lir::NativeGlobalBridges::default(),
            callback_bridges: Arena::new(),
            foreign_callback_families: Arena::new(),
            foreign_callback_bridges: Arena::new(),
            output: scoop_lir::LirOutput::Executable {
                entry: scoop_lir::LocalFunctionRef::Managed(local_functions.alloc_managed()),
            },
            meta: scoop_lir::LirMeta {
                exact_types: Vec::new(),
                target_profile: scoop_lir::LirTargetProfile::DARWIN_AARCH64,
                canonical_c_abi: scoop_lir::CanonicalCAbiMetadata::default(),
                native_externals: scoop_lir::NativeExternalMetadata::default(),
                well_known_type_descriptors: scoop_lir::WellKnownTypeDescriptors {
                    string: scoop_lir::TypeDescriptorRef::Local(
                        scoop_lir::TypeDescriptorId::from_raw(0.into()),
                    ),
                },
                arrays: Arena::new(),
                layouts: Arena::new(),
                type_descriptors: Arena::new(),
                core_external_type_descriptors: Arena::new(),
                core_external_callables: Arena::new(),
                dependency_external_callables: Arena::new(),
            },
        }
    }

    fn abi_value(ty: LirType, size: u64, align: u64, scan: RefScan) -> scoop_lir::AbiValue {
        scoop_lir::AbiValue::new(
            ty,
            scoop_lir::AbiNonZeroLayout::new(size, align).expect("valid test ABI layout"),
            scan,
        )
        .expect("valid test ABI storage type")
    }

    #[test]
    fn abi_value_layout_must_match_the_embedded_target_profile() {
        let module = module_with_types(StructDefs::default(), EnumDefs::default());
        let value = abi_value(LirType::I64, 16, 8, RefScan::None);
        let error = AbiMetadataValidator::new(&module)
            .validate_value(&value, "test value")
            .expect_err("wrong target size must be rejected");
        assert!(
            error.0.contains("size/alignment 16/8") && error.0.contains("expected 8/8"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn struct_field_offsets_are_revalidated_before_accepting_an_abi_scan() {
        let mut structs = StructDefs::default();
        let id = structs.alloc_scoop(
            "BadOffset".to_string(),
            16,
            8,
            false,
            vec![scoop_lir::StructField {
                ty: scoop_lir::MANAGED_PTR,
                layout: scoop_lir::FieldLayout {
                    offset: 8,
                    access_align: 8,
                },
            }],
        );
        let module = module_with_types(structs, EnumDefs::default());
        let value = abi_value(LirType::Struct(id), 16, 8, RefScan::References(vec![8]));
        let error = AbiMetadataValidator::new(&module)
            .validate_value(&value, "test value")
            .expect_err("wrong struct field offset must be rejected");
        assert!(
            error.0.contains("field 0 layout 8/8") && error.0.contains("target layout 0/8"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn enum_scan_must_match_its_variant_field_offsets() {
        let mut enums = EnumDefs::default();
        let id = enums.alloc(scoop_lir::EnumDef {
            name: "BadScan".to_string(),
            repr: EnumRepr::Tagged {
                variants: vec![scoop_lir::EnumVariantRepr {
                    fields: vec![scoop_lir::EnumFieldRepr {
                        ty: scoop_lir::MANAGED_PTR,
                        offset: 8,
                    }],
                    slot_offset: 8,
                    slot_size: 8,
                    slot_align: 8,
                    gc_free: false,
                }],
                size: 16,
                align: 8,
            },
            scan: RefScan::References(vec![0]),
        });
        let module = module_with_types(StructDefs::default(), enums);
        let value = abi_value(LirType::Enum(id), 16, 8, RefScan::References(vec![0]));
        let error = AbiMetadataValidator::new(&module)
            .validate_value(&value, "test value")
            .expect_err("enum scan must come from its exact field offsets");
        assert!(
            error.0.contains("scan refs[0]") && error.0.contains("field-derived scan refs[8]"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn aggregate_argument_cannot_claim_direct_scoop_passing() {
        let module = module_with_types(StructDefs::default(), EnumDefs::default());
        let argument = scoop_lir::AbiArgument::Direct(abi_value(
            LirType::Aggregate(vec![LirType::I64, LirType::I64]),
            16,
            8,
            RefScan::None,
        ));
        let error = AbiMetadataValidator::new(&module)
            .validate_arguments(&[argument], "test signature")
            .expect_err("aggregate direct passing must be rejected");
        assert!(
            error.0.contains("uses direct passing")
                && error.0.contains("requires indirect passing"),
            "unexpected error: {error}"
        );
    }

    #[test]
    fn scalar_result_cannot_claim_indirect_scoop_passing() {
        let module = module_with_types(StructDefs::default(), EnumDefs::default());
        let signature = scoop_lir::ScoopAbiSignature::new(
            Vec::new(),
            scoop_lir::AbiReturn::Indirect(abi_value(LirType::I64, 8, 8, RefScan::None)),
            scoop_lir::CallingConvention::Cdecl,
        );
        let error = AbiMetadataValidator::new(&module)
            .validate_scoop_signature(&signature, "test signature")
            .expect_err("scalar indirect passing must be rejected");
        assert!(
            error
                .0
                .contains("uses indirect passing for i64, but the module target profile requires direct passing"),
            "unexpected error: {error}"
        );
    }
}
