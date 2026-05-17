use std::io::Write;

use jwst_codec::{CrdtWriter, JwstCodecError, JwstCodecResult, RawEncoder};


use jwst_core::{Any, Value, Workspace};
pub use jwst_logger::{debug, error, info, trace, warn};
pub use nanoid::nanoid;
pub use serde::{Deserialize, Serialize};

pub fn encode_update_with_guid<S: AsRef<str>>(update: &[u8], guid: S) -> JwstCodecResult<Vec<u8>> {
    let mut encoder = RawEncoder::default();
    encoder.write_var_string(guid)?;
    let mut buffer = encoder.into_inner();

    buffer
        .write_all(update)
        .map_err(|e| JwstCodecError::InvalidWriteBuffer(e.to_string()))?;

    Ok(buffer)
}

fn value_variant(value: &Value) -> &'static str {
    match value {
        Value::Any(_) => "Any",
        Value::Array(_) => "Y.Array",
        Value::Map(_) => "Y.Map",
        Value::Text(_) => "Y.Text",
        _ => "Other",
    }
}

fn value_to_plain_any(value: &Value) -> Any {
    match value {
        Value::Any(any) => any.clone(),
        Value::Array(array) => Any::Array(array.iter().map(|value| value_to_plain_any(&value)).collect()),
        Value::Map(map) => {
            let entries = map
                .entries()
                .map(|(key, value)| (key.to_string(), value_to_plain_any(&value)));
            entries.collect()
        }
        Value::Text(text) => Any::String(text.to_string()),
        _ => Any::Null,
    }
}

fn any_needs_plain_rebuild(any: &Any) -> bool {
    matches!(any, Any::Object(_) | Any::Array(_))
}

/// Rebuild workspaces that are made only of root Y.Maps containing plain Any
/// values. Imported plain-data maps can carry old struct metadata that REST can
/// read but standard yjs peers ignore during the initial sync. Building a fresh
/// update from the visible JSON shape removes those stale structs.
pub fn rebuild_plain_root_maps_for_yjs_update(workspace: &Workspace, reason: &str) -> Option<(Vec<u8>, usize)> {
    let workspace_id = workspace.id();
    let mut roots = Vec::new();
    let mut has_object_like_value = false;
    let mut entry_count = 0usize;

    for root in workspace.doc_keys() {
        let Ok(map) = workspace.get_or_create_map(&root) else {
            info!(
                "rebuild_plain_yjs: workspace={} root={} reason={} skipped non-map root",
                workspace_id, root, reason
            );
            return None;
        };

        let mut entries = Vec::new();
        for (key, value) in map.entries() {
            let Value::Any(any) = value else {
                info!(
                    "rebuild_plain_yjs: workspace={} root={} key={} reason={} skipped because value is {}",
                    workspace_id,
                    root,
                    key,
                    reason,
                    value_variant(&value)
                );
                return None;
            };

            has_object_like_value |= any_needs_plain_rebuild(&any);
            entries.push((key.to_string(), any));
        }

        entry_count += entries.len();
        roots.push((root, entries));
    }

    if !has_object_like_value {
        trace!(
            "rebuild_plain_yjs: workspace={} reason={} no object-like Any values",
            workspace_id,
            reason
        );
        return None;
    }

    let Ok(rebuilt) = Workspace::new(&workspace_id) else {
        error!(
            "rebuild_plain_yjs: workspace={} reason={} failed to create rebuilt workspace",
            workspace_id, reason
        );
        return None;
    };

    for (root, entries) in roots {
        let Ok(mut rebuilt_map) = rebuilt.get_or_create_map(&root) else {
            error!(
                "rebuild_plain_yjs: workspace={} root={} reason={} failed to create rebuilt root map",
                workspace_id, root, reason
            );
            return None;
        };

        for (key, value) in entries {
            if let Err(e) = rebuilt_map.insert(key.clone(), value) {
                error!(
                    "rebuild_plain_yjs: workspace={} root={} key={} reason={} failed: {:?}",
                    workspace_id, root, key, reason, e
                );
                return None;
            }
        }
    }

    match rebuilt.to_binary() {
        Ok(update) => {
            info!(
                "rebuild_plain_yjs: workspace={} reason={} rebuilt {} plain root-map entries into {} bytes",
                workspace_id,
                reason,
                entry_count,
                update.len()
            );
            Some((update, entry_count))
        }
        Err(e) => {
            error!(
                "rebuild_plain_yjs: workspace={} reason={} failed to encode rebuilt workspace: {:?}",
                workspace_id, reason, e
            );
            None
        }
    }
}

/// Convert direct nested CRDT entries under root Y.Maps into plain Any values.
///
/// Some imported snapshots contain a root map entry whose value is another
/// Y.Map/Y.Array/Y.Text. The REST serializer can walk those nested jwst-codec
/// types, but a standard yjs/y-websocket peer may not receive them from the
/// same imported binary. Rewriting only those direct root-map entries preserves
/// the JSON shape while making the value visible to yjs clients.
pub fn normalize_workspace_for_yjs(workspace: &Workspace, reason: &str) -> usize {
    let workspace_id = workspace.id();
    let mut normalized = 0usize;

    for root in workspace.doc_keys() {
        let Ok(mut map) = workspace.get_or_create_map(&root) else {
            trace!(
                "normalize_yjs_nested: workspace={} root={} reason={} skipped non-map root",
                workspace_id,
                root,
                reason
            );
            continue;
        };

        let replacements = map
            .entries()
            .filter_map(|(key, value)| match value {
                Value::Map(_) | Value::Array(_) | Value::Text(_) => {
                    Some((key.to_string(), value_variant(&value), value_to_plain_any(&value)))
                }
                _ => None,
            })
            .collect::<Vec<_>>();

        if replacements.is_empty() {
            trace!(
                "normalize_yjs_nested: workspace={} root={} reason={} no nested entries",
                workspace_id,
                root,
                reason
            );
            continue;
        }

        for (key, variant, any) in replacements {
            info!(
                "normalize_yjs_nested: workspace={} root={} key={} reason={} {} -> Any",
                workspace_id, root, key, reason, variant
            );
            match map.insert(key.clone(), any) {
                Ok(()) => normalized += 1,
                Err(e) => error!(
                    "normalize_yjs_nested: workspace={} root={} key={} reason={} failed: {:?}",
                    workspace_id, root, key, reason, e
                ),
            }
        }
    }

    if normalized > 0 {
        info!(
            "normalize_yjs_nested: workspace={} reason={} normalized {} direct nested entries",
            workspace_id, reason, normalized
        );
    }

    normalized
}
