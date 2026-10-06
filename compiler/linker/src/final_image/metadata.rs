//! Verify the image contents newly produced by program-link.

use crate::{LinkError, error, program::image::SelectedImage};

pub(super) fn check<'a>(
    images: &[SelectedImage],
    symbol: impl Fn(&str) -> Result<u64, LinkError>,
    at: impl Fn(u64, usize) -> Result<&'a [u8], LinkError>,
    pointer: impl Fn(u64) -> Result<u64, LinkError>,
    read_only: impl Fn(u64, u64) -> Result<(), LinkError>,
) -> Result<(), LinkError> {
    for image in images {
        let address = symbol(&image.name)?;
        read_only(address, 240)?;
        let bytes = at(address, 240)?;
        if bytes[..8] != 0x5343_4f4f_5049_4d47u64.to_le_bytes()
            || bytes[8..12] != 4u32.to_le_bytes()
            || bytes[12..16] != 240u32.to_le_bytes()
            || bytes[64..96] != image.cone
            || bytes[96..128] != image.fingerprint
        {
            return Err(error(format!(
                "final image {} differs from its selected contents",
                image.name
            )));
        }
        for (index, part) in image.coordinate.iter().enumerate() {
            let field = 16 + index * 16;
            if count(bytes, field + 8) != part.len() as u64
                || at(pointer(address + field as u64)?, part.len())? != part
            {
                return Err(error("final image coordinate differs from its Cone"));
            }
        }
        if count(bytes, 136) != image.dependencies.len() as u64 {
            return Err(error("final image dependency count differs from its Cone"));
        }
        let dependencies = pointer(address + 128)?;
        for (index, expected) in image.dependencies.iter().enumerate() {
            if at(dependencies + index as u64 * 32, 32)? != expected {
                return Err(error("final image dependency differs from its Cone"));
            }
        }
        for (table, records) in image.tables.iter().enumerate() {
            let field = 144 + table * 16;
            if count(bytes, field + 8) != records.len() as u64 {
                return Err(error(format!(
                    "final image {} table {table} has an incorrect selected count",
                    image.name
                )));
            }
            let base = pointer(address + field as u64)?;
            read_only(base, records.len().max(1) as u64 * 8)?;
            for (index, record) in records.iter().enumerate() {
                if pointer(base + index as u64 * 8)? != symbol(record)? {
                    return Err(error(format!(
                        "final image {} table {table} does not reference selected record {record}",
                        image.name
                    )));
                }
            }
        }
        for (name, expected) in &image.traps {
            if at(symbol(name)?, expected.len())? != *expected {
                return Err(error(
                    "final image trap message differs from its planned bytes",
                ));
            }
        }
    }
    Ok(())
}

fn count(record: &[u8], field: usize) -> u64 {
    u64::from_le_bytes(
        record[field..field + 8]
            .try_into()
            .expect("fixed image extent"),
    )
}
