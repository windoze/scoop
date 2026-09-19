use super::*;
use crate::cross_cone_interface::default_templates::resolution_resources::resource_node;

resource_node!(DecodedDefaultWhenV1, node, children, {
    let DecodedDefaultWhenV1 {
        subject,
        arms,
        fallback,
    } = node;
    children.push(fallback)?;
    children.push(arms)?;
    children.push(subject)?;
    Ok(())
});

resource_node!(DecodedDefaultWhenArmV1, node, children, {
    let DecodedDefaultWhenArmV1 {
        pattern,
        guard,
        body,
        definition_origin,
    } = node;
    children.push(definition_origin)?;
    children.push(body)?;
    children.push(guard)?;
    children.push(pattern)?;
    Ok(())
});

resource_node!(DecodedOptionalDefaultWhenGuardV1, node, children, {
    match node {
        Self::Absent => return Ok(()),
        Self::Present(field_0) => {
            children.push(field_0)?;
        }
    }
    Ok(())
});

resource_node!(DecodedDefaultWhenGuardV1, node, children, {
    let DecodedDefaultWhenGuardV1 { setup, condition } = node;
    children.push(condition)?;
    children.push(setup)?;
    Ok(())
});

resource_node!(DecodedDefaultWhenFallbackV1, node, children, {
    match node {
        Self::Else(field_0) => {
            children.push(field_0)?;
        }
        Self::IrrefutableArm { subject_type } => {
            children.push(subject_type)?;
        }
        Self::PatternMatrix { subject_type } => {
            children.push(subject_type)?;
        }
        Self::EnumPatternMatrix {
            subject_type,
            owner_type,
        } => {
            children.push(owner_type)?;
            children.push(subject_type)?;
        }
    }
    Ok(())
});

resource_node!(DecodedDefaultTryV1, node, children, {
    let DecodedDefaultTryV1 {
        body,
        catches,
        finally_body,
    } = node;
    children.push(finally_body)?;
    children.push(catches)?;
    children.push(body)?;
    Ok(())
});

resource_node!(DecodedDefaultCatchV1, node, children, {
    let DecodedDefaultCatchV1 {
        local_index,
        value_type,
        body,
        definition_origin,
    } = node;
    children.push(definition_origin)?;
    children.push(body)?;
    children.push(value_type)?;
    children.local_index(*local_index)?;
    children.push(local_index)?;
    Ok(())
});

resource_node!(DecodedOptionalDefaultStatementListV1, node, children, {
    match node {
        Self::Absent => return Ok(()),
        Self::Present(field_0) => {
            children.push(field_0)?;
        }
    }
    Ok(())
});
