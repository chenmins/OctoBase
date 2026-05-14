use std::{collections::hash_map::Iter, rc::Rc};

use jwst_logger::debug;

use super::*;
use crate::{
    doc::{AsInner, Node, Parent, YTypeRef},
    impl_type, JwstCodecResult,
};

impl_type!(Map);

pub(crate) trait MapType: AsInner<Inner = YTypeRef> {
    fn _insert<V: Into<Value>>(&mut self, key: String, value: V) -> JwstCodecResult {
        if let Some((mut store, mut ty)) = self.as_inner().write() {
            let left = ty.map.get(&SmolStr::new(&key)).cloned();

            // Diagnostic: log left item id and right neighbor for the map slot we're writing into.
            // Useful when debugging cases where an HTTP set succeeds locally but a peer
            // (e.g. y-websocket client) does not see the new value as the live one for `key`.
            let left_dbg = left.as_ref().and_then(|l| l.get()).map(|li| {
                let right_id = li.right.get().map(|ri| ri.id);
                let deleted = li.deleted();
                (li.id, right_id, deleted)
            });
            debug!(
                "ymap._insert key={:?} left_item={:?} (id, right_id, deleted)",
                key, left_dbg
            );

            let item = store.create_item(
                value.into().into(),
                left.unwrap_or(Somr::none()),
                Somr::none(),
                Some(Parent::Type(self.as_inner().clone())),
                Some(SmolStr::new(key.clone())),
            );
            store.integrate(Node::Item(item), 0, Some(&mut ty))?;

            // After integrate, log what `parent.map[key]` ended up pointing to and whether it's deleted.
            let after = ty.map.get(&SmolStr::new(&key)).and_then(|n| n.get()).map(|i| (i.id, i.deleted()));
            debug!("ymap._insert key={:?} AFTER integrate parent.map[key]={:?}", key, after);
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

    /// Two clients concurrently set the SAME map key while offline, then their
    /// updates are merged into a third doc in BOTH possible orders. The merged
    /// doc must pick the same winner regardless of merge order, and that winner
    /// must be the one with the HIGHER client id (yjs's tie-break: lower client
    /// id goes to the LEFT, so the higher one stays as the rightmost / live item).
    ///
    /// This is the regression test for
    /// `apps/keck/doctor_prod_userName_wuyulong1.before_fix.bin`, where the
    /// `visit_status.updateTime` slot had two concurrent items
    /// (clients `1635070698` and `2258317784`, both with `origin_left = None`)
    /// and the keck server's view of the live value diverged from the y-websocket
    /// peer's view because `Node::head()` collapsed to an empty `Somr` and
    /// promoted whichever item integrated last to `parent.map[key]`.
    #[test]
    fn test_map_concurrent_set_same_key_picks_higher_client() {
        // Two distinct clients writing the same key while offline.
        const CLIENT_LOW: u64 = 1_635_070_698;
        const CLIENT_HIGH: u64 = 2_258_317_784;

        let update_low = {
            let doc = Doc::with_client(CLIENT_LOW);
            let mut m = doc.get_or_create_map("m").unwrap();
            m.insert("k".to_string(), "low_value").unwrap();
            doc.encode_update_v1().unwrap()
        };
        let update_high = {
            let doc = Doc::with_client(CLIENT_HIGH);
            let mut m = doc.get_or_create_map("m").unwrap();
            m.insert("k".to_string(), "high_value").unwrap();
            doc.encode_update_v1().unwrap()
        };

        // Apply LOW then HIGH.
        let merged_low_first = {
            let mut doc = Doc::new();
            doc.apply_update_from_binary_v1(update_low.clone()).unwrap();
            doc.apply_update_from_binary_v1(update_high.clone()).unwrap();
            doc.get_or_create_map("m").unwrap().get("k").unwrap()
        };
        // Apply HIGH then LOW.
        let merged_high_first = {
            let mut doc = Doc::new();
            doc.apply_update_from_binary_v1(update_high).unwrap();
            doc.apply_update_from_binary_v1(update_low).unwrap();
            doc.get_or_create_map("m").unwrap().get("k").unwrap()
        };

        // Both merge orders must pick the SAME winner (deterministic CRDT),
        // and that winner is the higher-client item ("high_value"), matching yjs.
        assert_eq!(merged_low_first, Value::Any(Any::String("high_value".to_string())));
        assert_eq!(merged_high_first, Value::Any(Any::String("high_value".to_string())));
    }
}
