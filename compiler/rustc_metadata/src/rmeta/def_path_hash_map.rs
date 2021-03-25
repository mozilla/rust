use crate::rmeta::DecodeContext;
use crate::rmeta::EncodeContext;
use crate::rmeta::MetadataBlob;
use rustc_data_structures::fingerprint::Fingerprint;
use rustc_data_structures::owning_ref::OwningRef;
use rustc_hir::def_path_hash_map::DefPathHashMap as DefPathHashMapInner;
use rustc_serialize::{opaque, Decodable, Decoder, Encodable, Encoder};
use rustc_span::def_id::{DefIndex, DefPathHash};

crate struct HashMapConfig;

impl odht::Config for HashMapConfig {
    type Key = DefPathHash;
    type Value = DefIndex;

    type EncodedKey = [u8; 16];
    type EncodedValue = [u8; 4];

    type H = odht::UnHashFn;

    #[inline]
    fn encode_key(k: &DefPathHash) -> [u8; 16] {
        k.0.to_le_bytes()
    }

    #[inline]
    fn encode_value(v: &DefIndex) -> [u8; 4] {
        v.as_u32().to_le_bytes()
    }

    #[inline]
    fn decode_key(k: &[u8; 16]) -> DefPathHash {
        DefPathHash(Fingerprint::from_le_bytes(*k))
    }

    #[inline]
    fn decode_value(v: &[u8; 4]) -> DefIndex {
        DefIndex::from_u32(u32::from_le_bytes(*v))
    }
}

crate enum DefPathHashMap<'tcx> {
    OwnedFromMetadata(odht::HashTable<HashMapConfig, OwningRef<MetadataBlob, [u8]>>),
    BorrowedFromTcx(&'tcx DefPathHashMapInner),
}

impl DefPathHashMap<'tcx> {
    #[inline]
    pub fn def_path_hash_to_def_index(&self, def_path_hash: &DefPathHash) -> Option<DefIndex> {
        match *self {
            DefPathHashMap::OwnedFromMetadata(ref map) => map.get(def_path_hash),
            DefPathHashMap::BorrowedFromTcx(_) => {
                panic!("DefPathHashMap::BorrowedFromTcx variant only exists for serialization")
            }
        }
    }
}

impl<'a, 'tcx> Encodable<EncodeContext<'a, 'tcx>> for DefPathHashMap<'tcx> {
    fn encode(&self, e: &mut EncodeContext<'a, 'tcx>) -> opaque::EncodeResult {
        match *self {
            DefPathHashMap::BorrowedFromTcx(def_path_hash_map) => {
                let bytes = def_path_hash_map.raw_bytes();
                e.emit_usize(bytes.len())?;
                e.emit_raw_bytes(bytes)
            }
            DefPathHashMap::OwnedFromMetadata(_) => {
                panic!("DefPathHashMap::OwnedFromMetadata variant only exists for deserialization")
            }
        }
    }
}

impl<'a, 'tcx> Decodable<DecodeContext<'a, 'tcx>> for DefPathHashMap<'tcx> {
    fn decode(d: &mut DecodeContext<'a, 'tcx>) -> Result<DefPathHashMap<'tcx>, String> {
        // Import TyDecoder so we can access the DecodeContext::position() method
        use crate::rustc_middle::ty::codec::TyDecoder;

        let len = d.read_usize()?;
        let pos = d.position();
        let o = OwningRef::new(d.blob().clone()).map(|x| &x[pos..pos + len]);

        // Although we already have the data we need via the OwningRef, we still need
        // to advance the DecodeContext's position so it's in a valid state after
        // the method. We use read_raw_bytes() for that.
        let _ = d.read_raw_bytes(len);

        let inner = odht::HashTable::from_raw_bytes(o).map_err(|e| format!("{}", e))?;
        Ok(DefPathHashMap::OwnedFromMetadata(inner))
    }
}
