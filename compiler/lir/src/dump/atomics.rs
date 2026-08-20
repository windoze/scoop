use super::*;

pub(super) fn dump_atomic(instruction: &Instruction, buf: &mut String) {
    match instruction {
        Instruction::AtomicLoad {
            out,
            location,
            order,
        } => buf.push_str(&format!(
            "    t{} = atomic_load {:?} {:?} {} +{}\n",
            out.into_raw(),
            location.kind,
            order,
            value_name(location.object),
            location.offset
        )),
        Instruction::AtomicStore {
            location,
            value,
            order,
        } => buf.push_str(&format!(
            "    atomic_store {:?} {:?} {} +{}, {}\n",
            location.kind,
            order,
            value_name(location.object),
            location.offset,
            value_name(*value)
        )),
        Instruction::AtomicRmw {
            out,
            location,
            value,
            operation,
            order,
        } => buf.push_str(&format!(
            "    t{} = atomic_rmw {:?} {:?} {:?} {} +{}, {}\n",
            out.into_raw(),
            location.kind,
            operation,
            order,
            value_name(location.object),
            location.offset,
            value_name(*value)
        )),
        Instruction::AtomicCmpXchg {
            out,
            location,
            expected,
            replacement,
            result,
            order,
        } => {
            let (success, failure) = order.memory_orders();
            buf.push_str(&format!(
                "    t{} = atomic_cmpxchg {:?} {:?} success={:?} failure={:?} {} +{}, {}, {}\n",
                out.into_raw(),
                location.kind,
                result,
                success,
                failure,
                value_name(location.object),
                location.offset,
                value_name(*expected),
                value_name(*replacement)
            ));
        }
        _ => unreachable!("atomic dump dispatch is exhaustive"),
    }
}
