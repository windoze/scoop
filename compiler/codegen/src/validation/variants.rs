//! Defensive validation for typed variant tests and payload projections.

use std::collections::{HashMap, HashSet};

use super::super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct VariantFact {
    operand: scoop_lir::Value,
    variant: scoop_lir::LirVariantRef,
}

mod dominance;

pub(super) fn validate_variant_primitives(module: &Module) -> Result<(), CodegenError> {
    for function in module.callable_bodies() {
        let tests = validate_function_shapes(module, function)?;
        dominance::validate(function, &tests)?;
    }
    Ok(())
}

fn validate_function_shapes(
    module: &Module,
    function: &Function,
) -> Result<HashMap<scoop_lir::TempId, VariantFact>, CodegenError> {
    // Variant dominance relies on Temp meaning one stable SSA-ish value.  No
    // older whole-LIR verifier enforced that invariant, so establish it here
    // for every instruction before trusting Temp identity in a fact.
    let mut definitions = HashSet::new();
    for (_, block) in function.blocks.iter() {
        for instruction in &block.instructions {
            for output in crate::module_context::instruction_temp_defs(instruction)
                .into_iter()
                .flatten()
            {
                if !definitions.insert(output) {
                    return Err(CodegenError(format!(
                        "function @{} defines temporary t{} more than once",
                        function.symbol(),
                        output.into_raw()
                    )));
                }
            }
        }
    }

    let mut tests = HashMap::new();
    for (_, block) in function.blocks.iter() {
        for instruction in &block.instructions {
            match instruction {
                Instruction::EnumWrap {
                    out,
                    variant,
                    fields,
                } => validate_enum_wrap(module, function, *out, *variant, fields)?,
                Instruction::VariantTest {
                    out,
                    operand,
                    variant,
                } => {
                    validate_variant_ref(module, function, *variant, "variant_test")?;
                    validate_stable_operand(function, *operand, "variant_test")?;
                    let operand_ty = super::checked_value_type(
                        module,
                        function,
                        *operand,
                        "variant_test operand",
                    )?;
                    let expected = LirType::Enum(variant.definition());
                    if operand_ty != expected {
                        return Err(CodegenError(format!(
                            "variant_test @{} expects {}, got {}",
                            function.symbol(),
                            expected.dump(),
                            operand_ty.dump()
                        )));
                    }
                    let result_ty =
                        super::checked_temp_type(function, *out, "variant_test result")?;
                    if result_ty != &LirType::I1 {
                        return Err(CodegenError(format!(
                            "variant_test @{} result is {}, expected i1",
                            function.symbol(),
                            result_ty.dump()
                        )));
                    }
                    tests.insert(
                        *out,
                        VariantFact {
                            operand: *operand,
                            variant: *variant,
                        },
                    );
                }
                Instruction::VariantPayloadProject {
                    out,
                    operand,
                    field,
                } => validate_projection_shape(module, function, *out, *operand, *field)?,
                _ => {}
            }
        }
    }
    Ok(tests)
}

fn validate_enum_wrap(
    module: &Module,
    function: &Function,
    out: scoop_lir::TempId,
    variant: scoop_lir::LirVariantRef,
    fields: &[scoop_lir::Value],
) -> Result<(), CodegenError> {
    validate_variant_ref(module, function, variant, "enum_wrap")?;
    let expected_result = LirType::Enum(variant.definition());
    let actual_result = super::checked_temp_type(function, out, "enum_wrap result")?;
    if actual_result != &expected_result {
        return Err(CodegenError(format!(
            "enum_wrap @{} result is {}, expected {}",
            function.symbol(),
            actual_result.dump(),
            expected_result.dump()
        )));
    }
    let expected_fields = match &module.enums[variant.definition()].repr {
        EnumRepr::Niche {
            kind,
            payload_variant,
        } if variant.index() == *payload_variant => vec![LirType::Ptr(kind.pointer_kind())],
        EnumRepr::Niche { .. } => Vec::new(),
        EnumRepr::Tagged { variants, .. } => variants[variant.index() as usize]
            .fields
            .iter()
            .map(|field| field.ty.clone())
            .collect(),
    };
    if fields.len() != expected_fields.len() {
        return Err(CodegenError(format!(
            "enum_wrap @{} variant {} has {} fields, expected {}",
            function.symbol(),
            variant.index(),
            fields.len(),
            expected_fields.len()
        )));
    }
    for (index, (value, expected)) in fields.iter().zip(expected_fields).enumerate() {
        let actual = super::checked_value_type(
            module,
            function,
            *value,
            &format!("enum_wrap field {index}"),
        )?;
        if actual != expected {
            return Err(CodegenError(format!(
                "enum_wrap @{} field {} has type {}, expected {}",
                function.symbol(),
                index,
                actual.dump(),
                expected.dump()
            )));
        }
    }
    Ok(())
}

fn validate_projection_shape(
    module: &Module,
    function: &Function,
    out: scoop_lir::TempId,
    operand: scoop_lir::Value,
    field: scoop_lir::LirVariantFieldRef,
) -> Result<(), CodegenError> {
    let variant = field.variant();
    validate_variant_ref(module, function, variant, "variant_payload_project")?;
    validate_stable_operand(function, operand, "variant_payload_project")?;
    if !module.enums.contains_variant_field(field) {
        return Err(CodegenError(format!(
            "variant_payload_project @{} carries invalid field {} for enum{} variant {}",
            function.symbol(),
            field.index(),
            field.definition().into_raw(),
            variant.index()
        )));
    }
    let operand_ty =
        super::checked_value_type(module, function, operand, "variant_payload_project operand")?;
    let expected_operand = LirType::Enum(field.definition());
    if operand_ty != expected_operand {
        return Err(CodegenError(format!(
            "variant_payload_project @{} expects {}, got {}",
            function.symbol(),
            expected_operand.dump(),
            operand_ty.dump()
        )));
    }
    let expected = module
        .enums
        .variant_field_type(field)
        .expect("a field contained by this store has an exact type");
    let actual = super::checked_temp_type(function, out, "variant_payload_project result")?;
    if actual != &expected {
        let detail = match (actual, &expected) {
            (LirType::Ptr(actual), LirType::Ptr(expected)) => format!(
                "pointer provenance is {}, expected {}",
                actual.dump(),
                expected.dump()
            ),
            _ => format!("result is {}, expected {}", actual.dump(), expected.dump()),
        };
        return Err(CodegenError(format!(
            "variant_payload_project @{} {detail}",
            function.symbol()
        )));
    }
    Ok(())
}

fn validate_stable_operand(
    function: &Function,
    operand: scoop_lir::Value,
    instruction: &str,
) -> Result<(), CodegenError> {
    if !matches!(
        operand,
        scoop_lir::Value::Param(_) | scoop_lir::Value::Local(_) | scoop_lir::Value::Temp(_)
    ) {
        return Err(CodegenError(format!(
            "{instruction} @{} requires a stable Param/Local/Temp enum value identity",
            function.symbol()
        )));
    }
    Ok(())
}

fn validate_variant_ref(
    module: &Module,
    function: &Function,
    variant: scoop_lir::LirVariantRef,
    instruction: &str,
) -> Result<(), CodegenError> {
    super::validate_variant_ref(
        module,
        variant,
        &format!("{instruction} @{}", function.symbol()),
    )
}
