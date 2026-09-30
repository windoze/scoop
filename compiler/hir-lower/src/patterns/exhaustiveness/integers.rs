use scoop_hir as hir;

use crate::Type;

/// Exact domain of the integer types represented by the current typed HIR.
/// Keeping this match closed prevents exhaustiveness proofs from assuming a
/// narrower source domain than values the remaining pipeline can produce.
pub(super) fn integer_domain(ty: &Type) -> Option<IntegerDomain> {
    match ty {
        Type::Integer(kind) => Some(match kind.signedness() {
            hir::IntegerSignedness::Signed => IntegerDomain::signed(kind.width().bits() as u8),
            hir::IntegerSignedness::Unsigned => IntegerDomain::unsigned(kind.width().bits() as u8),
        }),
        Type::Unit
        | Type::Boolean
        | Type::String
        | Type::Struct(_)
        | Type::ImportedClass(_)
        | Type::ImportedInterface(_)
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
    ordinals: impl IntoIterator<Item = u128>,
) -> Option<u128> {
    let mut expected = 0u128;
    for ordinal in ordinals {
        if ordinal != expected {
            return Some(expected);
        }
        expected += 1;
    }
    (expected < domain.cardinality()).then_some(expected)
}

fn integer_literal_raw(value: &hir::Expr, domain: IntegerDomain) -> Option<u64> {
    match &value.kind {
        hir::ExprKind::IntegerLiteral(value) => Some(value.raw_bits() & domain.mask()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn integer_domain_uses_mathematical_order_and_u128_cardinality() {
        let source_int = integer_domain(&Type::Integer(hir::IntegerKind::SIGNED_32))
            .expect("Int has a closed domain");
        assert_eq!(source_int, IntegerDomain::signed(32));
        assert_eq!(source_int.cardinality(), 4_294_967_296);
        assert_eq!(source_int.render(0x8000_0000), "-2147483648");

        let source_uint = integer_domain(&Type::Integer(hir::IntegerKind::UNSIGNED_32))
            .expect("UInt has a closed domain");
        assert_eq!(source_uint, IntegerDomain::unsigned(32));
        assert_eq!(source_uint.cardinality(), 4_294_967_296);
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
        assert_eq!(
            first_missing_integer_ordinal(domain, ordinals.iter().copied()),
            None
        );

        ordinals.remove(&127);
        assert_eq!(
            first_missing_integer_ordinal(domain, ordinals.iter().copied()),
            Some(127)
        );
        assert_eq!(domain.render(domain.raw_from_ordinal(127)), "-1");
    }

    #[test]
    fn complete_uint16_coverage_scans_only_observed_ordinals() {
        let domain = IntegerDomain::unsigned(16);
        assert_eq!(
            first_missing_integer_ordinal(domain, 0..domain.cardinality()),
            None
        );
    }
}
