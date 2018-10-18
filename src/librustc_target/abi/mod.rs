pub use self::Integer::*;
pub use self::Primitive::*;

use spec::{Target, AddrSpaceIdx, };

use std::fmt;
use std::ops::{Add, Deref, Sub, Mul, AddAssign, Range, RangeInclusive};

use rustc_data_structures::indexed_vec::{Idx, IndexVec};

pub mod call;

/// Parsed [Data layout](http://llvm.org/docs/LangRef.html#data-layout)
/// for a target, which contains everything needed to compute layouts.
pub struct TargetDataLayout {
    pub endian: Endian,
    pub i1_align: AbiAndPrefAlign,
    pub i8_align: AbiAndPrefAlign,
    pub i16_align: AbiAndPrefAlign,
    pub i32_align: AbiAndPrefAlign,
    pub i64_align: AbiAndPrefAlign,
    pub i128_align: AbiAndPrefAlign,
    pub f32_align: AbiAndPrefAlign,
    pub f64_align: AbiAndPrefAlign,
    pub pointers: Vec<Option<(Size, AbiAndPrefAlign)>>,
    pub pointer_size: Size,
    pub pointer_align: AbiAndPrefAlign,
    pub aggregate_align: AbiAndPrefAlign,

    /// Alignments for vector types.
    pub vector_align: Vec<(Size, AbiAndPrefAlign)>,

    pub alloca_address_space: AddrSpaceIdx,
    pub instruction_address_space: AddrSpaceIdx,
}

impl Default for TargetDataLayout {
    /// Creates an instance of `TargetDataLayout`.
    fn default() -> TargetDataLayout {
        let align = |bits| Align::from_bits(bits).unwrap();
        TargetDataLayout {
            endian: Endian::Big,
            i1_align: AbiAndPrefAlign::new(align(8)),
            i8_align: AbiAndPrefAlign::new(align(8)),
            i16_align: AbiAndPrefAlign::new(align(16)),
            i32_align: AbiAndPrefAlign::new(align(32)),
            i64_align: AbiAndPrefAlign { abi: align(32), pref: align(64) },
            i128_align: AbiAndPrefAlign { abi: align(32), pref: align(64) },
            f32_align: AbiAndPrefAlign::new(align(32)),
            f64_align: AbiAndPrefAlign::new(align(64)),
            pointers: vec![],
            pointer_size: Size::from_bits(64),
            pointer_align: AbiAndPrefAlign::new(align(64)),
            aggregate_align: AbiAndPrefAlign { abi: align(0), pref: align(64) },
            alloca_address_space: Default::default(),
            instruction_address_space: Default::default(),
            vector_align: vec![
                (Size::from_bits(64), AbiAndPrefAlign::new(align(64))),
                (Size::from_bits(128), AbiAndPrefAlign::new(align(128))),
            ],
        }
    }
}

impl TargetDataLayout {
    pub fn parse(target: &Target) -> Result<TargetDataLayout, String> {
        // Parse a bit count from a string.
        let parse_bits = |s: &str, kind: &str, cause: &str| {
            s.parse::<u64>().map_err(|err| {
                format!("invalid {} `{}` for `{}` in \"data-layout\": {}",
                        kind, s, cause, err)
            })
        };

        // Parse a size string.
        let size = |s: &str, cause: &str| {
            parse_bits(s, "size", cause).map(Size::from_bits)
        };

        // Parse an alignment string.
        let align = |s: &[&str], cause: &str| {
            if s.is_empty() {
                return Err(format!("missing alignment for `{}` in \"data-layout\"", cause));
            }
            let align_from_bits = |bits| {
                Align::from_bits(bits).map_err(|err| {
                    format!("invalid alignment for `{}` in \"data-layout\": {}",
                            cause, err)
                })
            };
            let abi = parse_bits(s[0], "alignment", cause)?;
            let pref = s.get(1).map_or(Ok(abi), |pref| parse_bits(pref, "alignment", cause))?;
            Ok(AbiAndPrefAlign {
                abi: align_from_bits(abi)?,
                pref: align_from_bits(pref)?,
            })
        };

        fn resize_and_set<T>(vec: &mut Vec<T>, idx: usize, v: T)
            where T: Default,
        {
          while idx >= vec.len() {
            vec.push(T::default());
          }

          vec[idx] = v;
        }

        let mut dl = TargetDataLayout::default();
        let mut i128_align_src = 64;
        for spec in target.data_layout.split('-') {
            match spec.split(':').collect::<Vec<_>>()[..] {
                ["e"] => dl.endian = Endian::Little,
                ["E"] => dl.endian = Endian::Big,
                ["a", ref a..] => dl.aggregate_align = align(a, "a")?,
                ["f32", ref a..] => dl.f32_align = align(a, "f32")?,
                ["f64", ref a..] => dl.f64_align = align(a, "f64")?,
                [p @ "p", s, ref a..] | [p @ "p0", s, ref a..] => {
                    dl.pointer_size = size(s, p)?;
                    dl.pointer_align = align(a, p)?;
                    resize_and_set(&mut dl.pointers, 0, Some((dl.pointer_size,
                                                              dl.pointer_align)));
                },
                [p, s, ref a..] if p.starts_with('p') => {
                  let idx = parse_bits(&p[1..], "u32", "address space index")? as usize;
                  let size = size(s, p)?;
                  let align = align(a, p)?;
                  resize_and_set(&mut dl.pointers, idx, Some((size, align)));
                },
                [ref p] if p.starts_with("P") => {
                    let idx = parse_bits(&p[1..], "u32",
                                         "instruction address space")? as u32;
                    dl.instruction_address_space = AddrSpaceIdx(idx);
                }
               [s, ref a..] if s.starts_with("i") => {
                    let bits = match s[1..].parse::<u64>() {
                        Ok(bits) => bits,
                        Err(_) => {
                            size(&s[1..], "i")?; // For the user error.
                            continue;
                        }
                    };
                    let a = align(a, s)?;
                    match bits {
                        1 => dl.i1_align = a,
                        8 => dl.i8_align = a,
                        16 => dl.i16_align = a,
                        32 => dl.i32_align = a,
                        64 => dl.i64_align = a,
                        _ => {}
                    }
                    if bits >= i128_align_src && bits <= 128 {
                        // Default alignment for i128 is decided by taking the alignment of
                        // largest-sized i{64...128}.
                        i128_align_src = bits;
                        dl.i128_align = a;
                    }
                }
                [s, ref a..] if s.starts_with("v") => {
                    let v_size = size(&s[1..], "v")?;
                    let a = align(a, s)?;
                    if let Some(v) = dl.vector_align.iter_mut().find(|v| v.0 == v_size) {
                        v.1 = a;
                        continue;
                    }
                    // No existing entry, add a new one.
                    dl.vector_align.push((v_size, a));
                },
                [s, ..] if s.starts_with("A") => {
                    // default alloca address space
                    let idx = parse_bits(&s[1..], "u32",
                                         "default alloca address space")? as u32;
                    dl.alloca_address_space = AddrSpaceIdx(idx);
                },
                _ => {} // Ignore everything else.
            }
        }

        // Perform consistency checks against the Target information.
        let endian_str = match dl.endian {
            Endian::Little => "little",
            Endian::Big => "big"
        };
        if endian_str != target.target_endian {
            return Err(format!("inconsistent target specification: \"data-layout\" claims \
                                architecture is {}-endian, while \"target-endian\" is `{}`",
                               endian_str, target.target_endian));
        }

        if dl.pointer_size.bits().to_string() != target.target_pointer_width {
            return Err(format!("inconsistent target specification: \"data-layout\" claims \
                                pointers are {}-bit, while \"target-pointer-width\" is `{}`",
                               dl.pointer_size.bits(), target.target_pointer_width));
        }

        // We don't specialize pointer sizes for specific address spaces,
        // so enforce that the default address space can hold all the bits
        // of any other spaces. Similar for alignment.
        {
            let ptrs_iter = dl.pointers.iter().enumerate()
              .filter_map(|(idx, ptrs)| {
                  ptrs.map(|(s, a)| (idx, s, a) )
              });
            for (idx, size, align) in ptrs_iter {
                if size > dl.pointer_size {
                    return Err(format!("Address space {} pointer is bigger than the default \
                                    pointer: {} vs {}",
                                       idx, size.bits(), dl.pointer_size.bits()));
                }
                if align.abi > dl.pointer_align.abi {
                    return Err(format!("Address space {} pointer alignment is bigger than the \
                                    default pointer: {} vs {}",
                                       idx, align.abi.bits(), dl.pointer_align.abi.bits()));
                }
            }
        }

        Ok(dl)
    }

    pub fn pointer_info(&self, addr_space: AddrSpaceIdx) -> (Size, AbiAndPrefAlign) {
        self.pointers.get(addr_space.0 as usize)
            .and_then(|&v| v )
            .unwrap_or((self.pointer_size, self.pointer_align))
    }

    /// Return exclusive upper bound on object size.
    ///
    /// The theoretical maximum object size is defined as the maximum positive `isize` value.
    /// This ensures that the `offset` semantics remain well-defined by allowing it to correctly
    /// index every address within an object along with one byte past the end, along with allowing
    /// `isize` to store the difference between any two pointers into an object.
    ///
    /// The upper bound on 64-bit currently needs to be lower because LLVM uses a 64-bit integer
    /// to represent object size in bits. It would need to be 1 << 61 to account for this, but is
    /// currently conservatively bounded to 1 << 47 as that is enough to cover the current usable
    /// address space on 64-bit ARMv8 and x86_64.
    pub fn obj_size_bound(&self) -> u64 {
        match self.pointer_size.bits() {
            16 => 1 << 15,
            32 => 1 << 31,
            64 => 1 << 47,
            bits => panic!("obj_size_bound: unknown pointer bit size {}", bits)
        }
    }

    pub fn ptr_sized_integer(&self) -> Integer {
        match self.pointer_size.bits() {
            16 => I16,
            32 => I32,
            64 => I64,
            bits => panic!("ptr_sized_integer: unknown pointer bit size {}", bits)
        }
    }

    pub fn vector_align(&self, vec_size: Size) -> AbiAndPrefAlign {
        for &(size, align) in &self.vector_align {
            if size == vec_size {
                return align;
            }
        }
        // Default to natural alignment, which is what LLVM does.
        // That is, use the size, rounded up to a power of 2.
        AbiAndPrefAlign::new(Align::from_bytes(vec_size.bytes().next_power_of_two()).unwrap())
    }
}

pub trait HasDataLayout {
    fn data_layout(&self) -> &TargetDataLayout;
}

impl HasDataLayout for TargetDataLayout {
    fn data_layout(&self) -> &TargetDataLayout {
        self
    }
}

/// Endianness of the target, which must match cfg(target-endian).
#[derive(Copy, Clone, PartialEq)]
pub enum Endian {
    Little,
    Big
}

/// Size of a type in bytes.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, RustcEncodable, RustcDecodable)]
pub struct Size {
    raw: u64
}

impl Size {
    pub const ZERO: Size = Self::from_bytes(0);

    #[inline]
    pub fn from_bits(bits: u64) -> Size {
        // Avoid potential overflow from `bits + 7`.
        Size::from_bytes(bits / 8 + ((bits % 8) + 7) / 8)
    }

    #[inline]
    pub const fn from_bytes(bytes: u64) -> Size {
        Size {
            raw: bytes
        }
    }

    #[inline]
    pub fn bytes(self) -> u64 {
        self.raw
    }

    #[inline]
    pub fn bits(self) -> u64 {
        self.bytes().checked_mul(8).unwrap_or_else(|| {
            panic!("Size::bits: {} bytes in bits doesn't fit in u64", self.bytes())
        })
    }

    #[inline]
    pub fn align_to(self, align: Align) -> Size {
        let mask = align.bytes() - 1;
        Size::from_bytes((self.bytes() + mask) & !mask)
    }

    #[inline]
    pub fn is_aligned(self, align: Align) -> bool {
        let mask = align.bytes() - 1;
        self.bytes() & mask == 0
    }

    #[inline]
    pub fn checked_add<C: HasDataLayout>(self, offset: Size, cx: &C) -> Option<Size> {
        let dl = cx.data_layout();

        let bytes = self.bytes().checked_add(offset.bytes())?;

        if bytes < dl.obj_size_bound() {
            Some(Size::from_bytes(bytes))
        } else {
            None
        }
    }

    #[inline]
    pub fn checked_mul<C: HasDataLayout>(self, count: u64, cx: &C) -> Option<Size> {
        let dl = cx.data_layout();

        let bytes = self.bytes().checked_mul(count)?;
        if bytes < dl.obj_size_bound() {
            Some(Size::from_bytes(bytes))
        } else {
            None
        }
    }
}

// Panicking addition, subtraction and multiplication for convenience.
// Avoid during layout computation, return `LayoutError` instead.

impl Add for Size {
    type Output = Size;
    #[inline]
    fn add(self, other: Size) -> Size {
        Size::from_bytes(self.bytes().checked_add(other.bytes()).unwrap_or_else(|| {
            panic!("Size::add: {} + {} doesn't fit in u64", self.bytes(), other.bytes())
        }))
    }
}

impl Sub for Size {
    type Output = Size;
    #[inline]
    fn sub(self, other: Size) -> Size {
        Size::from_bytes(self.bytes().checked_sub(other.bytes()).unwrap_or_else(|| {
            panic!("Size::sub: {} - {} would result in negative size", self.bytes(), other.bytes())
        }))
    }
}

impl Mul<Size> for u64 {
    type Output = Size;
    #[inline]
    fn mul(self, size: Size) -> Size {
        size * self
    }
}

impl Mul<u64> for Size {
    type Output = Size;
    #[inline]
    fn mul(self, count: u64) -> Size {
        match self.bytes().checked_mul(count) {
            Some(bytes) => Size::from_bytes(bytes),
            None => {
                panic!("Size::mul: {} * {} doesn't fit in u64", self.bytes(), count)
            }
        }
    }
}

impl AddAssign for Size {
    #[inline]
    fn add_assign(&mut self, other: Size) {
        *self = *self + other;
    }
}

/// Alignment of a type in bytes (always a power of two).
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, RustcEncodable, RustcDecodable)]
pub struct Align {
    pow2: u8,
}

impl Align {
    pub fn from_bits(bits: u64) -> Result<Align, String> {
        Align::from_bytes(Size::from_bits(bits).bytes())
    }

    pub fn from_bytes(align: u64) -> Result<Align, String> {
        // Treat an alignment of 0 bytes like 1-byte alignment.
        if align == 0 {
            return Ok(Align { pow2: 0 });
        }

        let mut bytes = align;
        let mut pow2: u8 = 0;
        while (bytes & 1) == 0 {
            pow2 += 1;
            bytes >>= 1;
        }
        if bytes != 1 {
            return Err(format!("`{}` is not a power of 2", align));
        }
        if pow2 > 29 {
            return Err(format!("`{}` is too large", align));
        }

        Ok(Align { pow2 })
    }

    pub fn bytes(self) -> u64 {
        1 << self.pow2
    }

    pub fn bits(self) -> u64 {
        self.bytes() * 8
    }

    /// Compute the best alignment possible for the given offset
    /// (the largest power of two that the offset is a multiple of).
    ///
    /// N.B., for an offset of `0`, this happens to return `2^64`.
    pub fn max_for_offset(offset: Size) -> Align {
        Align {
            pow2: offset.bytes().trailing_zeros() as u8,
        }
    }

    /// Lower the alignment, if necessary, such that the given offset
    /// is aligned to it (the offset is a multiple of the alignment).
    pub fn restrict_for_offset(self, offset: Size) -> Align {
        self.min(Align::max_for_offset(offset))
    }
}

/// A pair of aligments, ABI-mandated and preferred.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug, RustcEncodable, RustcDecodable)]
pub struct AbiAndPrefAlign {
    pub abi: Align,
    pub pref: Align,
}

impl AbiAndPrefAlign {
    pub fn new(align: Align) -> AbiAndPrefAlign {
        AbiAndPrefAlign {
            abi: align,
            pref: align,
        }
    }

    pub fn min(self, other: AbiAndPrefAlign) -> AbiAndPrefAlign {
        AbiAndPrefAlign {
            abi: self.abi.min(other.abi),
            pref: self.pref.min(other.pref),
        }
    }

    pub fn max(self, other: AbiAndPrefAlign) -> AbiAndPrefAlign {
        AbiAndPrefAlign {
            abi: self.abi.max(other.abi),
            pref: self.pref.max(other.pref),
        }
    }
}

/// Integers, also used for enum discriminants.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Integer {
    I8,
    I16,
    I32,
    I64,
    I128,
}

impl Integer {
    pub fn size(self) -> Size {
        match self {
            I8 => Size::from_bytes(1),
            I16 => Size::from_bytes(2),
            I32 => Size::from_bytes(4),
            I64  => Size::from_bytes(8),
            I128  => Size::from_bytes(16),
        }
    }

    pub fn align<C: HasDataLayout>(self, cx: &C) -> AbiAndPrefAlign {
        let dl = cx.data_layout();

        match self {
            I8 => dl.i8_align,
            I16 => dl.i16_align,
            I32 => dl.i32_align,
            I64 => dl.i64_align,
            I128 => dl.i128_align,
        }
    }

    /// Find the smallest Integer type which can represent the signed value.
    pub fn fit_signed(x: i128) -> Integer {
        match x {
            -0x0000_0000_0000_0080..=0x0000_0000_0000_007f => I8,
            -0x0000_0000_0000_8000..=0x0000_0000_0000_7fff => I16,
            -0x0000_0000_8000_0000..=0x0000_0000_7fff_ffff => I32,
            -0x8000_0000_0000_0000..=0x7fff_ffff_ffff_ffff => I64,
            _ => I128
        }
    }

    /// Find the smallest Integer type which can represent the unsigned value.
    pub fn fit_unsigned(x: u128) -> Integer {
        match x {
            0..=0x0000_0000_0000_00ff => I8,
            0..=0x0000_0000_0000_ffff => I16,
            0..=0x0000_0000_ffff_ffff => I32,
            0..=0xffff_ffff_ffff_ffff => I64,
            _ => I128,
        }
    }

    /// Find the smallest integer with the given alignment.
    pub fn for_align<C: HasDataLayout>(cx: &C, wanted: Align) -> Option<Integer> {
        let dl = cx.data_layout();

        for &candidate in &[I8, I16, I32, I64, I128] {
            if wanted == candidate.align(dl).abi && wanted.bytes() == candidate.size().bytes() {
                return Some(candidate);
            }
        }
        None
    }

    /// Find the largest integer with the given alignment or less.
    pub fn approximate_align<C: HasDataLayout>(cx: &C, wanted: Align) -> Integer {
        let dl = cx.data_layout();

        // FIXME(eddyb) maybe include I128 in the future, when it works everywhere.
        for &candidate in &[I64, I32, I16] {
            if wanted >= candidate.align(dl).abi && wanted.bytes() >= candidate.size().bytes() {
                return candidate;
            }
        }
        I8
    }
}


#[derive(Clone, PartialEq, Eq, RustcEncodable, RustcDecodable, Hash, Copy,
         PartialOrd, Ord)]
pub enum FloatTy {
    F32,
    F64,
}

impl fmt::Debug for FloatTy {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl fmt::Display for FloatTy {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.ty_to_string())
    }
}

impl FloatTy {
    pub fn ty_to_string(self) -> &'static str {
        match self {
            FloatTy::F32 => "f32",
            FloatTy::F64 => "f64",
        }
    }

    pub fn bit_width(self) -> usize {
        match self {
            FloatTy::F32 => 32,
            FloatTy::F64 => 64,
        }
    }
}

/// Fundamental unit of memory access and layout.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub enum Primitive {
    /// The `bool` is the signedness of the `Integer` type.
    ///
    /// One would think we would not care about such details this low down,
    /// but some ABIs are described in terms of C types and ISAs where the
    /// integer arithmetic is done on {sign,zero}-extended registers, e.g.
    /// a negative integer passed by zero-extension will appear positive in
    /// the callee, and most operations on it will produce the wrong values.
    Int(Integer, bool),
    Float(FloatTy),
    Pointer
}

impl<'a, 'tcx> Primitive {
    pub fn size<C: HasDataLayout>(self, cx: &C) -> Size {
        let dl = cx.data_layout();

        match self {
            Int(i, _) => i.size(),
            Float(FloatTy::F32) => Size::from_bits(32),
            Float(FloatTy::F64) => Size::from_bits(64),
            Pointer => dl.pointer_size
        }
    }

    pub fn align<C: HasDataLayout>(self, cx: &C) -> AbiAndPrefAlign {
        let dl = cx.data_layout();

        match self {
            Int(i, _) => i.align(dl),
            Float(FloatTy::F32) => dl.f32_align,
            Float(FloatTy::F64) => dl.f64_align,
            Pointer => dl.pointer_align
        }
    }

    pub fn is_float(self) -> bool {
        match self {
            Float(_) => true,
            _ => false
        }
    }

    pub fn is_int(self) -> bool {
        match self {
            Int(..) => true,
            _ => false,
        }
    }
}

/// Information about one scalar component of a Rust type.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Scalar {
    pub value: Primitive,

    /// Inclusive wrap-around range of valid values, that is, if
    /// start > end, it represents `start..=max_value()`,
    /// followed by `0..=end`.
    ///
    /// That is, for an i8 primitive, a range of `254..=2` means following
    /// sequence:
    ///
    ///    254 (-2), 255 (-1), 0, 1, 2
    ///
    /// This is intended specifically to mirror LLVM’s `!range` metadata,
    /// semantics.
    // FIXME(eddyb) always use the shortest range, e.g., by finding
    // the largest space between two consecutive valid values and
    // taking everything else as the (shortest) valid range.
    pub valid_range: RangeInclusive<u128>,
}

impl Scalar {
    pub fn is_bool(&self) -> bool {
        if let Int(I8, _) = self.value {
            self.valid_range == (0..=1)
        } else {
            false
        }
    }

    /// Returns the valid range as a `x..y` range.
    ///
    /// If `x` and `y` are equal, the range is full, not empty.
    pub fn valid_range_exclusive<C: HasDataLayout>(&self, cx: &C) -> Range<u128> {
        // For a (max) value of -1, max will be `-1 as usize`, which overflows.
        // However, that is fine here (it would still represent the full range),
        // i.e., if the range is everything.
        let bits = self.value.size(cx).bits();
        assert!(bits <= 128);
        let mask = !0u128 >> (128 - bits);
        let start = *self.valid_range.start();
        let end = *self.valid_range.end();
        assert_eq!(start, start & mask);
        assert_eq!(end, end & mask);
        start..(end.wrapping_add(1) & mask)
    }
}

/// Describes how the fields of a type are located in memory.
#[derive(PartialEq, Eq, Hash, Debug)]
pub enum FieldPlacement {
    /// All fields start at no offset. The `usize` is the field count.
    ///
    /// In the case of primitives the number of fields is `0`.
    Union(usize),

    /// Array/vector-like placement, with all fields of identical types.
    Array {
        stride: Size,
        count: u64
    },

    /// Struct-like placement, with precomputed offsets.
    ///
    /// Fields are guaranteed to not overlap, but note that gaps
    /// before, between and after all the fields are NOT always
    /// padding, and as such their contents may not be discarded.
    /// For example, enum variants leave a gap at the start,
    /// where the discriminant field in the enum layout goes.
    Arbitrary {
        /// Offsets for the first byte of each field,
        /// ordered to match the source definition order.
        /// This vector does not go in increasing order.
        // FIXME(eddyb) use small vector optimization for the common case.
        offsets: Vec<Size>,

        /// Maps source order field indices to memory order indices,
        /// depending how fields were permuted.
        // FIXME(camlorn) also consider small vector  optimization here.
        memory_index: Vec<u32>
    }
}

impl FieldPlacement {
    pub fn count(&self) -> usize {
        match *self {
            FieldPlacement::Union(count) => count,
            FieldPlacement::Array { count, .. } => {
                let usize_count = count as usize;
                assert_eq!(usize_count as u64, count);
                usize_count
            }
            FieldPlacement::Arbitrary { ref offsets, .. } => offsets.len()
        }
    }

    pub fn offset(&self, i: usize) -> Size {
        match *self {
            FieldPlacement::Union(_) => Size::ZERO,
            FieldPlacement::Array { stride, count } => {
                let i = i as u64;
                assert!(i < count);
                stride * i
            }
            FieldPlacement::Arbitrary { ref offsets, .. } => offsets[i]
        }
    }

    pub fn memory_index(&self, i: usize) -> usize {
        match *self {
            FieldPlacement::Union(_) |
            FieldPlacement::Array { .. } => i,
            FieldPlacement::Arbitrary { ref memory_index, .. } => {
                let r = memory_index[i];
                assert_eq!(r as usize as u32, r);
                r as usize
            }
        }
    }

    /// Get source indices of the fields by increasing offsets.
    #[inline]
    pub fn index_by_increasing_offset<'a>(&'a self) -> impl Iterator<Item=usize>+'a {
        let mut inverse_small = [0u8; 64];
        let mut inverse_big = vec![];
        let use_small = self.count() <= inverse_small.len();

        // We have to write this logic twice in order to keep the array small.
        if let FieldPlacement::Arbitrary { ref memory_index, .. } = *self {
            if use_small {
                for i in 0..self.count() {
                    inverse_small[memory_index[i] as usize] = i as u8;
                }
            } else {
                inverse_big = vec![0; self.count()];
                for i in 0..self.count() {
                    inverse_big[memory_index[i] as usize] = i as u32;
                }
            }
        }

        (0..self.count()).map(move |i| {
            match *self {
                FieldPlacement::Union(_) |
                FieldPlacement::Array { .. } => i,
                FieldPlacement::Arbitrary { .. } => {
                    if use_small { inverse_small[i] as usize }
                    else { inverse_big[i] as usize }
                }
            }
        })
    }
}

/// Describes how values of the type are passed by target ABIs,
/// in terms of categories of C types there are ABI rules for.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Abi {
    Uninhabited,
    Scalar(Scalar),
    ScalarPair(Scalar, Scalar),
    Vector {
        element: Scalar,
        count: u64
    },
    Aggregate {
        /// If true, the size is exact, otherwise it's only a lower bound.
        sized: bool,
    }
}

impl Abi {
    /// Returns true if the layout corresponds to an unsized type.
    pub fn is_unsized(&self) -> bool {
        match *self {
            Abi::Uninhabited |
            Abi::Scalar(_) |
            Abi::ScalarPair(..) |
            Abi::Vector { .. } => false,
            Abi::Aggregate { sized } => !sized
        }
    }

    /// Returns true if this is a single signed integer scalar
    pub fn is_signed(&self) -> bool {
        match *self {
            Abi::Scalar(ref scal) => match scal.value {
                Primitive::Int(_, signed) => signed,
                _ => false,
            },
            _ => false,
        }
    }

    /// Returns true if this is an uninhabited type
    pub fn is_uninhabited(&self) -> bool {
        match *self {
            Abi::Uninhabited => true,
            _ => false,
        }
    }
}

newtype_index! {
    pub struct VariantIdx { .. }
}

#[derive(PartialEq, Eq, Hash, Debug)]
pub enum Variants {
    /// Single enum variants, structs/tuples, unions, and all non-ADTs.
    Single {
        index: VariantIdx,
    },

    /// General-case enums: for each case there is a struct, and they all have
    /// all space reserved for the tag, and their first field starts
    /// at a non-0 offset, after where the tag would go.
    Tagged {
        tag: Scalar,
        variants: IndexVec<VariantIdx, LayoutDetails>,
    },

    /// Multiple cases distinguished by a niche (values invalid for a type):
    /// the variant `dataful_variant` contains a niche at an arbitrary
    /// offset (field 0 of the enum), which for a variant with discriminant
    /// `d` is set to `(d - niche_variants.start).wrapping_add(niche_start)`.
    ///
    /// For example, `Option<(usize, &T)>`  is represented such that
    /// `None` has a null pointer for the second tuple field, and
    /// `Some` is the identity function (with a non-null reference).
    NicheFilling {
        dataful_variant: VariantIdx,
        niche_variants: RangeInclusive<VariantIdx>,
        niche: Scalar,
        niche_start: u128,
        variants: IndexVec<VariantIdx, LayoutDetails>,
    }
}

#[derive(PartialEq, Eq, Hash, Debug)]
pub struct LayoutDetails {
    pub variants: Variants,
    pub fields: FieldPlacement,
    pub abi: Abi,
    pub align: AbiAndPrefAlign,
    pub size: Size
}

impl LayoutDetails {
    pub fn scalar<C: HasDataLayout>(cx: &C, scalar: Scalar) -> Self {
        let size = scalar.value.size(cx);
        let align = scalar.value.align(cx);
        LayoutDetails {
            variants: Variants::Single { index: VariantIdx::new(0) },
            fields: FieldPlacement::Union(0),
            abi: Abi::Scalar(scalar),
            size,
            align,
        }
    }
}

/// The details of the layout of a type, alongside the type itself.
/// Provides various type traversal APIs (e.g., recursing into fields).
///
/// Note that the details are NOT guaranteed to always be identical
/// to those obtained from `layout_of(ty)`, as we need to produce
/// layouts for which Rust types do not exist, such as enum variants
/// or synthetic fields of enums (i.e., discriminants) and fat pointers.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct TyLayout<'a, Ty> {
    pub ty: Ty,
    pub details: &'a LayoutDetails
}

impl<'a, Ty> Deref for TyLayout<'a, Ty> {
    type Target = &'a LayoutDetails;
    fn deref(&self) -> &&'a LayoutDetails {
        &self.details
    }
}

pub trait LayoutOf {
    type Ty;
    type TyLayout;

    fn layout_of(&self, ty: Self::Ty) -> Self::TyLayout;
}

pub trait TyLayoutMethods<'a, C: LayoutOf<Ty = Self>>: Sized {
    fn for_variant(
        this: TyLayout<'a, Self>,
        cx: &C,
        variant_index: VariantIdx,
    ) -> TyLayout<'a, Self>;
    fn field(this: TyLayout<'a, Self>, cx: &C, i: usize) -> C::TyLayout;
}

impl<'a, Ty> TyLayout<'a, Ty> {
    pub fn for_variant<C>(self, cx: &C, variant_index: VariantIdx) -> Self
    where Ty: TyLayoutMethods<'a, C>, C: LayoutOf<Ty = Ty> {
        Ty::for_variant(self, cx, variant_index)
    }
    pub fn field<C>(self, cx: &C, i: usize) -> C::TyLayout
    where Ty: TyLayoutMethods<'a, C>, C: LayoutOf<Ty = Ty> {
        Ty::field(self, cx, i)
    }
}

impl<'a, Ty> TyLayout<'a, Ty> {
    /// Returns true if the layout corresponds to an unsized type.
    pub fn is_unsized(&self) -> bool {
        self.abi.is_unsized()
    }

    /// Returns true if the type is a ZST and not unsized.
    pub fn is_zst(&self) -> bool {
        match self.abi {
            Abi::Scalar(_) |
            Abi::ScalarPair(..) |
            Abi::Vector { .. } => false,
            Abi::Uninhabited => self.size.bytes() == 0,
            Abi::Aggregate { sized } => sized && self.size.bytes() == 0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use spec::{Target, TargetTriple, };

    #[test]
    fn pointer_size_align() {
        // amdgcn-amd-amdhsa-amdgiz
        const DL: &'static str = "e-p:64:64-p1:64:64-p2:64:64-p3:32:32-\
                                  p4:32:32-p5:32:32-i64:64-v16:16-v24:32-\
                                  v32:32-v48:64-v96:128-v192:256-v256:256-\
                                  v512:512-v1024:1024-v2048:2048-n32:64-A5";

        // Doesn't need to be real...
        let triple = TargetTriple::TargetTriple("x86_64-unknown-linux-gnu".into());
        let mut target = Target::search(&triple).unwrap();
        target.data_layout = DL.into();

        let dl = TargetDataLayout::parse(&target);
        assert!(dl.is_ok());
        let dl = dl.unwrap();

        let default = (dl.pointer_size, dl.pointer_align);

        let thirty_two_size = Size::from_bits(32);
        let thirty_two_align = AbiAndPrefAlign::new(Align::from_bits(32).unwrap());
        let thirty_two = (thirty_two_size, thirty_two_align);
        let sixty_four_size = Size::from_bits(64);
        let sixty_four_align = AbiAndPrefAlign::new(Align::from_bits(64).unwrap());
        let sixty_four = (sixty_four_size, sixty_four_align);

        assert_eq!(dl.pointer_info(AddrSpaceIdx(0)), default);
        assert_eq!(dl.pointer_info(AddrSpaceIdx(0)), sixty_four);
        assert_eq!(dl.pointer_info(AddrSpaceIdx(1)), sixty_four);
        assert_eq!(dl.pointer_info(AddrSpaceIdx(2)), sixty_four);
        assert_eq!(dl.pointer_info(AddrSpaceIdx(3)), thirty_two);
        assert_eq!(dl.pointer_info(AddrSpaceIdx(4)), thirty_two);
        assert_eq!(dl.pointer_info(AddrSpaceIdx(5)), thirty_two);

        // unknown address spaces need to be the same as the default:
        assert_eq!(dl.pointer_info(AddrSpaceIdx(7)), default);
    }

    #[test]
    fn default_is_biggest() {
        // Note p1 is 128 bits.
        const DL: &'static str = "e-p:64:64-p1:128:128-p2:64:64-p3:32:32-\
                                  p4:32:32-p5:32:32-i64:64-v16:16-v24:32-\
                                  v32:32-v48:64-v96:128-v192:256-v256:256-\
                                  v512:512-v1024:1024-v2048:2048-n32:64-A5";

        // Doesn't need to be real...
        let triple = TargetTriple::TargetTriple("x86_64-unknown-linux-gnu".into());
        let mut target = Target::search(&triple).unwrap();
        target.data_layout = DL.into();

        assert!(TargetDataLayout::parse(&target).is_err());
    }
    #[test]
    fn alloca_addr_space() {
        // amdgcn-amd-amdhsa-amdgiz
        const DL: &'static str = "e-p:64:64-p1:64:64-p2:64:64-p3:32:32-\
                                  p4:32:32-p5:32:32-i64:64-v16:16-v24:32-\
                                  v32:32-v48:64-v96:128-v192:256-v256:256-\
                                  v512:512-v1024:1024-v2048:2048-n32:64-A5";

        // Doesn't need to be real...
        let triple = TargetTriple::TargetTriple("x86_64-unknown-linux-gnu".into());
        let mut target = Target::search(&triple).unwrap();
        target.data_layout = DL.into();

        let dl = TargetDataLayout::parse(&target);
        assert!(dl.is_ok());
        let dl = dl.unwrap();

        assert_eq!(dl.alloca_address_space, AddrSpaceIdx(5));
    }
}
