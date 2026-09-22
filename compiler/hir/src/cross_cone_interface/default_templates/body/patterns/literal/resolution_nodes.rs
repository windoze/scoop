use super::*;
use crate::cross_cone_interface::default_templates::resolution_resources::resource_node;

resource_node!(DecodedDefaultLiteralEqualityV1, node, children, {
    match node {
        Self::Integer { kind } => {
            children.push(kind)?;
        }
        Self::Ordinary { target } => {
            children.push(target)?;
        }
    }
    Ok(())
});
