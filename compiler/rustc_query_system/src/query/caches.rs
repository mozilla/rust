use crate::dep_graph::DepNodeIndex;
use crate::query::plumbing::{QueryCacheStore, QueryLookup};

use rustc_arena::TypedArena;
use rustc_data_structures::fx::FxHashMap;
use rustc_data_structures::sharded::Sharded;
use rustc_data_structures::sync::WorkerLocal;
use std::default::Default;
use std::fmt::Debug;
use std::hash::Hash;
use std::cell::Cell;
use std::marker::PhantomData;

pub trait CacheSelector<K, V> {
    type Cache;
}

pub trait QueryStorage {
    type Value: Debug;
    type Stored: Clone;

    /// Store a value without putting it in the cache.
    /// This is meant to be used with cycle errors.
    fn store_nocache(&self, value: Self::Value) -> Self::Stored;
}

pub trait QueryCache: QueryStorage + Sized {
    type Key: Hash + Eq + Clone + Debug;
    type Sharded: Default;

    /// Checks if the query is already computed and in the cache.
    /// It returns the shard index and a lock guard to the shard,
    /// which will be used if the query is not in the cache and we need
    /// to compute it.
    fn lookup<'s, R, OnHit>(
        &self,
        state: &'s QueryCacheStore<Self>,
        key: &Self::Key,
        // `on_hit` can be called while holding a lock to the query state shard.
        on_hit: OnHit,
    ) -> Result<R, QueryLookup>
    where
        OnHit: FnOnce(&Self::Stored, DepNodeIndex) -> R;

    fn complete(
        &self,
        lock_sharded_storage: &mut Self::Sharded,
        key: Self::Key,
        value: Self::Value,
        index: DepNodeIndex,
    ) -> Self::Stored;

    fn iter(
        &self,
        shards: &Sharded<Self::Sharded>,
        f: &mut dyn FnMut(&Self::Key, &Self::Value, DepNodeIndex),
    );
}

pub struct DefaultCacheSelector;

impl<K: Eq + Hash, V: Clone> CacheSelector<K, V> for DefaultCacheSelector {
    type Cache = DefaultCache<K, V>;
}

pub struct DefaultCache<K, V>(PhantomData<(K, V)>);

impl<K, V> Default for DefaultCache<K, V> {
    fn default() -> Self {
        DefaultCache(PhantomData)
    }
}

impl<K: Eq + Hash, V: Clone + Debug> QueryStorage for DefaultCache<K, V> {
    type Value = V;
    type Stored = V;

    #[inline]
    fn store_nocache(&self, value: Self::Value) -> Self::Stored {
        // We have no dedicated storage
        value
    }
}

impl<K, V> QueryCache for DefaultCache<K, V>
where
    K: Eq + Hash + Clone + Debug,
    V: Clone + Debug,
{
    type Key = K;
    type Sharded = FxHashMap<K, (V, DepNodeIndex)>;

    #[inline(always)]
    fn lookup<'s, R, OnHit>(
        &self,
        state: &'s QueryCacheStore<Self>,
        key: &K,
        on_hit: OnHit,
    ) -> Result<R, QueryLookup>
    where
        OnHit: FnOnce(&V, DepNodeIndex) -> R,
    {
        let (lookup, lock) = state.get_lookup(key);
        let result = lock.raw_entry().from_key_hashed_nocheck(lookup.key_hash, key);

        if let Some((_, value)) = result {
            let hit_result = on_hit(&value.0, value.1);
            Ok(hit_result)
        } else {
            Err(lookup)
        }
    }

    #[inline]
    fn complete(
        &self,
        lock_sharded_storage: &mut Self::Sharded,
        key: K,
        value: V,
        index: DepNodeIndex,
    ) -> Self::Stored {
        lock_sharded_storage.insert(key, (value.clone(), index));
        value
    }

    fn iter(
        &self,
        shards: &Sharded<Self::Sharded>,
        f: &mut dyn FnMut(&Self::Key, &Self::Value, DepNodeIndex),
    ) {
        let shards = shards.lock_shards();
        for shard in shards.iter() {
            for (k, v) in shard.iter() {
                f(k, &v.0, v.1);
            }
        }
    }
}

pub struct ArenaCacheSelector<'tcx>(PhantomData<&'tcx ()>);

impl<'tcx, K: Eq + Hash, V: 'tcx> CacheSelector<K, V> for ArenaCacheSelector<'tcx> {
    type Cache = ArenaCache<'tcx, K, V>;
}

pub struct ArenaCache<'tcx, K, V> {
    arena: WorkerLocal<TypedArena<(V, DepNodeIndex)>>,
    phantom: PhantomData<(K, &'tcx V)>,
}

impl<'tcx, K, V> Default for ArenaCache<'tcx, K, V> {
    fn default() -> Self {
        ArenaCache { arena: WorkerLocal::new(|_| TypedArena::default()), phantom: PhantomData }
    }
}

impl<'tcx, K: Eq + Hash, V: Debug + 'tcx> QueryStorage for ArenaCache<'tcx, K, V> {
    type Value = V;
    type Stored = &'tcx V;

    #[inline]
    fn store_nocache(&self, value: Self::Value) -> Self::Stored {
        let value = self.arena.alloc((value, DepNodeIndex::INVALID));
        let value = unsafe { &*(&value.0 as *const _) };
        &value
    }
}

impl<'tcx, K, V: 'tcx> QueryCache for ArenaCache<'tcx, K, V>
where
    K: Eq + Hash + Clone + Debug,
    V: Debug,
{
    type Key = K;
    type Sharded = FxHashMap<K, &'tcx (V, DepNodeIndex)>;

    #[inline(always)]
    fn lookup<'s, R, OnHit>(
        &self,
        state: &'s QueryCacheStore<Self>,
        key: &K,
        on_hit: OnHit,
    ) -> Result<R, QueryLookup>
    where
        OnHit: FnOnce(&&'tcx V, DepNodeIndex) -> R,
    {
        let (lookup, lock) = state.get_lookup(key);
        let result = lock.raw_entry().from_key_hashed_nocheck(lookup.key_hash, key);

        if let Some((_, value)) = result {
            let hit_result = on_hit(&&value.0, value.1);
            Ok(hit_result)
        } else {
            Err(lookup)
        }
    }

    #[inline]
    fn complete(
        &self,
        lock_sharded_storage: &mut Self::Sharded,
        key: K,
        value: V,
        index: DepNodeIndex,
    ) -> Self::Stored {
        let value = self.arena.alloc((value, index));
        let value = unsafe { &*(value as *const _) };
        lock_sharded_storage.insert(key, value);
        &value.0
    }

    fn iter(
        &self,
        shards: &Sharded<Self::Sharded>,
        f: &mut dyn FnMut(&Self::Key, &Self::Value, DepNodeIndex),
    ) {
        let shards = shards.lock_shards();
        for shard in shards.iter() {
            for (k, v) in shard.iter() {
                f(k, &v.0, v.1);
            }
        }
    }
}

pub struct CellCacheSelector<'tcx>(PhantomData<&'tcx ()>);

impl<'tcx, K: Eq + Hash, V: Clone> CacheSelector<K, V> for CellCacheSelector<'tcx> {
    type Cache = CellCache<K, V>;
}

pub struct CellCache<K, V>(PhantomData<(K, V)>);

impl<K, V> Default for CellCache<K, V> {
    fn default() -> Self {
        CellCache(PhantomData)
    }
}

impl<K, V> CellCache<K, V> {
    const KEY_IS_ZST: () = {
        if std::mem::size_of::<K>() != 0 {
            panic!("Key must be a ZST");
        }
    };
}

impl<K: Eq + Hash, V: Clone + Debug> QueryStorage for CellCache<K, V> {
    type Value = V;
    type Stored = V;

    #[inline]
    fn store_nocache(&self, value: Self::Value) -> Self::Stored {
        // We have no dedicated storage
        value
    }
}

impl<K, V> QueryCache for CellCache<K, V>
where
    K: Eq + Hash + Copy + Clone + Debug,
    V: Copy + Clone + Debug,
{
    type Key = K;
    type Sharded = Cell<Option<(K, V, DepNodeIndex)>>;

    #[inline(always)]
    fn lookup<'s, R, OnHit>(
        &self,
        state: &'s QueryCacheStore<Self>,
        _key: &K,
        on_hit: OnHit,
    ) -> Result<R, QueryLookup>
    where
        OnHit: FnOnce(&V, DepNodeIndex) -> R,
    {
        let _ = Self::KEY_IS_ZST;

        let lock = state.shards.get_shard_by_index(0).lock();
        if let Some((_key, val, dep_node)) = lock.get() {
            Ok(on_hit(&val, dep_node))
        } else {
            Err(QueryLookup {
                key_hash: 0,
                shard: 0
            })
        }
    }

    #[inline]
    fn complete(
        &self,
        lock_sharded_storage: &mut Self::Sharded,
        key: K,
        value: V,
        index: DepNodeIndex,
    ) -> Self::Stored {
        lock_sharded_storage.set(Some((key, value, index)));
        value
    }

    fn iter(
        &self,
        shards: &Sharded<Self::Sharded>,
        f: &mut dyn FnMut(&Self::Key, &Self::Value, DepNodeIndex),
    ) {
        let lock = shards.get_shard_by_index(0).lock();
        if let Some((key, val, dep_node)) = lock.get() {
            f(&key, &val, dep_node);
        }
    }
}
