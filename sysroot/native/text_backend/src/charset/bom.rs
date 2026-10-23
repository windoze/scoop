use super::{Options, kind::Kind};

#[derive(Default)]
pub struct Header {
    pub bytes: [u8; 4],
    pub size: usize,
    pub read: usize,
}

impl Header {
    pub fn resolve(
        &mut self,
        kind: Kind,
        options: &Options,
        end: bool,
    ) -> Result<Option<(Kind, i64)>, i32> {
        let (big, little, order) = kind.bom_pair();
        if big.is_empty() {
            return Ok(Some((kind, 0)));
        }
        let input = &self.bytes[..self.size];
        let matches_big = big.starts_with(input);
        let matches_little = little.starts_with(input);
        if self.size < big.len() && !end && (matches_big || matches_little) {
            return Ok(None);
        }
        let present = self.size == big.len() && (matches_big || matches_little);
        let found = if matches_big { 1 } else { 2 };
        if present && order != 0 && order != found {
            return Err(7);
        }
        if !present && (options.input_bom == 2 || (kind.ambiguous() && options.source_order == 0)) {
            return Err(6);
        }
        if present && options.input_bom != 1 {
            self.read = self.size;
        }
        Ok(Some((
            kind.ordered(if present {
                found
            } else {
                options.source_order as u8
            }),
            self.read as i64,
        )))
    }
}
