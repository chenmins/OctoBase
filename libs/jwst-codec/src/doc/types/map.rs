use std::{collections::hash_map::Iter, rc::Rc};

use super::*;
use crate::{
    doc::{AsInner, Node, Parent, YTypeRef},
    impl_type, JwstCodecResult,
};

impl_type!(Map);

pub(crate) trait MapType: AsInner<Inner = YTypeRef> {
    fn _insert<V: Into<Value>>(&mut self, key: String, value: V) -> JwstCodecResult {
        if let Some((mut store, mut ty)) = self.as_inner().write() {
            // Normally `ty.map[key]` points at the right-most (head) item of
            // the parent_sub chain, which is exactly the item we want to use
            // as `left` so that the new item is appended at the end of the
            // chain (and therefore has `right == None` after integration,
            // staying alive — see the integrate rule that auto-deletes
            // map-typed items whose `right.is_some()`).
            //
            // However, after loading a binary snapshot that contained
            // concurrent writes to the same map key, the cached pointer may
            // refer to an item that has since been marked deleted (or that
            // never was the actual right-most alive item on remote peers).
            // If we used that pointer blindly the new item would carry an
            // `origin_left_id` pointing at a deleted item; when the update is
            // sent over y-websocket, remote peers may then place the new
            // item in the middle of their parent_sub chain, where the
            // auto-delete rule kicks in and the value silently disappears.
            //
            // To be robust we resolve the actual right-most *alive* item in
            // the parent_sub chain and use it as `left` instead. Falling back
            // to `None` when no alive item exists matches the semantics of an
            // insert into an empty key.
            let raw_left = ty.map.get(&SmolStr::new(&key)).cloned();
            let left = resolve_visible_head(raw_left);

            let item = store.create_item(
                value.into().into(),
                left.unwrap_or(Somr::none()),
                Somr::none(),
                Some(Parent::Type(self.as_inner().clone())),
                Some(SmolStr::new(key)),
            );
            store.integrate(Node::Item(item), 0, Some(&mut ty))?;
        }

        Ok(())
    }

    fn _get(&self, key: &str) -> Option<Value> {
        self.as_inner().ty().and_then(|ty| {
            ty.map.get(key).and_then(|item| {
                if let Some(item) = item.get() {
                    if item.deleted() {
                        return None;
                    }

                    Some(Value::from(&item.content))
                } else {
                    None
                }
            })
        })
    }

    fn _contains_key(&self, key: &str) -> bool {
        if let Some(ty) = self.as_inner().ty() {
            ty.map
                .get(key)
                .and_then(|item| item.get())
                .map_or(false, |item| !item.deleted())
        } else {
            false
        }
    }

    fn _remove(&mut self, key: &str) {
        if let Some((mut store, mut ty)) = self.as_inner().write() {
            if let Some(item) = ty.map.get(key).cloned() {
                if let Some(item) = item.get() {
                    store.delete_item(item, Some(&mut ty));
                }
            }
        }
    }

    fn _len(&self) -> u64 {
        self._keys().count() as u64
    }

    fn _iter(&self) -> EntriesInnerIterator {
        let ty = self.as_inner().ty();

        if let Some(ty) = ty {
            let ty = Rc::new(ty);

            EntriesInnerIterator {
                iter: Some(unsafe { &*Rc::as_ptr(&ty) }.map.iter()),
                _lock: Some(ty),
            }
        } else {
            EntriesInnerIterator {
                _lock: None,
                iter: None,
            }
        }
    }

    fn _keys(&self) -> KeysIterator {
        KeysIterator(self._iter())
    }

    fn _values(&self) -> ValuesIterator {
        ValuesIterator(self._iter())
    }

    fn _entries(&self) -> EntriesIterator {
        EntriesIterator(self._iter())
    }
}

pub(crate) struct EntriesInnerIterator<'a> {
    _lock: Option<Rc<RwLockReadGuard<'a, YType>>>,
    iter: Option<Iter<'a, SmolStr, ItemRef>>,
}

pub struct KeysIterator<'a>(EntriesInnerIterator<'a>);
pub struct ValuesIterator<'a>(EntriesInnerIterator<'a>);
pub struct EntriesIterator<'a>(EntriesInnerIterator<'a>);

impl<'a> Iterator for EntriesInnerIterator<'a> {
    type Item = (&'a str, &'a Item);

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(iter) = &mut self.iter {
            for (k, v) in iter {
                if let Some(item) = v.get() {
                    if !item.deleted() {
                        return Some((k.as_str(), item));
                    }
                }
            }

            None
        } else {
            None
        }
    }
}

impl<'a> Iterator for KeysIterator<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(k, _)| k)
    }
}

impl Iterator for ValuesIterator<'_> {
    type Item = Value;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(_, v)| Value::from(&v.content))
    }
}

impl<'a> Iterator for EntriesIterator<'a> {
    type Item = (&'a str, Value);

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|(k, v)| (k, Value::from(&v.content)))
    }
}

impl MapType for Map {}

/// Resolve the right-most *alive* item in a parent_sub chain, starting from
/// the cached head pointer (`ty.map[key]`).
///
/// `ty.map[key]` is supposed to be the right-most item in the chain — and on
/// a freshly-built doc that is always true — but after merging in a remote
/// snapshot that contained concurrent writes to the same key, the cached
/// pointer may refer to an item that has since been marked deleted (and that
/// may even sit in the middle of the chain rather than at the tail).
///
/// In that situation we walk the chain to its actual right-most node and then
/// walk back until we find an alive item, returning it. If the cached pointer
/// is already alive we use it directly. If no alive item exists in the chain
/// we return `None`, which mirrors the semantics of inserting into an empty
/// key.
fn resolve_visible_head(head: Option<Somr<Item>>) -> Option<Somr<Item>> {
    let head = head?;

    {
        let live = head.get()?;
        if !live.deleted() {
            return Some(head);
        }
    }

    // The cached head pointer is deleted. Walk to the true right-most node,
    // then walk left looking for the first alive item.
    let mut rightmost: Somr<Item> = head.clone();
    loop {
        let next = match rightmost.get() {
            Some(item) => item.right.clone(),
            None => break,
        };
        if next.get().is_some() {
            rightmost = next;
        } else {
            break;
        }
    }

    let mut cursor: Somr<Item> = rightmost;
    loop {
        match cursor.get() {
            Some(item) => {
                if !item.deleted() {
                    return Some(cursor);
                }
                let left = item.left.clone();
                if left.get().is_none() {
                    return None;
                }
                cursor = left;
            }
            None => return None,
        }
    }
}

impl Map {
    #[inline(always)]
    pub fn insert<V: Into<Value>>(&mut self, key: String, value: V) -> JwstCodecResult {
        self._insert(key, value)
    }

    #[inline(always)]
    pub fn get(&self, key: &str) -> Option<Value> {
        self._get(key)
    }

    #[inline(always)]
    pub fn contains_key(&self, key: &str) -> bool {
        self._contains_key(key)
    }

    #[inline(always)]
    pub fn remove(&mut self, key: &str) {
        self._remove(key)
    }

    #[inline(always)]
    pub fn len(&self) -> u64 {
        self._len()
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[inline(always)]
    pub fn iter(&self) -> EntriesIterator {
        self._entries()
    }

    #[inline(always)]
    pub fn entries(&self) -> EntriesIterator {
        self._entries()
    }

    #[inline(always)]
    pub fn keys(&self) -> KeysIterator {
        self._keys()
    }

    #[inline(always)]
    pub fn values(&self) -> ValuesIterator {
        self._values()
    }
}

impl serde::Serialize for Map {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;

        let mut map = serializer.serialize_map(Some(self.len() as usize))?;
        for (key, value) in self.iter() {
            map.serialize_entry(&key, &value)?;
        }
        map.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{loom_model, Any, Doc};

    #[test]
    fn test_map_basic() {
        loom_model!({
            let doc = Doc::new();
            let mut map = doc.get_or_create_map("map").unwrap();
            map.insert("1".to_string(), "value").unwrap();
            assert_eq!(map.get("1").unwrap(), Value::Any(Any::String("value".to_string())));
            assert!(!map.contains_key("nonexistent_key"));
            assert_eq!(map.len(), 1);
            assert!(map.contains_key("1"));
            map.remove("1");
            assert!(!map.contains_key("1"));
            assert_eq!(map.len(), 0);
        });
    }

    #[test]
    fn test_map_equal() {
        loom_model!({
            let doc = Doc::new();
            let mut map = doc.get_or_create_map("map").unwrap();
            map.insert("1".to_string(), "value").unwrap();
            map.insert("2".to_string(), false).unwrap();

            let binary = doc.encode_update_v1().unwrap();
            let new_doc = Doc::try_from_binary_v1(binary).unwrap();
            let map = new_doc.get_or_create_map("map").unwrap();
            assert_eq!(map.get("1").unwrap(), Value::Any(Any::String("value".to_string())));
            assert_eq!(map.get("2").unwrap(), Value::Any(Any::False));
            assert_eq!(map.len(), 2);
        });
    }

    #[test]
    fn test_map_renew_value() {
        loom_model!({
            let doc = Doc::new();
            let mut map = doc.get_or_create_map("map").unwrap();
            map.insert("1".to_string(), "value").unwrap();
            map.insert("1".to_string(), "value2").unwrap();
            assert_eq!(map.get("1").unwrap(), Value::Any(Any::String("value2".to_string())));
            assert_eq!(map.len(), 1);
        });
    }

    #[test]
    fn test_map_re_encode() {
        loom_model!({
            let binary = {
                let doc = Doc::new();
                let mut map = doc.get_or_create_map("map").unwrap();
                map.insert("1".to_string(), "value1").unwrap();
                map.insert("2".to_string(), "value2").unwrap();
                doc.encode_update_v1().unwrap()
            };

            {
                let doc = Doc::try_from_binary_v1(binary).unwrap();
                let map = doc.get_or_create_map("map").unwrap();
                assert_eq!(map.get("1").unwrap(), Value::Any(Any::String("value1".to_string())));
                assert_eq!(map.get("2").unwrap(), Value::Any(Any::String("value2".to_string())));
            }
        });
    }

    #[test]
    fn test_map_iter() {
        loom_model!({
            let doc = Doc::new();
            let mut map = doc.get_or_create_map("map").unwrap();
            map.insert("1".to_string(), "value1").unwrap();
            map.insert("2".to_string(), "value2").unwrap();
            let mut vec = map.entries().collect::<Vec<_>>();

            // hashmap iteration is in random order instead of insert order
            vec.sort_by(|a, b| a.0.cmp(b.0));

            assert_eq!(
                vec,
                vec![
                    ("1", Value::Any(Any::String("value1".to_string()))),
                    ("2", Value::Any(Any::String("value2".to_string())))
                ]
            )
        });
    }

    /// Regression test for the `Map::_insert` behaviour after the cached
    /// `parent.map[key]` pointer has become a deleted item.
    ///
    /// Simulates the bug observed when loading a binary snapshot containing
    /// concurrent writes to the same map key: the server's internal head
    /// pointer ends up referring to an item that has since been marked
    /// deleted. Without the fix, the subsequent `insert` would create a new
    /// item whose `origin_left_id` points at the deleted item, which other
    /// peers may then auto-delete during integration (the map-typed
    /// `right.is_some()` rule). With the fix, `_insert` walks back to an
    /// alive item (or falls back to `None`) before creating the struct.
    #[test]
    fn test_map_insert_after_cached_head_deleted() {
        loom_model!({
            let doc = Doc::new();
            let mut map = doc.get_or_create_map("map").unwrap();

            // Establish a live value at the key.
            map.insert("k".to_string(), "v1").unwrap();
            assert_eq!(map.get("k").unwrap(), Value::Any(Any::String("v1".to_string())));

            // Set it twice more so the chain becomes non-trivial.
            map.insert("k".to_string(), "v2").unwrap();
            map.insert("k".to_string(), "v3").unwrap();
            assert_eq!(map.get("k").unwrap(), Value::Any(Any::String("v3".to_string())));

            // Remove the key: this marks the cached head item as deleted but
            // leaves `parent.map["k"]` pointing at it.
            map.remove("k");
            assert!(!map.contains_key("k"));

            // Insert again. Prior to the fix this would create a struct whose
            // `left` is the deleted v3 item; after the fix we resolve to None
            // (no alive item in the chain) and create a clean head.
            map.insert("k".to_string(), "v4").unwrap();
            assert_eq!(map.get("k").unwrap(), Value::Any(Any::String("v4".to_string())));

            // Round-trip through the binary encoding to make sure the wire
            // form is consistent (i.e., the new struct is not lost when
            // another peer re-applies the update).
            let binary = doc.encode_update_v1().unwrap();
            let new_doc = Doc::try_from_binary_v1(binary).unwrap();
            let map = new_doc.get_or_create_map("map").unwrap();
            assert_eq!(map.get("k").unwrap(), Value::Any(Any::String("v4".to_string())));
        });
    }
}
