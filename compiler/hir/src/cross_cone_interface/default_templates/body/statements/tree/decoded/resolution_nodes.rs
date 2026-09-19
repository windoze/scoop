use super::*;
use crate::cross_cone_interface::default_templates::resolution_resources::resource_node;

resource_node!(DecodedDefaultStatementV1, node, children, {
    let DecodedDefaultStatementV1 {
        kind,
        definition_origin,
    } = node;
    children.push(definition_origin)?;
    children.push(kind)?;
    Ok(())
});

resource_node!(DecodedDefaultStatementKindV1, node, children, {
    match node {
        Self::Expr(field_0) => {
            children.push(field_0)?;
        }
        Self::InitializationEnsure(field_0) => {
            children.push(field_0)?;
        }
        Self::LocalFunction(field_0) => {
            children.push(field_0)?;
        }
        Self::Return(field_0) => {
            children.push(field_0)?;
        }
        Self::ValDecl { pattern, init } => {
            children.push(init)?;
            children.push(pattern)?;
        }
        Self::Assign { target, value } => {
            children.push(value)?;
            children.push(target)?;
        }
        Self::If {
            condition,
            then_body,
            else_body,
        } => {
            children.push(else_body)?;
            children.push(then_body)?;
            children.push(condition)?;
        }
        Self::While {
            condition_setup,
            condition,
            body,
        } => {
            children.push(body)?;
            children.push(condition)?;
            children.push(condition_setup)?;
        }
        Self::For(field_0) => {
            children.push(field_0)?;
        }
        Self::Break => return Ok(()),
        Self::Continue => return Ok(()),
        Self::When(field_0) => {
            children.push(field_0)?;
        }
        Self::Try(field_0) => {
            children.push(field_0)?;
        }
        Self::Throw(field_0) => {
            children.push(field_0)?;
        }
    }
    Ok(())
});
