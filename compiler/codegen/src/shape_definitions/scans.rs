//! Pack a complete shape scan tree into its one address-significant atom.

use inkwell::module::Linkage;
use inkwell::types::AnyType;
use inkwell::values::{AnyValue, UnnamedAddress};

use super::*;
use crate::{SCAN_ARRAY, SCAN_SEQUENCE};

pub(super) fn emit_or_reuse_scan_definition<'ctx>(
    context: &'ctx Context,
    llvm: &LlvmModule<'ctx>,
    request: scoop_lir::PersistentSymbolRequest,
    scan: &RefScan,
) -> Result<(GlobalValue<'ctx>, bool), CodegenError> {
    let symbol = request.symbol();
    let symbol = symbol.as_str();
    if llvm.get_function(symbol).is_some() {
        return Err(CodegenError(format!(
            "scan definition `{symbol}` collides with an LLVM function"
        )));
    }
    if let Some(global) = llvm.get_global(symbol) {
        validate_existing_scan(context, global, request, scan)?;
        return Ok((global, false));
    }
    let mut words = Vec::new();
    append_scan(scan, &mut words);
    let count = u32::try_from(words.len())
        .map_err(|_| CodegenError("scan object exceeds LLVM array length".into()))?;
    let i64 = context.i64_type();
    let global = llvm.add_global(i64.array_type(count), None, symbol);
    let base = global.as_pointer_value();
    let values = words
        .into_iter()
        .map(|word| match word {
            ScanWord::Integer(value) => i64.const_int(value, false),
            ScanWord::Child(index) => {
                // Every child index is within this complete, nonempty i64 array.
                unsafe { base.const_gep(i64, &[i64.const_int(index as u64, false)]) }
                    .const_to_int(i64)
            }
        })
        .collect::<Vec<_>>();
    global.set_constant(true);
    global.set_alignment(8);
    global.set_initializer(&i64.const_array(&values));
    crate::emission::apply_persistent_linkage(&global, request, true)?;
    Ok((global, true))
}

#[derive(Clone, Copy)]
enum ScanWord {
    Integer(u64),
    Child(usize),
}

fn append_scan(scan: &RefScan, words: &mut Vec<ScanWord>) -> usize {
    let start = words.len();
    match scan {
        RefScan::None => words.push(ScanWord::Integer(0)),
        RefScan::References(offsets) => {
            words.push(ScanWord::Integer(offsets.len() as u64));
            words.extend(offsets.iter().copied().map(ScanWord::Integer));
        }
        RefScan::Sequence(parts) => {
            words.push(ScanWord::Integer(SCAN_SEQUENCE));
            words.push(ScanWord::Integer(parts.len() as u64));
            let children = words.len();
            words.resize(children + parts.len(), ScanWord::Integer(0));
            for (index, part) in parts.iter().enumerate() {
                let child = append_scan(part, words);
                words[children + index] = ScanWord::Child(child);
            }
        }
        RefScan::Array {
            length_offset,
            first_element_offset,
            stride,
            element,
        } => {
            words.extend([
                ScanWord::Integer(SCAN_ARRAY),
                ScanWord::Integer(*length_offset),
                ScanWord::Integer(*first_element_offset),
                ScanWord::Integer(stride.get()),
            ]);
            let pointer = words.len();
            words.push(ScanWord::Integer(0));
            let child = append_scan(element.as_ref_scan(), words);
            words[pointer] = ScanWord::Child(child);
        }
    }
    start
}

fn validate_existing_scan(
    context: &Context,
    global: GlobalValue<'_>,
    request: scoop_lir::PersistentSymbolRequest,
    scan: &RefScan,
) -> Result<(), CodegenError> {
    let symbol = request.symbol();
    let expected_words = match scan {
        RefScan::None => vec![context.i64_type().const_zero()],
        RefScan::References(offsets) => {
            std::iter::once(context.i64_type().const_int(offsets.len() as u64, false))
                .chain(
                    offsets
                        .iter()
                        .map(|offset| context.i64_type().const_int(*offset, false)),
                )
                .collect()
        }
        RefScan::Sequence(_) | RefScan::Array { .. } => {
            return Err(CodegenError(format!(
                "recursive scan definition `{symbol}` was defined before canonical shape emission"
            )));
        }
    };
    let expected = context.i64_type().const_array(&expected_words);
    let initializer = global.get_initializer();
    if global.get_value_type() != expected.get_type().as_any_type_enum()
        || global.get_linkage()
            != match request.linkage() {
                scoop_lir::LinkageClass::OdrWeak => Linkage::WeakODR,
                _ => Linkage::External,
            }
        || global.get_unnamed_address() != UnnamedAddress::None
        || !global.is_constant()
        || global.get_alignment() != 8
        || initializer.map(|value| value.print_to_string()).as_ref()
            != Some(&expected.print_to_string())
    {
        return Err(CodegenError(format!(
            "predefined scan `{symbol}` does not match its canonical definition"
        )));
    }
    Ok(())
}
