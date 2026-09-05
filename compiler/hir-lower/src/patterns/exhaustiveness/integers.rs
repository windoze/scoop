use std::collections::BTreeSet;

use scoop_hir as hir;

use crate::Type;

/// Exact domain of the integer types represented by the current typed HIR.
/// Keeping this match closed prevents exhaustiveness proofs from assuming a
/// narrower source domain than values the remaining pipeline can produce.
pub(super) fn integer_domain(ty: &Type) -> Option<IntegerDomain> {
    match ty {
        Type::Int => Some(IntegerDomain::signed(64)),
        Type::UInt => Some(IntegerDomain::unsigned(64)),
        Type::Unit
        | Type::Boolean
        | Type::String
        | Type::Struct(_)
        | Type::Class(_)
        | Type::Interface(_)
        | Type::Any
        | Type::Tuple(_)
        | Type::Function(_)
        | Type::Ptr(_)
        | Type::FunPtr(_)
        | Type::Enum(_)
        | Type::Param(_) => None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct IntegerDomain {
    signed: bool,
    width: u8,
}

impl IntegerDomain {
    pub(super) const fn signed(width: u8) -> Self {
        Self {
            signed: true,
            width,
        }
    }

    pub(super) const fn unsigned(width: u8) -> Self {
        Self {
            signed: false,
            width,
        }
    }

    pub(super) fn cardinality(self) -> u128 {
        1u128 << self.width
    }

    fn mask(self) -> u64 {
        if self.width == 64 {
            u64::MAX
        } else {
            (1u64 << self.width) - 1
        }
    }

    pub(super) fn ordinal(self, raw: u64) -> u128 {
        let raw = raw & self.mask();
        if self.signed {
            u128::from(raw ^ (1u64 << (self.width - 1)))
        } else {
            u128::from(raw)
        }
    }

    pub(super) fn raw_from_ordinal(self, ordinal: u128) -> u64 {
        let ordinal = ordinal as u64;
        if self.signed {
            ordinal ^ (1u64 << (self.width - 1))
        } else {
            ordinal
        }
    }

    fn normalize(self, value: i128) -> u64 {
        value.rem_euclid(self.cardinality() as i128) as u64
    }

    pub(super) fn render(self, raw: u64) -> String {
        let raw = raw & self.mask();
        if self.signed {
            let sign = 1u64 << (self.width - 1);
            let value = if raw & sign == 0 {
                i128::from(raw)
            } else {
                i128::from(raw) - self.cardinality() as i128
            };
            value.to_string()
        } else {
            format!("{raw}u")
        }
    }
}

pub(super) fn integer_pattern_raw(pattern: &hir::Pattern, domain: IntegerDomain) -> Option<u64> {
    let hir::Pattern::Literal { value, .. } = pattern else {
        return None;
    };
    integer_literal_raw(value, domain)
}

pub(super) fn first_missing_integer_ordinal(
    domain: IntegerDomain,
    ordinals: &BTreeSet<u128>,
) -> Option<u128> {
    if ordinals.len() as u128 == domain.cardinality() {
        return None;
    }
    let mut expected = 0u128;
    for ordinal in ordinals {
        if *ordinal != expected {
            return Some(expected);
        }
        expected += 1;
    }
    Some(expected)
}

fn integer_literal_raw(value: &hir::Expr, domain: IntegerDomain) -> Option<u64> {
    match &value.kind {
        hir::ExprKind::IntLiteral(value) => Some(domain.normalize(i128::from(*value))),
        hir::ExprKind::PrimitiveUnary {
            kind: hir::PrimitiveUnaryKind::IntUnaryPlus | hir::PrimitiveUnaryKind::UIntUnaryPlus,
            operand,
        } => integer_literal_raw(operand, domain),
        hir::ExprKind::PrimitiveUnary {
            kind: hir::PrimitiveUnaryKind::IntUnaryMinus,
            operand,
        } => {
            let operand = integer_literal_raw(operand, domain)?;
            Some(0u64.wrapping_sub(operand) & domain.mask())
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integer_domain_uses_mathematical_order_and_u128_cardinality() {
        let source_int = integer_domain(&Type::Int).expect("Int has a closed domain");
        assert_eq!(source_int, IntegerDomain::signed(64));
        assert_eq!(source_int.cardinality(), 18_446_744_073_709_551_616);
        assert_eq!(
            source_int.render(0x8000_0000_0000_0000),
            "-9223372036854775808"
        );

        let source_uint = integer_domain(&Type::UInt).expect("UInt has a closed domain");
        assert_eq!(source_uint, IntegerDomain::unsigned(64));
        assert_eq!(source_uint.cardinality(), 18_446_744_073_709_551_616);
        assert_eq!(source_uint.render(0), "0u");

        let signed = IntegerDomain::signed(8);
        assert_eq!(signed.cardinality(), 256);
        assert_eq!(signed.ordinal(0x80), 0);
        assert_eq!(signed.ordinal(0xff), 127);
        assert_eq!(signed.ordinal(0), 128);
        assert_eq!(signed.render(0x80), "-128");
        assert_eq!(signed.render(0x7f), "127");

        let unsigned = IntegerDomain::unsigned(8);
        assert_eq!(unsigned.ordinal(0), 0);
        assert_eq!(unsigned.ordinal(0xff), 255);
        assert_eq!(unsigned.render(0xff), "255u");
    }

    #[test]
    fn finite_integer_domain_is_exhaustive_only_after_every_bit_pattern() {
        let domain = IntegerDomain::signed(8);
        let mut ordinals: BTreeSet<u128> = (0..256).collect();
        assert_eq!(first_missing_integer_ordinal(domain, &ordinals), None);

        ordinals.remove(&127);
        assert_eq!(first_missing_integer_ordinal(domain, &ordinals), Some(127));
        assert_eq!(domain.render(domain.raw_from_ordinal(127)), "-1");
    }
}
