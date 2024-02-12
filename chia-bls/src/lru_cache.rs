use std::collections::HashMap;

// A Least Recently Used Cache of HashMaps

pub type Bytes32 = [u8; 32];
pub type Bytes48 = [u8; 48];

pub struct LRUCache<K, V>{
    cache: HashMap<K, V>,
    order: Vec<K>,
    capacity: usize,
}

impl<K: Eq + std::hash::Hash + Clone, V> LRUCache<K, V> {
    pub fn new(capacity: usize) -> LRUCache<K, V> {
        LRUCache {
            cache: HashMap::new(),
            order: Vec::new(),
            capacity,
        }
    }

    pub fn get(&mut self, key: &K) -> Option<&V> {
        match self.cache.get_mut(&key) {
            Some(value) => {
                if let Some(index) = self.order.iter().position(|&x| x == *key) {
                    // Move the element to the back
                    self.order.push(self.order.remove(index));
                }
                Some(value)  // return value
            }
            None => None,  // not found, return None
        }
    }

    pub fn put(&mut self, key: K, value: V) {
        if self.cache.contains_key(&key) {
            self.cache.insert(key.clone(), value);  // overwrite existing value
            if let Some(index) = self.order.iter().position(|&x| x == key) {
                // Move the element to the back
                self.order.push(self.order.remove(index));
            }
        } else {
            self.cache.insert(key.clone(), value);
            self.order.push(key.clone());
            if self.cache.len() > self.capacity {
                let oldest = self.order.remove(0);
                self.cache.remove(&oldest);
            }
        }
    }

    pub fn remove(&mut self, key: &K) {
        let Some(index) = self.order.iter().position(|&x| x == *key);
        self.cache.remove(key);
        self.order.remove(index);
    }

    pub fn set_capacity(&mut self, new_size: usize) {
        if new_size < self.capacity && self.order.len() > new_size {
            let len = self.order.len();
            let new_len = len.saturating_sub(self.capacity - new_size);
            self.order.truncate(new_len);
        }
        self.capacity = new_size;
    }
}