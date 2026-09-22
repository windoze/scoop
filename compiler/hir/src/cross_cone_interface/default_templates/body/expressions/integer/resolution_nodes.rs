use super::*;
use crate::cross_cone_interface::default_templates::resolution_resources::resource_node;

resource_node!(DecodedDefaultIntegerOperationV1, node, children, {
    match node {
        Self::NoGc { kind, operation } => {
            children.push(operation)?;
            children.push(kind)?;
        }
        Self::Managed { kind, operation } => {
            children.push(operation)?;
            children.push(kind)?;
        }
    }
    Ok(())
});
