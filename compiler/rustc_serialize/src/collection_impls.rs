//! Implementations of serialization for structures found in liballoc

use std::hash::{BuildHasher, Hash};

use crate::{Decodable, Decoder, Encodable, Encoder};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, LinkedList, VecDeque};
use std::rc::Rc;
use std::sync::Arc;

use smallvec::{Array, SmallVec};

impl<S: Encoder, A: Array<Item: Encodable<S>>> Encodable<S> for SmallVec<A> {
    fn encode(&self, s: &mut S) -> Result<(), S::Error> {
        let slice: &[A::Item] = self;
        slice.encode(s)
    }
}

impl<D: Decoder, A: Array<Item: Decodable<D>>> Decodable<D> for SmallVec<A> {
    fn decode(d: &mut D) -> Result<SmallVec<A>, D::Error> {
        let len = d.read_usize()?;
        let mut vec = SmallVec::with_capacity(len);
        // FIXME(#48994) - could just be collected into a Result<SmallVec, D::Error>
        for _ in 0..len {
            vec.push(Decodable::decode(d)?);
        }
        Ok(vec)
    }
}

impl<S: Encoder, T: Encodable<S>> Encodable<S> for LinkedList<T> {
    fn encode(&self, s: &mut S) -> Result<(), S::Error> {
        s.emit_usize(self.len())?;
        for e in self.iter() {
            e.encode(s)?;
        }
        Ok(())
    }
}

impl<D: Decoder, T: Decodable<D>> Decodable<D> for LinkedList<T> {
    fn decode(d: &mut D) -> Result<LinkedList<T>, D::Error> {
        let len = d.read_usize()?;
        let mut list = LinkedList::new();
        for _ in 0..len {
            list.push_back(Decodable::decode(d)?);
        }
        Ok(list)
    }
}

impl<S: Encoder, T: Encodable<S>> Encodable<S> for VecDeque<T> {
    fn encode(&self, s: &mut S) -> Result<(), S::Error> {
        s.emit_usize(self.len())?;
        for e in self.iter() {
            e.encode(s)?;
        }
        Ok(())
    }
}

impl<D: Decoder, T: Decodable<D>> Decodable<D> for VecDeque<T> {
    fn decode(d: &mut D) -> Result<VecDeque<T>, D::Error> {
        let len = d.read_usize()?;
        let mut deque: VecDeque<T> = VecDeque::with_capacity(len);
        for _ in 0..len {
            deque.push_back(Decodable::decode(d)?);
        }
        Ok(deque)
    }
}

impl<S: Encoder, K, V> Encodable<S> for BTreeMap<K, V>
where
    K: Encodable<S> + PartialEq + Ord,
    V: Encodable<S>,
{
    fn encode(&self, e: &mut S) -> Result<(), S::Error> {
        e.emit_usize(self.len())?;
        for (key, val) in self.iter() {
            key.encode(e)?;
            val.encode(e)?;
        }
        Ok(())
    }
}

impl<D: Decoder, K, V> Decodable<D> for BTreeMap<K, V>
where
    K: Decodable<D> + PartialEq + Ord,
    V: Decodable<D>,
{
    fn decode(d: &mut D) -> Result<BTreeMap<K, V>, D::Error> {
        let len = d.read_usize()?;
        let mut map = BTreeMap::new();
        for _ in 0..len {
            let key = Decodable::decode(d)?;
            let val = Decodable::decode(d)?;
            map.insert(key, val);
        }
        Ok(map)
    }
}

impl<S: Encoder, T> Encodable<S> for BTreeSet<T>
where
    T: Encodable<S> + PartialEq + Ord,
{
    fn encode(&self, s: &mut S) -> Result<(), S::Error> {
        s.emit_usize(self.len())?;
        for e in self.iter() {
            e.encode(s)?;
        }
        Ok(())
    }
}

impl<D: Decoder, T> Decodable<D> for BTreeSet<T>
where
    T: Decodable<D> + PartialEq + Ord,
{
    fn decode(d: &mut D) -> Result<BTreeSet<T>, D::Error> {
        let len = d.read_usize()?;
        let mut set = BTreeSet::new();
        for _ in 0..len {
            set.insert(Decodable::decode(d)?);
        }
        Ok(set)
    }
}

impl<E: Encoder, K, V, S> Encodable<E> for HashMap<K, V, S>
where
    K: Encodable<E> + Eq,
    V: Encodable<E>,
    S: BuildHasher,
{
    fn encode(&self, e: &mut E) -> Result<(), E::Error> {
        e.emit_usize(self.len())?;
        for (key, val) in self.iter() {
            key.encode(e)?;
            val.encode(e)?;
        }
        Ok(())
    }
}

impl<D: Decoder, K, V, S> Decodable<D> for HashMap<K, V, S>
where
    K: Decodable<D> + Hash + Eq,
    V: Decodable<D>,
    S: BuildHasher + Default,
{
    fn decode(d: &mut D) -> Result<HashMap<K, V, S>, D::Error> {
        let len = d.read_usize()?;
        let state = Default::default();
        let mut map = HashMap::with_capacity_and_hasher(len, state);
        for _ in 0..len {
            let key = Decodable::decode(d)?;
            let val = Decodable::decode(d)?;
            map.insert(key, val);
        }
        Ok(map)
    }
}

impl<E: Encoder, T, S> Encodable<E> for HashSet<T, S>
where
    T: Encodable<E> + Eq,
    S: BuildHasher,
{
    fn encode(&self, s: &mut E) -> Result<(), E::Error> {
        s.emit_usize(self.len())?;
        for e in self.iter() {
            e.encode(s)?;
        }
        Ok(())
    }
}

impl<E: Encoder, T, S> Encodable<E> for &HashSet<T, S>
where
    T: Encodable<E> + Eq,
    S: BuildHasher,
{
    fn encode(&self, s: &mut E) -> Result<(), E::Error> {
        (**self).encode(s)
    }
}

impl<D: Decoder, T, S> Decodable<D> for HashSet<T, S>
where
    T: Decodable<D> + Hash + Eq,
    S: BuildHasher + Default,
{
    fn decode(d: &mut D) -> Result<HashSet<T, S>, D::Error> {
        let len = d.read_usize()?;
        let state = Default::default();
        let mut set = HashSet::with_capacity_and_hasher(len, state);
        for _ in 0..len {
            set.insert(Decodable::decode(d)?);
        }
        Ok(set)
    }
}

impl<E: Encoder, K, V, S> Encodable<E> for indexmap::IndexMap<K, V, S>
where
    K: Encodable<E> + Hash + Eq,
    V: Encodable<E>,
    S: BuildHasher,
{
    fn encode(&self, e: &mut E) -> Result<(), E::Error> {
        e.emit_usize(self.len())?;
        for (key, val) in self.iter() {
            key.encode(e)?;
            val.encode(e)?;
        }
        Ok(())
    }
}

impl<D: Decoder, K, V, S> Decodable<D> for indexmap::IndexMap<K, V, S>
where
    K: Decodable<D> + Hash + Eq,
    V: Decodable<D>,
    S: BuildHasher + Default,
{
    fn decode(d: &mut D) -> Result<indexmap::IndexMap<K, V, S>, D::Error> {
        let len = d.read_usize()?;
        let state = Default::default();
        let mut map = indexmap::IndexMap::with_capacity_and_hasher(len, state);
        for _ in 0..len {
            let key = Decodable::decode(d)?;
            let val = Decodable::decode(d)?;
            map.insert(key, val);
        }
        Ok(map)
    }
}

impl<E: Encoder, T, S> Encodable<E> for indexmap::IndexSet<T, S>
where
    T: Encodable<E> + Hash + Eq,
    S: BuildHasher,
{
    fn encode(&self, s: &mut E) -> Result<(), E::Error> {
        s.emit_usize(self.len())?;
        for e in self.iter() {
            e.encode(s)?;
        }
        Ok(())
    }
}

impl<D: Decoder, T, S> Decodable<D> for indexmap::IndexSet<T, S>
where
    T: Decodable<D> + Hash + Eq,
    S: BuildHasher + Default,
{
    fn decode(d: &mut D) -> Result<indexmap::IndexSet<T, S>, D::Error> {
        let len = d.read_usize()?;
        let state = Default::default();
        let mut set = indexmap::IndexSet::with_capacity_and_hasher(len, state);
        for _ in 0..len {
            set.insert(Decodable::decode(d)?);
        }
        Ok(set)
    }
}

impl<E: Encoder, T: Encodable<E>> Encodable<E> for Rc<[T]> {
    fn encode(&self, s: &mut E) -> Result<(), E::Error> {
        let slice: &[T] = self;
        slice.encode(s)
    }
}

impl<D: Decoder, T: Decodable<D>> Decodable<D> for Rc<[T]> {
    fn decode(d: &mut D) -> Result<Rc<[T]>, D::Error> {
        let vec: Vec<T> = Decodable::decode(d)?;
        Ok(vec.into())
    }
}

impl<E: Encoder, T: Encodable<E>> Encodable<E> for Arc<[T]> {
    fn encode(&self, s: &mut E) -> Result<(), E::Error> {
        let slice: &[T] = self;
        slice.encode(s)
    }
}

impl<D: Decoder, T: Decodable<D>> Decodable<D> for Arc<[T]> {
    fn decode(d: &mut D) -> Result<Arc<[T]>, D::Error> {
        let vec: Vec<T> = Decodable::decode(d)?;
        Ok(vec.into())
    }
}
