use std::collections::{HashMap, LinkedList};

// A Least Recently Used Cache of HashMaps
struct LRUCache<K, V>{
    cache: HashMap<K, V>,
    order: LinkedList<K>,
    capacity: usize,
}

impl<K: Eq + std::hash::Hash + Clone, V> LRUCache<K, V> {
    fn new(capacity: usize) -> LRUCache<K, V> {
        LRUCache {
            cache: HashMap::new(),
            order: LinkedList::new(),
            capacity,
        }
    }

    fn get(&mut self, key: &K) -> Option<&V> {
        match self.cache.get_mut(key) {
            Some(value) => {
                self.order.remove(key);  // remove from current location
                self.order.push_back(key.clone());  // add to back of list
                Some(value)  // return value
            }
            None => None,  // not found, return None
        }
    }

    fn put(&mut self, key: K, value: V) {
        if self.cache.contains_key(&key) {
            self.cache.insert(key.clone(), value);  // overwrite existing value
            self.order.remove(&key);  // we're moving this to the back, so remove from current location
            self.order.push_back(key);  // add to back of list
        } else {
            self.cache.insert(key.clone(), value);
            self.order.push_back(key.clone());
            if self.cache.len() > self.capacity {
                if let Some(oldest) = self.order.pop_front() {
                    self.cache.remove(&oldest);
                }
            }
        }
    }

    fn remove(&mut self, key: &K) {
        self.cache.remove(key);
        self.order.remove(key);
    }
}