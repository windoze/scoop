//! Representation operations attached to actual core member declarations.

use super::*;

mod wire;

pub type HirFloatConversion = scoop_identity::FloatConversion<IntegerKind>;
pub type DefaultFloatConversionV1 = scoop_identity::FloatConversion<DefaultIntegerKindV1>;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum FloatIntrinsicKind {
    Unary {
        kind: FloatKind,
        operation: FloatUnaryOperator,
    },
    Binary {
        kind: FloatKind,
        operation: FloatBinaryOperator,
    },
    Conversion(HirFloatConversion),
}

impl FloatIntrinsicKind {
    pub fn name(self) -> String {
        match self {
            Self::Unary { kind, operation } => {
                format!("{}_{}", kind.registry_key(), operation.registry_key())
            }
            Self::Binary { kind, operation } => {
                format!("{}_{}", kind.registry_key(), operation.registry_key())
            }
            Self::Conversion(scoop_identity::FloatConversion::FromInteger { source, target }) => {
                format!("{}_to_{}", source.registry_key(), target.registry_key())
            }
            Self::Conversion(scoop_identity::FloatConversion::ToInteger { source, target }) => {
                format!("{}_to_{}", source.registry_key(), target.registry_key())
            }
            Self::Conversion(scoop_identity::FloatConversion::BetweenFloats { source, target }) => {
                format!("{}_to_{}", source.registry_key(), target.registry_key())
            }
        }
    }

    pub fn owner(self) -> IntrinsicTypeKind {
        match self {
            Self::Unary { kind, .. } | Self::Binary { kind, .. } => IntrinsicTypeKind::Float(kind),
            Self::Conversion(scoop_identity::FloatConversion::FromInteger { source, .. }) => {
                IntrinsicTypeKind::Integer(source)
            }
            Self::Conversion(
                scoop_identity::FloatConversion::ToInteger { source, .. }
                | scoop_identity::FloatConversion::BetweenFloats { source, .. },
            ) => IntrinsicTypeKind::Float(source),
        }
    }
}

pub fn float_intrinsic_kinds() -> Vec<FloatIntrinsicKind> {
    let mut result = Vec::new();
    for kind in FloatKind::ALL {
        result.extend(
            FloatUnaryOperator::ALL
                .iter()
                .map(|&operation| FloatIntrinsicKind::Unary { kind, operation }),
        );
        result.extend(
            FloatBinaryOperator::ALL
                .iter()
                .copied()
                .filter(|operation| operation.has_source_member())
                .map(|operation| FloatIntrinsicKind::Binary { kind, operation }),
        );
        for integer in IntegerKind::ALL {
            result.push(FloatIntrinsicKind::Conversion(
                scoop_identity::FloatConversion::FromInteger {
                    source: integer,
                    target: kind,
                },
            ));
            result.push(FloatIntrinsicKind::Conversion(
                scoop_identity::FloatConversion::ToInteger {
                    source: kind,
                    target: integer,
                },
            ));
        }
        for target in FloatKind::ALL {
            result.push(FloatIntrinsicKind::Conversion(
                scoop_identity::FloatConversion::BetweenFloats {
                    source: kind,
                    target,
                },
            ));
        }
    }
    result
}
