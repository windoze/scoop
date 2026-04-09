/// Parse a decimal integer literal (possibly containing `_` separators) into a `u128`.
///
/// This function is used by both HIR lowering (to store parsed values in `LiteralKind`)
/// and codegen (for `const_eval_int_expr_bits` and similar paths).
pub fn parse_int_literal_decimal(text: &str) -> u128 {
    let mut out: u128 = 0;
    for ch in text.chars() {
        if ch == '_' {
            continue;
        }
        if let Some(d) = ch.to_digit(10) {
            out = out.saturating_mul(10).saturating_add(u128::from(d));
        }
    }
    out
}
