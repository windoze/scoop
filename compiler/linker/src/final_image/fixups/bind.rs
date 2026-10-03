use super::*;

#[derive(Clone)]
pub(crate) struct Binding {
    pub symbol: String,
    pub ordinal: i32,
    pub addend: i64,
    pub weak: bool,
}

pub(in crate::final_image) fn bindings(
    bytes: &[u8],
    segments: &[Segment],
    weak: bool,
) -> Result<BTreeMap<u64, Binding>, LinkError> {
    let mut cursor = Cursor::new(bytes);
    let mut position = Position::default();
    let mut binding = Binding {
        symbol: String::new(),
        ordinal: 0,
        addend: 0,
        weak,
    };
    let mut result = BTreeMap::new();
    while !cursor.done() {
        let byte = cursor.byte()?;
        let immediate = u64::from(byte & 0xf);
        let (count, skip) = match byte & 0xf0 {
            0x00 => {
                binding.symbol.clear();
                binding.addend = 0;
                continue;
            }
            0x10 => {
                binding.ordinal = immediate as i32;
                continue;
            }
            0x20 => {
                binding.ordinal = i32::try_from(cursor.uleb()?).map_err(error)?;
                continue;
            }
            0x30 => {
                binding.ordinal = if immediate == 0 {
                    0
                } else {
                    (immediate as i8 | -16) as i32
                };
                continue;
            }
            0x40 => {
                binding.symbol = cursor.name()?;
                continue;
            }
            0x50 if immediate == 1 => continue,
            0x60 => {
                binding.addend = cursor.sleb()?;
                continue;
            }
            0x70 => {
                position.segment = immediate as usize;
                position.offset = cursor.uleb()?;
                continue;
            }
            0x80 => {
                position.advance(cursor.uleb()?)?;
                continue;
            }
            0x90 => (1, 0),
            0xa0 => (1, cursor.uleb()?),
            0xb0 => (1, immediate * 8),
            0xc0 => (cursor.uleb()?, cursor.uleb()?),
            _ => return Err(error(format!("invalid final bind opcode {byte:#x}"))),
        };
        if binding.symbol.is_empty() {
            return Err(error("pointer binding has no symbol"));
        }
        position.check_run(segments, count, skip)?;
        for _ in 0..count {
            let address = position.address(segments)?;
            if result.insert(address, binding.clone()).is_some() {
                return Err(error("duplicate binding within fixup stream"));
            }
            position.advance(
                8u64.checked_add(skip)
                    .ok_or_else(|| error("bind skip overflow"))?,
            )?;
        }
    }
    Ok(result)
}
