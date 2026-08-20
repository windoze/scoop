//! Partition the scalar domain without enumerating absent code points.

use super::*;

impl Lowerer {
    pub(super) fn missing_character_witness(
        &mut self,
        types: &[hir::TypeId],
        matrix: &Matrix,
    ) -> Option<Vec<Witness>> {
        let mut singletons: BTreeMap<char, Matrix> = BTreeMap::new();
        let mut others = Matrix::new();
        for row in matrix {
            let tail = row[1..].to_vec();
            if super::super::is_irrefutable(&row[0]) {
                others.push(tail);
            } else if let hir::Pattern::Literal { value, .. } = &row[0]
                && let hir::ExprKind::CharLiteral(value) = value.kind
            {
                singletons.entry(value).or_default().push(tail);
            } else {
                unreachable!("a Char pattern column contains scalar literals and wildcards");
            }
        }
        let other_witness = self.missing_witness(&types[1..], &others)?;
        let mut next = 0;
        for value in singletons.keys() {
            if *value as u32 != next {
                break;
            }
            next += 1;
            if next == 0xd800 {
                next = 0xe000;
            }
        }
        let first_other = char::from_u32(next);
        for (value, rows) in singletons {
            if first_other.is_some_and(|first| value > first) {
                break;
            }
            let mut specialized = others.clone();
            specialized.extend(rows);
            if let Some(tail) = self.missing_witness(&types[1..], &specialized) {
                return Some(character_witness(value, tail));
            }
        }
        first_other.map(|value| character_witness(value, other_witness))
    }
}

fn character_witness(value: char, tail: Vec<Witness>) -> Vec<Witness> {
    std::iter::once(Witness::Char(value)).chain(tail).collect()
}
