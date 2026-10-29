//! Target aggregate classification from already-laid-out scalar leaves.

use super::ProcedureAbi;
use lir::{AbiArrayElement, AbiCarrier, AbiCoercion, AbiPart, AbiValuePosition, FloatKind};
use scoop_lir as lir;

pub(super) fn coercion(
    layout: &lir::AbiAggregateLayout,
    abi: ProcedureAbi,
    position: AbiValuePosition,
) -> Option<AbiCoercion> {
    Aggregate(layout).coercion(abi, position)
}

struct Aggregate<'a>(&'a lir::AbiAggregateLayout);

impl std::ops::Deref for Aggregate<'_> {
    type Target = lir::AbiAggregateLayout;
    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl Aggregate<'_> {
    /// None is the target's MEMORY/indirect convention, not a missing plan.
    fn coercion(&self, abi: ProcedureAbi, position: AbiValuePosition) -> Option<AbiCoercion> {
        match abi {
            ProcedureAbi::DarwinAarch64 => self.aarch64(position),
            ProcedureAbi::SysvAmd64 => self.sysv(),
            ProcedureAbi::Win64 => self.win64(),
        }
    }

    fn part(&self, carrier: AbiCarrier, offset: u64, extent: u64) -> AbiPart {
        let alignment = if offset == 0 {
            self.alignment
        } else {
            self.alignment.min(1 << offset.trailing_zeros())
        };
        AbiPart::new(carrier, offset, extent, alignment)
            .expect("laid-out scalar leaves produce bounded ABI carriers")
    }

    fn aarch64(&self, position: AbiValuePosition) -> Option<AbiCoercion> {
        if let Some(carrier) = self.hfa() {
            return Some(AbiCoercion::One(self.part(carrier, 0, self.size)));
        }
        if self.size == 0 || self.size > 16 {
            return None;
        }
        let carrier = if self.alignment == 16 {
            AbiCarrier::Integer(128)
        } else if self.size > 8 {
            AbiCarrier::Array {
                element: AbiArrayElement::I64,
                count: 2,
            }
        } else {
            AbiCarrier::Integer(match position {
                AbiValuePosition::Argument => 64,
                AbiValuePosition::Result => (self.size * 8) as u8,
            })
        };
        Some(AbiCoercion::One(self.part(carrier, 0, self.size)))
    }

    fn win64(&self) -> Option<AbiCoercion> {
        matches!(self.size, 1 | 2 | 4 | 8).then(|| {
            AbiCoercion::One(self.part(AbiCarrier::Integer((self.size * 8) as u8), 0, self.size))
        })
    }

    fn hfa(&self) -> Option<AbiCarrier> {
        if !(1..=4).contains(&self.leaves.len()) {
            return None;
        }
        let AbiCarrier::Float(kind) = self.leaves[0].carrier else {
            return None;
        };
        let width = AbiCarrier::Float(kind).byte_size();
        if self.size != width * self.leaves.len() as u64
            || !self.leaves.iter().enumerate().all(|(i, leaf)| {
                leaf.carrier == AbiCarrier::Float(kind) && leaf.offset == i as u64 * width
            })
        {
            return None;
        }
        Some(if self.leaves.len() == 1 {
            AbiCarrier::Float(kind)
        } else {
            AbiCarrier::Array {
                element: match kind {
                    FloatKind::F32 => AbiArrayElement::F32,
                    FloatKind::F64 => AbiArrayElement::F64,
                },
                count: self.leaves.len() as u8,
            }
        })
    }

    fn sysv(&self) -> Option<AbiCoercion> {
        if self.size == 0
            || self.size > 16
            || self
                .leaves
                .iter()
                .any(|leaf| leaf.offset % leaf.alignment != 0)
        {
            return None;
        }
        let mut classes = [Eightbyte::Empty; 2];
        for leaf in &self.leaves {
            let class = if matches!(leaf.carrier, AbiCarrier::Float(_)) {
                Eightbyte::Sse
            } else {
                Eightbyte::Integer
            };
            let end = leaf.offset + leaf.carrier.byte_size();
            for index in leaf.offset / 8..end.div_ceil(8) {
                let old = &mut classes[index as usize];
                *old = match (*old, class) {
                    (Eightbyte::Integer, _) | (_, Eightbyte::Integer) => Eightbyte::Integer,
                    _ => class,
                };
            }
        }
        let parts = classes
            .iter()
            .enumerate()
            .filter_map(|(i, class)| {
                let offset = i as u64 * 8;
                let carrier = match class {
                    Eightbyte::Empty => return None,
                    Eightbyte::Integer => {
                        let pointer = self.leaves.iter().find(|leaf| {
                            leaf.offset == offset && matches!(leaf.carrier, AbiCarrier::Pointer(_))
                        });
                        pointer.map_or(
                            AbiCarrier::Integer(
                                (self.size.saturating_sub(offset).min(8) * 8) as u8,
                            ),
                            |leaf| leaf.carrier,
                        )
                    }
                    Eightbyte::Sse => {
                        let end = self
                            .leaves
                            .iter()
                            .filter(|leaf| leaf.offset / 8 == i as u64)
                            .map(|leaf| leaf.offset + leaf.carrier.byte_size())
                            .max()?;
                        if self.leaves.iter().any(|leaf| {
                            leaf.offset == offset
                                && leaf.carrier == AbiCarrier::Float(FloatKind::F64)
                        }) {
                            AbiCarrier::Float(FloatKind::F64)
                        } else if end > offset + 4 {
                            AbiCarrier::FloatPair
                        } else {
                            AbiCarrier::Float(FloatKind::F32)
                        }
                    }
                };
                Some(self.part(carrier, offset, carrier.byte_size().min(self.size - offset)))
            })
            .collect::<Vec<_>>();
        match parts.as_slice() {
            [one] => Some(AbiCoercion::One(*one)),
            [one, two] => Some(AbiCoercion::Two([*one, *two])),
            _ => None,
        }
    }
}

#[derive(Clone, Copy)]
enum Eightbyte {
    Empty,
    Integer,
    Sse,
}

#[cfg(test)]
mod tests;
