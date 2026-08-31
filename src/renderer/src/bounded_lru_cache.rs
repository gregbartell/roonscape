use std::cell::RefCell;
use std::collections::VecDeque;

pub(crate) struct BoundedLruCache<K, V> {
    capacity: usize,
    entries: RefCell<VecDeque<(K, V)>>,
}

impl<K: PartialEq, V> BoundedLruCache<K, V> {
    pub(crate) fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "cache capacity must be positive");
        Self {
            capacity,
            entries: RefCell::new(VecDeque::with_capacity(capacity)),
        }
    }

    pub(crate) fn access_and_promote<T>(
        &self,
        key: &K,
        access: impl FnOnce(&mut V) -> T,
    ) -> Option<T> {
        let mut entries = self.entries.borrow_mut();
        let position = entries.iter().position(|(cached, _)| cached == key)?;
        let mut entry = entries
            .remove(position)
            .expect("the located cache entry should exist");
        let result = access(&mut entry.1);
        entries.push_back(entry);
        Some(result)
    }

    pub(crate) fn insert(&self, key: K, value: V) {
        let mut entries = self.entries.borrow_mut();
        if entries.len() == self.capacity {
            entries.pop_front();
        }
        entries.push_back((key, value));
    }
}
