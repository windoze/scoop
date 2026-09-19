use super::*;
use crate::cross_cone_interface::default_templates::resolution_resources::resource_node;

resource_node!(DecodedDefaultCallableReferenceV1, node, children, {
    let DecodedDefaultCallableReferenceV1 {
        invoke,
        definition_path,
        target,
        function_type,
        captures,
        owner_type_parameter_count,
    } = node;
    children.push(owner_type_parameter_count)?;
    children.push(captures)?;
    children.push(function_type)?;
    children.push(target)?;
    children.push(definition_path)?;
    children.push(invoke)?;
    Ok(())
});

resource_node!(DecodedDefaultCallableReferenceTargetV1, node, children, {
    match node {
        Self::Named(field_0) => {
            children.push(field_0)?;
        }
        Self::Local {
            declaration,
            callee,
        } => {
            children.push(callee)?;
            children.push(declaration)?;
        }
        Self::BoundMember { receiver, callee } => {
            children.push(callee)?;
            children.push(receiver)?;
        }
        Self::BoundExtension { receiver, callee } => {
            children.push(callee)?;
            children.push(receiver)?;
        }
    }
    Ok(())
});
