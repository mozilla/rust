use crate::Interner;

use rustc_data_structures::stable_map::FxHashMap;
use rustc_serialize::{Decoder, Encoder};

/// The shorthand encoding uses an enum's variant index `usize`
/// and is offset by this value so it never matches a real variant.
/// This offset is also chosen so that the first byte is never < 0x80.
pub const SHORTHAND_OFFSET: usize = 0x80;

/// Trait for decoding to a reference.
///
/// This is a separate trait from `Decodable` so that we can implement it for
/// upstream types, such as `FxHashSet`.
///
/// The `TyDecodable` derive macro will use this trait for fields that are
/// references (and don't use a type alias to hide that).
///
/// `Decodable` can still be implemented in cases where `Decodable` is required
/// by a trait bound.
pub trait RefDecodable<'tcx, D: TyDecoder> {
    fn decode(d: &mut D) -> Result<&'tcx Self, D::Error>;
}

pub trait TyEncoder: Encoder {
    type I: Interner;
    const CLEAR_CROSS_CRATE: bool;

    fn position(&self) -> usize;
    fn type_shorthands(&mut self) -> &mut FxHashMap<<Self::I as Interner>::Ty, usize>;
    fn predicate_shorthands(&mut self) -> &mut FxHashMap<<Self::I as Interner>::PredicateKind, usize>;
    fn encode_alloc_id(&mut self, alloc_id: &<Self::I as Interner>::AllocId) -> Result<(), Self::Error>;
}


pub trait TyDecoder: Decoder {
    type I: Interner;
    const CLEAR_CROSS_CRATE: bool;

    fn interner(&self) -> Self::I;

    fn peek_byte(&self) -> u8;

    fn position(&self) -> usize;

    fn cached_ty_for_shorthand<F>(
        &mut self,
        shorthand: usize,
        or_insert_with: F,
    ) -> Result<<Self::I as Interner>::Ty, Self::Error>
    where
        F: FnOnce(&mut Self) -> Result<<Self::I as Interner>::Ty, Self::Error>;

    fn with_position<F, R>(&mut self, pos: usize, f: F) -> R
    where
        F: FnOnce(&mut Self) -> R;

    fn positioned_at_shorthand(&self) -> bool {
        (self.peek_byte() & (SHORTHAND_OFFSET as u8)) != 0
    }

    fn decode_alloc_id(&mut self) -> Result<<Self::I as Interner>::AllocId, Self::Error>;
}
