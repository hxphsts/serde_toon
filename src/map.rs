//! Ordered map type for TOON objects.
//!
//! This module provides [`ToonMap`], a wrapper around [`IndexMap`] that maintains
//! insertion order for object fields. This is important for TOON as field order
//! affects serialization output (fields are typically sorted alphabetically for
//! deterministic output in tabular format).
//!
//! ## Why IndexMap?
//!
//! TOON uses `IndexMap` instead of `HashMap` to ensure:
//!
//! - **Deterministic output**: Fields serialize in a consistent order
//! - **Iteration order**: Fields are iterated in insertion order
//! - **Compatibility**: Easier testing and debugging with predictable output
//!
//! ## Examples
//!
//! ```rust
//! use serde_toon::{ToonMap, Value};
//!
//! let mut map = ToonMap::new();
//! map.insert("name".to_string(), Value::from("Alice"));
//! map.insert("age".to_string(), Value::from(30));
//!
//! assert_eq!(map.len(), 2);
//! assert_eq!(map.get("name").and_then(|v| v.as_str()), Some("Alice"));
//! ```

use indexmap::IndexMap;
use std::collections::HashMap;

/// An ordered map of string keys to TOON values.
///
/// This is a thin wrapper around [`IndexMap`] that maintains insertion order,
/// which is important for deterministic TOON serialization.
///
/// # Examples
///
/// ```rust
/// use serde_toon::{ToonMap, Value};
///
/// let mut map = ToonMap::new();
/// map.insert("first".to_string(), Value::from(1));
/// map.insert("second".to_string(), Value::from(2));
///
/// // Iteration maintains insertion order
/// let keys: Vec<_> = map.keys().cloned().collect();
/// assert_eq!(keys, vec!["first", "second"]);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct ToonMap(IndexMap<String, crate::Value>);

impl ToonMap {
    /// Creates an empty `ToonMap`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use serde_toon::ToonMap;
    ///
    /// let map = ToonMap::new();
    /// assert!(map.is_empty());
    /// ```
    #[must_use]
    pub fn new() -> Self {
        ToonMap(IndexMap::new())
    }

    /// Creates an empty `ToonMap` with the specified capacity.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use serde_toon::ToonMap;
    ///
    /// let map = ToonMap::with_capacity(10);
    /// assert!(map.is_empty());
    /// ```
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        ToonMap(IndexMap::with_capacity(capacity))
    }

    /// Inserts a key-value pair into the map.
    ///
    /// If the map already contained this key, the old value is returned.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use serde_toon::{ToonMap, Value};
    ///
    /// let mut map = ToonMap::new();
    /// assert!(map.insert("key".to_string(), Value::from(42)).is_none());
    /// assert!(map.insert("key".to_string(), Value::from(43)).is_some());
    /// ```
    pub fn insert(&mut self, key: String, value: crate::Value) -> Option<crate::Value> {
        self.0.insert(key, value)
    }

    /// Returns a reference to the value corresponding to the key.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use serde_toon::{ToonMap, Value};
    ///
    /// let mut map = ToonMap::new();
    /// map.insert("key".to_string(), Value::from(42));
    /// assert_eq!(map.get("key").and_then(|v| v.as_i64()), Some(42));
    /// ```
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&crate::Value> {
        self.0.get(key)
    }

    /// Returns the number of elements in the map.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use serde_toon::{ToonMap, Value};
    ///
    /// let mut map = ToonMap::new();
    /// assert_eq!(map.len(), 0);
    /// map.insert("key".to_string(), Value::from(42));
    /// assert_eq!(map.len(), 1);
    /// ```
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns `true` if the map contains no elements.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use serde_toon::ToonMap;
    ///
    /// let map = ToonMap::new();
    /// assert!(map.is_empty());
    /// ```
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns an iterator over the keys of the map, in insertion order.
    pub fn keys(&self) -> indexmap::map::Keys<'_, String, crate::Value> {
        self.0.keys()
    }

    /// Returns an iterator over the values of the map, in insertion order.
    pub fn values(&self) -> indexmap::map::Values<'_, String, crate::Value> {
        self.0.values()
    }

    /// Returns an iterator over the key-value pairs of the map, in insertion order.
    pub fn iter(&self) -> indexmap::map::Iter<'_, String, crate::Value> {
        self.0.iter()
    }
}

impl Default for ToonMap {
    fn default() -> Self {
        Self::new()
    }
}

impl From<HashMap<String, crate::Value>> for ToonMap {
    fn from(map: HashMap<String, crate::Value>) -> Self {
        ToonMap(map.into_iter().collect())
    }
}

impl From<ToonMap> for HashMap<String, crate::Value> {
    fn from(map: ToonMap) -> Self {
        map.0.into_iter().collect()
    }
}

impl IntoIterator for ToonMap {
    type Item = (String, crate::Value);
    type IntoIter = indexmap::map::IntoIter<String, crate::Value>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl FromIterator<(String, crate::Value)> for ToonMap {
    fn from_iter<T: IntoIterator<Item = (String, crate::Value)>>(iter: T) -> Self {
        ToonMap(IndexMap::from_iter(iter))
    }
}
