//! Edit one member declaration without resolving its dependencies.

use scoop_manifest::DependencyCoordinateKey;
use toml_edit::{DocumentMut, Item, Table, Value};

use crate::{PmError, PmResult, Workspace, WorkspaceMember, lockfile::Purpose};

pub fn dependency(
    workspace: &Workspace,
    member: &WorkspaceMember,
    purpose: Purpose,
    key: &DependencyCoordinateKey,
    replacement: Option<Value>,
) -> PmResult<bool> {
    let mut document = member
        .manifest_text
        .parse::<DocumentMut>()
        .map_err(|error| PmError::new(format!("{}: {error}", member.manifest.display())))?;
    let section = match purpose {
        Purpose::Production => "dependencies",
        Purpose::Test => "test-dependencies",
    };
    let name = key.to_string();
    if let Some(mut value) = replacement {
        if !document.contains_key(section) {
            document[section] = Item::Table(Table::new());
        }
        let table = document[section]
            .as_table_like_mut()
            .ok_or_else(|| PmError::new(format!("{section} must be a table")))?;
        if let Some(previous) = table.get_mut(&name) {
            if let Some(previous) = previous.as_value() {
                *value.decor_mut() = previous.decor().clone();
            }
            *previous = if let Some(previous) = previous.as_table() {
                let mut updated = Table::new();
                *updated.decor_mut() = previous.decor().clone();
                if let Some(position) = previous.position() {
                    updated.set_position(position);
                }
                if let Some(fields) = value.as_inline_table() {
                    for (key, value) in fields.iter() {
                        updated.insert(key, Item::Value(value.clone()));
                    }
                } else {
                    updated.insert("version", Item::Value(value));
                }
                Item::Table(updated)
            } else {
                Item::Value(value)
            };
        } else {
            table.insert(&name, Item::Value(value));
        }
    } else {
        let removed = document
            .get_mut(section)
            .and_then(Item::as_table_like_mut)
            .and_then(|table| table.remove(&name));
        if removed.is_none() {
            return Err(PmError::new(format!(
                "{key} is not declared in [{section}] of {}",
                member.manifest.display()
            )));
        }
    }
    let text = document.to_string();
    if text == member.manifest_text {
        return Ok(false);
    }
    workspace.validate_edited_member(member, &text)?;
    crate::cache::atomic_write(&member.manifest, text.as_bytes())?;
    Ok(true)
}
