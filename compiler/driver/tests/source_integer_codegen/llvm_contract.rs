use std::collections::HashMap;

use super::IntegerCase;

fn function_text<'a>(llvm: &'a str, symbol: &str) -> &'a str {
    let quoted = format!("@\"{symbol}\"(");
    let plain = format!("@{symbol}(");
    let start = llvm
        .match_indices("define ")
        .map(|(start, _)| start)
        .find(|start| {
            let declaration = llvm[*start..].lines().next().unwrap_or_default();
            declaration.contains(&quoted) || declaration.contains(&plain)
        })
        .unwrap_or_else(|| panic!("LLVM function `{symbol}` definition is missing"));
    let end = llvm[start..]
        .find("\n}\n")
        .map(|offset| start + offset + 2)
        .expect("function definition has a closing brace");
    &llvm[start..end]
}

fn blocks(function: &str) -> HashMap<String, String> {
    let mut blocks = HashMap::<String, String>::new();
    let mut current = None;
    for line in function.lines().skip(1) {
        if line == "}" {
            break;
        }
        if line.is_empty() {
            continue;
        }
        if !line.starts_with(char::is_whitespace) {
            let name = line
                .split_once(':')
                .map(|(name, _)| name)
                .unwrap_or_else(|| panic!("malformed LLVM block label `{line}`"));
            blocks.entry(name.to_string()).or_default();
            current = Some(name.to_string());
        } else if let Some(name) = &current {
            let block = blocks.get_mut(name).expect("current LLVM block exists");
            block.push_str(line);
            block.push('\n');
        }
    }
    blocks
}

pub(super) fn assert_contract(
    lir: &scoop_lir::Module,
    llvm: &str,
    callable_body: scoop_identity::PersistentCallableBodyId,
    case: IntegerCase,
) {
    let function = lir
        .functions
        .iter()
        .find(|function| function.callable_body.id() == callable_body)
        .unwrap_or_else(|| panic!("{} reaches LIR", case.function));
    let function_ir = function_text(llvm, function.symbol());
    let block_ir = blocks(function_ir);
    let (expected_div, expected_rem) = match case.lir_kind.signedness() {
        scoop_lir::IntegerSignedness::Signed => ("sdiv", "srem"),
        scoop_lir::IntegerSignedness::Unsigned => ("udiv", "urem"),
    };

    let safe_sites = function
        .blocks
        .iter()
        .flat_map(|(_, block)| {
            block.instructions.iter().filter_map(|instruction| {
                let scoop_lir::Instruction::SafeIntegerDivRem {
                    kind, operation, ..
                } = instruction
                else {
                    return None;
                };
                Some((&block.name, *kind, *operation))
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(safe_sites.len(), 2, "{} LIR safe sites", case.function);
    for (block, kind, operation) in safe_sites {
        assert_eq!(kind, case.lir_kind);
        let opcode = match operation {
            scoop_lir::IntegerDivRemOperation::Divide => expected_div,
            scoop_lir::IntegerDivRemOperation::Remainder => expected_rem,
        };
        let contents = &block_ir[block];
        let instruction = format!(" {opcode} {} ", case.llvm_type);
        assert_eq!(
            contents.matches(&instruction).count(),
            1,
            "{} {opcode} must be emitted in MIR's safe block `{block}`:\n{contents}",
            case.function
        );
    }
    for opcode in ["sdiv", "srem", "udiv", "urem"] {
        let expected = usize::from(opcode == expected_div || opcode == expected_rem);
        assert_eq!(
            function_ir
                .lines()
                .filter(|line| line.contains(&format!(" {opcode} {} ", case.llvm_type)))
                .count(),
            expected,
            "{} has no primitive div/rem outside typed safe LIR blocks",
            case.function
        );
    }

    let wrapping_blocks = function
        .blocks
        .iter()
        .filter_map(|(_, block)| {
            block
                .instructions
                .iter()
                .any(|instruction| {
                    matches!(
                        instruction,
                        scoop_lir::Instruction::IntegerBinary {
                            operation: scoop_lir::IntegerBinaryOperation::Add
                                | scoop_lir::IntegerBinaryOperation::Subtract
                                | scoop_lir::IntegerBinaryOperation::Multiply,
                            ..
                        } | scoop_lir::Instruction::IntegerUnary {
                            operation: scoop_lir::IntegerUnaryOperation::Negate,
                            ..
                        }
                    )
                })
                .then_some(&block.name)
        })
        .collect::<Vec<_>>();
    assert_eq!(wrapping_blocks.len(), 1);
    let wrapping_ir = &block_ir[wrapping_blocks[0]];
    for (opcode, count) in [("add", 1), ("mul", 1), ("sub", 2)] {
        assert_eq!(
            wrapping_ir
                .lines()
                .filter(|line| line.contains(&format!(" {opcode} {} ", case.llvm_type)))
                .count(),
            count,
            "{} preserves every wrapping {opcode}",
            case.function
        );
    }
    assert!(
        wrapping_ir
            .lines()
            .filter(|line| {
                [" add ", " sub ", " mul "]
                    .iter()
                    .any(|opcode| line.contains(opcode))
            })
            .all(|line| !line.contains(" nsw ") && !line.contains(" nuw ")),
        "{} source wrapping operations gained LLVM overflow flags:\n{wrapping_ir}",
        case.function
    );

    assert_masked_shift(wrapping_ir, case);
}

fn assert_masked_shift(block: &str, case: IntegerCase) {
    let mask = u64::from(case.lir_kind.width().bits() - 1);
    let mask_line = block
        .lines()
        .find(|line| {
            line.contains(" = and i64 ") && line.trim_end().ends_with(&format!(", {mask}"))
        })
        .unwrap_or_else(|| panic!("{} LLVM shift mask is missing", case.function));
    let masked_count = assignment_result(mask_line);
    let normalized_count = if case.llvm_type == "i64" {
        masked_count
    } else {
        let conversion = block
            .lines()
            .find(|line| {
                line.contains(&format!(
                    " = trunc i64 {masked_count} to {}",
                    case.llvm_type
                ))
            })
            .unwrap_or_else(|| panic!("{} LLVM shift mask conversion is missing", case.function));
        assignment_result(conversion)
    };
    let shift = block
        .lines()
        .find(|line| line.contains(&format!(" = shl {} ", case.llvm_type)))
        .unwrap_or_else(|| panic!("{} LLVM shift is missing", case.function));
    assert!(
        shift.trim_end().ends_with(&format!(", {normalized_count}")),
        "{} LLVM shift must consume the masked count: {shift}",
        case.function
    );
}

fn assignment_result(instruction: &str) -> &str {
    instruction
        .trim()
        .split_once(" = ")
        .expect("LLVM value instruction is an assignment")
        .0
}
