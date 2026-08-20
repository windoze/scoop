use super::*;

/// Follow the optional argument builder before the selected main invocation.
pub(super) fn main_invoke<'a>(
    function: &'a Function,
    first: &'a InvokeSite,
    gateway: Gateway,
    takes_arguments: bool,
) -> Result<(&'a InvokeSite, Vec<AbiCallArgument>)> {
    if !takes_arguments {
        return Ok((first, Vec::new()));
    }
    let fail = |reason| error(function, reason);
    if !matches!(gateway, Gateway::Root { .. })
        || !matches!(first, InvokeSite::Managed(_))
        || !first.args().is_empty()
    {
        return Err(fail(
            "argv construction must invoke the ordinary managed core helper",
        ));
    }
    let out = first
        .direct_out()
        .filter(|out| {
            function
                .temps
                .iter()
                .any(|(id, temp)| id == *out && temp.ty == MANAGED_PTR)
        })
        .ok_or_else(|| fail("argv construction must produce an array reference"))?;
    let block = function
        .blocks
        .iter()
        .find_map(|(id, block)| (id == first.normal()).then_some(block))
        .ok_or_else(|| fail("argv construction has no main block"))?;
    let [Instruction::Invoke { site: main }] = block.instructions.as_slice() else {
        return Err(fail("argv construction must continue directly to main"));
    };
    if main.unwind() != first.unwind()
        || !matches!(block.terminator, Terminator::Br(target) if target == main.normal())
    {
        return Err(fail(
            "argv construction and main must share their failure exit",
        ));
    }
    Ok((main, vec![AbiCallArgument::Direct(Value::Temp(out))]))
}
