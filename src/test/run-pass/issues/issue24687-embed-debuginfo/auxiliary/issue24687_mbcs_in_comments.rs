use std::fmt;

// This ia file with many multi-byte characters, to try to encourage
// the compiler to trip on them. The Drop implementation below will
// need to have its source location embedded into the debug info for
// the output file.

// αααααααααααααααααααααααααααααααααααααααααααααααααααααααααααααααααααααα
// ββββββββββββββββββββββββββββββββββββββββββββββββββββββββββββββββββββββ
// γγγγγγγγγγγγγγγγγγγγγγγγγγγγγγγγγγγγγγγγγγγγγγγγγγγγγγγγγγγγγγγγγγγγγγ
// δδδδδδδδδδδδδδδδδδδδδδδδδδδδδδδδδδδδδδδδδδδδδδδδδδδδδδδδδδδδδδδδδδδδδδ
// εεεεεεεεεεεεεεεεεεεεεεεεεεεεεεεεεεεεεεεεεεεεεεεεεεεεεεεεεεεεεεεεεεεεεε

// ζζζζζζζζζζζζζζζζζζζζζζζζζζζζζζζζζζζζζζζζζζζζζζζζζζζζζζζζζζζζζζζζζζζζζζ
// ηηηηηηηηηηηηηηηηηηηηηηηηηηηηηηηηηηηηηηηηηηηηηηηηηηηηηηηηηηηηηηηηηηηηηη
// θθθθθθθθθθθθθθθθθθθθθθθθθθθθθθθθθθθθθθθθθθθθθθθθθθθθθθθθθθθθθθθθθθθθθθ
// ιιιιιιιιιιιιιιιιιιιιιιιιιιιιιιιιιιιιιιιιιιιιιιιιιιιιιιιιιιιιιιιιιιιιιι
// κκκκκκκκκκκκκκκκκκκκκκκκκκκκκκκκκκκκκκκκκκκκκκκκκκκκκκκκκκκκκκκκκκκκκκ

pub struct D<X:fmt::Debug>(pub X);

impl<X:fmt::Debug> Drop for D<X> {
    fn drop(&mut self) {
        // λλλλλλλλλλλλλλλλλλλλλλλλλλλλλλλλλλλλλλλλλλλλλλλλλλλλλλλλλλλλ
        println!("Dropping D({:?})", self.0);
    }
}
