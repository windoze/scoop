use super::*;

pub(super) fn dump_boxing(function: &Function, instruction: &Instruction, buf: &mut String) {
    match instruction {
        Instruction::BoxValue {
            out,
            payload,
            safepoint,
            live,
        } => {
            let payload = match payload {
                BoxPayload::ZeroSized(descriptor) => {
                    format!("box_zst td{}", descriptor.descriptor().into_raw())
                }
                BoxPayload::NonZero(place) => format!(
                    "box_value td{}, local{} scan={}",
                    place.descriptor().descriptor().into_raw(),
                    place.local().into_raw(),
                    place.descriptor().value().scan().dump()
                ),
            };
            buf.push_str(&format!(
                "    t{} = {} sp{} live=[{}]\n",
                out.into_raw(),
                payload,
                safepoint_name(function, *safepoint),
                live_set_name(live)
            ));
        }
        Instruction::UnboxValue { object, result } => {
            let result = match result {
                UnboxResult::ZeroSized { descriptor, out } => format!(
                    "unbox_zst td{}, {} -> t{}",
                    descriptor.descriptor().into_raw(),
                    value_name(*object),
                    out.into_raw()
                ),
                UnboxResult::NonZero(place) => format!(
                    "unbox_value td{}, {} -> local{}",
                    place.descriptor().descriptor().into_raw(),
                    value_name(*object),
                    place.local().into_raw()
                ),
            };
            buf.push_str(&format!("    {result}\n"));
        }
        _ => unreachable!("boxing dump dispatch is exhaustive"),
    }
}
