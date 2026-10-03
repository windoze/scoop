use super::*;

pub(super) fn matches(expected: &lir::RefScan, actual: &lir::RefScan) -> Result<bool> {
    use lir::RefScan as Scan;
    Ok(match (expected, actual) {
        (Scan::None, Scan::None) => true,
        (Scan::References(expected), Scan::References(actual)) => {
            if expected.len() != actual.len() {
                return Ok(false);
            }

            expected == actual
        }
        (Scan::Sequence(expected), Scan::Sequence(actual)) => {
            if expected.len() != actual.len() {
                return Ok(false);
            }
            for (expected, actual) in expected.iter().zip(actual) {
                if !matches(expected, actual)? {
                    return Ok(false);
                }
            }
            true
        }
        (
            Scan::Array {
                length_offset,
                first_element_offset,
                stride,
                element,
            },
            Scan::Array {
                length_offset: actual_length,
                first_element_offset: actual_first,
                stride: actual_stride,
                element: actual_element,
            },
        ) => {
            length_offset == actual_length
                && first_element_offset == actual_first
                && stride == actual_stride
                && matches(element.as_ref_scan(), actual_element.as_ref_scan())?
        }
        _ => false,
    })
}

pub(super) fn shape_matches(
    expected: &lir::TypeInstanceShapeV1,
    actual: &lir::TypeInstanceShapeV1,
) -> Result<bool> {
    Ok(expected.instance_kind() == actual.instance_kind()
        && expected.inline_storage_kind() == actual.inline_storage_kind()
        && expected.minimum_size() == actual.minimum_size()
        && expected.instance_alignment() == actual.instance_alignment()
        && expected.inline_offset() == actual.inline_offset()
        && expected.inline_size() == actual.inline_size()
        && expected.inline_stride() == actual.inline_stride()
        && expected.inline_alignment() == actual.inline_alignment()
        && matches(expected.object_scan(), actual.object_scan())?
        && matches(expected.inline_scan(), actual.inline_scan())?)
}
