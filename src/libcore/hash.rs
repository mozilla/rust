/*!
 * Implementation of SipHash 2-4
 *
 * See: http://131002.net/siphash/
 *
 * Consider this as a main "general-purpose" hash for all hashtables: it
 * runs at good speed (competitive with spooky and city) and permits
 * cryptographically strong _keyed_ hashing. Key your hashtables from a
 * CPRNG like rand::rng.
 */

pure fn hash_bytes(buf: &[const u8]) -> u64 {
    ret hash_bytes_keyed(buf, 0u64, 0u64);
}

pure fn hash_bytes_keyed(buf: &[const u8], k0: u64, k1: u64) -> u64 {

    let mut v0 : u64 = k0 ^ 0x736f_6d65_7073_6575;
    let mut v1 : u64 = k1 ^ 0x646f_7261_6e64_6f6d;
    let mut v2 : u64 = k0 ^ 0x6c79_6765_6e65_7261;
    let mut v3 : u64 = k1 ^ 0x7465_6462_7974_6573;

    #macro([#u8to64_le(buf,i),
            (buf[0+i] as u64 |
             buf[1+i] as u64 << 8 |
             buf[2+i] as u64 << 16 |
             buf[3+i] as u64 << 24 |
             buf[4+i] as u64 << 32 |
             buf[5+i] as u64 << 40 |
             buf[6+i] as u64 << 48 |
             buf[7+i] as u64 << 56)]);

    #macro([#rotl(x,b), (x << b) | (x >> (64 - b))]);

    #macro([#compress(v0,v1,v2,v3), {
        v0 += v1; v1 = #rotl(v1, 13); v1 ^= v0; v0 = #rotl(v0, 32);
        v2 += v3; v3 = #rotl(v3, 16); v3 ^= v2;
        v0 += v3; v3 = #rotl(v3, 21); v3 ^= v0;
        v2 += v1; v1 = #rotl(v1, 17); v1 ^= v2; v2 = #rotl(v2, 32);
    }]);

    let len = vec::len(buf);
    let end = len & (!0x7);
    let left = len & 0x7;

    let mut i = 0;
    while i < end {
        let m = #u8to64_le(buf, i);
        v3 ^= m;
        #compress(v0,v1,v2,v3);
        #compress(v0,v1,v2,v3);
        v0 ^= m;
        i += 8;
    }

    let mut b : u64 = (len as u64 & 0xff) << 56;

    if left > 0 { b |= buf[i] as u64; }
    if left > 1 { b |= buf[i + 1] as u64 << 8; }
    if left > 2 { b |= buf[i + 2] as u64 << 16; }
    if left > 3 { b |= buf[i + 3] as u64 << 24; }
    if left > 4 { b |= buf[i + 4] as u64 << 32; }
    if left > 5 { b |= buf[i + 5] as u64 << 40; }
    if left > 6 { b |= buf[i + 6] as u64 << 48; }

    v3 ^= b;
    #compress(v0,v1,v2,v3);
    #compress(v0,v1,v2,v3);
    v0 ^= b;

    v2 ^= 0xff;

    #compress(v0,v1,v2,v3);
    #compress(v0,v1,v2,v3);
    #compress(v0,v1,v2,v3);
    #compress(v0,v1,v2,v3);

    ret v0 ^ v1 ^ v2 ^ v3;
}


iface streaming {
    fn input(~[u8]);
    fn input_str(~str);
    fn result() -> ~[u8];
    fn result_str() -> ~str;
    fn reset();
}

fn siphash(key0 : u64, key1 : u64) -> streaming {
    type sipstate = {
        k0 : u64,
        k1 : u64,
        mut length : uint, // how many bytes we've processed
        mut v0 : u64,      // hash state
        mut v1 : u64,
        mut v2 : u64,
        mut v3 : u64,
        tail : ~[mut u8]/8, // unprocessed bytes
        mut ntail : uint,   //  how many bytes in tail are valid
    };

    fn add_input(st : sipstate, msg : ~[u8]) {
        let length = vec::len(msg);
        st.length += length;

        let mut needed = 0u;

        if st.ntail != 0 {
            needed = 8 - st.ntail;

            if length < needed {

                let mut t = 0;
                while t < length {
                    st.tail[st.ntail+t] = msg[t];
                    t += 1;
                }
                st.ntail += length;

                ret;
            }

            let mut t = 0;
            while t < needed {
                st.tail[st.ntail+t] = msg[t];
                t += 1;
            }

            let m = #u8to64_le(st.tail, 0);

            st.v3 ^= m;
            #compress(st.v0, st.v1, st.v2, st.v3);
            #compress(st.v0, st.v1, st.v2, st.v3);
            st.v0 ^= m;

            st.ntail = 0;
        }

        let len = length - needed;
        let end = len & (!0x7);
        let left = len & 0x7;

        let mut i = needed;
        while i < end {
            let mi = #u8to64_le(msg, i);

            st.v3 ^= mi;
            #compress(st.v0, st.v1, st.v2, st.v3);
            #compress(st.v0, st.v1, st.v2, st.v3);
            st.v0 ^= mi;

            i += 8;
        }

        let mut t = 0u;
        while t < left {
            st.tail[t] = msg[i+t];
            t += 1
        }
        st.ntail = left;
    }

    fn mk_result(st : sipstate) -> ~[u8] {

        let mut v0 = st.v0;
        let mut v1 = st.v1;
        let mut v2 = st.v2;
        let mut v3 = st.v3;

        let mut b : u64 = (st.length as u64 & 0xff) << 56;

        if st.ntail > 0 { b |= st.tail[0] as u64 <<  0; }
        if st.ntail > 1 { b |= st.tail[1] as u64 <<  8; }
        if st.ntail > 2 { b |= st.tail[2] as u64 << 16; }
        if st.ntail > 3 { b |= st.tail[3] as u64 << 24; }
        if st.ntail > 4 { b |= st.tail[4] as u64 << 32; }
        if st.ntail > 5 { b |= st.tail[5] as u64 << 40; }
        if st.ntail > 6 { b |= st.tail[6] as u64 << 48; }

        v3 ^= b;
        #compress(v0, v1, v2, v3);
        #compress(v0, v1, v2, v3);
        v0 ^= b;

        v2 ^= 0xff;
        #compress(v0, v1, v2, v3);
        #compress(v0, v1, v2, v3);
        #compress(v0, v1, v2, v3);
        #compress(v0, v1, v2, v3);

        let h = v0 ^ v1 ^ v2 ^ v3;

        ret ~[
            (h >> 0) as u8,
            (h >> 8) as u8,
            (h >> 16) as u8,
            (h >> 24) as u8,
            (h >> 32) as u8,
            (h >> 40) as u8,
            (h >> 48) as u8,
            (h >> 56) as u8,
        ];
    }

   impl of streaming for sipstate {
        fn reset() {
            self.length = 0;
            self.v0 = self.k0 ^ 0x736f6d6570736575;
            self.v1 = self.k1 ^ 0x646f72616e646f6d;
            self.v2 = self.k0 ^ 0x6c7967656e657261;
            self.v3 = self.k1 ^ 0x7465646279746573;
            self.ntail = 0;
        }
        fn input(msg: ~[u8]) { add_input(self, msg); }
        fn input_str(msg: ~str) { add_input(self, str::bytes(msg)); }
        fn result() -> ~[u8] { ret mk_result(self); }
        fn result_str() -> ~str {
            let r = mk_result(self);
            let mut s = ~"";
            for vec::each(r) |b| { s += uint::to_str(b as uint, 16u); }
            ret s;
        }
    }

    let st = {
        k0 : key0,
        k1 : key1,
        mut length : 0u,
        mut v0 : 0u64,
        mut v1 : 0u64,
        mut v2 : 0u64,
        mut v3 : 0u64,
        tail : ~[mut 0u8,0,0,0,0,0,0,0]/8,
        mut ntail : 0u,
    };

    let sh = st as streaming;
    sh.reset();
    ret sh;
}

#[test]
fn test_siphash() {
    let vecs : [[u8]/8]/64 = [
        [ 0x31, 0x0e, 0x0e, 0xdd, 0x47, 0xdb, 0x6f, 0x72, ]/_,
        [ 0xfd, 0x67, 0xdc, 0x93, 0xc5, 0x39, 0xf8, 0x74, ]/_,
        [ 0x5a, 0x4f, 0xa9, 0xd9, 0x09, 0x80, 0x6c, 0x0d, ]/_,
        [ 0x2d, 0x7e, 0xfb, 0xd7, 0x96, 0x66, 0x67, 0x85, ]/_,
        [ 0xb7, 0x87, 0x71, 0x27, 0xe0, 0x94, 0x27, 0xcf, ]/_,
        [ 0x8d, 0xa6, 0x99, 0xcd, 0x64, 0x55, 0x76, 0x18, ]/_,
        [ 0xce, 0xe3, 0xfe, 0x58, 0x6e, 0x46, 0xc9, 0xcb, ]/_,
        [ 0x37, 0xd1, 0x01, 0x8b, 0xf5, 0x00, 0x02, 0xab, ]/_,
        [ 0x62, 0x24, 0x93, 0x9a, 0x79, 0xf5, 0xf5, 0x93, ]/_,
        [ 0xb0, 0xe4, 0xa9, 0x0b, 0xdf, 0x82, 0x00, 0x9e, ]/_,
        [ 0xf3, 0xb9, 0xdd, 0x94, 0xc5, 0xbb, 0x5d, 0x7a, ]/_,
        [ 0xa7, 0xad, 0x6b, 0x22, 0x46, 0x2f, 0xb3, 0xf4, ]/_,
        [ 0xfb, 0xe5, 0x0e, 0x86, 0xbc, 0x8f, 0x1e, 0x75, ]/_,
        [ 0x90, 0x3d, 0x84, 0xc0, 0x27, 0x56, 0xea, 0x14, ]/_,
        [ 0xee, 0xf2, 0x7a, 0x8e, 0x90, 0xca, 0x23, 0xf7, ]/_,
        [ 0xe5, 0x45, 0xbe, 0x49, 0x61, 0xca, 0x29, 0xa1, ]/_,
        [ 0xdb, 0x9b, 0xc2, 0x57, 0x7f, 0xcc, 0x2a, 0x3f, ]/_,
        [ 0x94, 0x47, 0xbe, 0x2c, 0xf5, 0xe9, 0x9a, 0x69, ]/_,
        [ 0x9c, 0xd3, 0x8d, 0x96, 0xf0, 0xb3, 0xc1, 0x4b, ]/_,
        [ 0xbd, 0x61, 0x79, 0xa7, 0x1d, 0xc9, 0x6d, 0xbb, ]/_,
        [ 0x98, 0xee, 0xa2, 0x1a, 0xf2, 0x5c, 0xd6, 0xbe, ]/_,
        [ 0xc7, 0x67, 0x3b, 0x2e, 0xb0, 0xcb, 0xf2, 0xd0, ]/_,
        [ 0x88, 0x3e, 0xa3, 0xe3, 0x95, 0x67, 0x53, 0x93, ]/_,
        [ 0xc8, 0xce, 0x5c, 0xcd, 0x8c, 0x03, 0x0c, 0xa8, ]/_,
        [ 0x94, 0xaf, 0x49, 0xf6, 0xc6, 0x50, 0xad, 0xb8, ]/_,
        [ 0xea, 0xb8, 0x85, 0x8a, 0xde, 0x92, 0xe1, 0xbc, ]/_,
        [ 0xf3, 0x15, 0xbb, 0x5b, 0xb8, 0x35, 0xd8, 0x17, ]/_,
        [ 0xad, 0xcf, 0x6b, 0x07, 0x63, 0x61, 0x2e, 0x2f, ]/_,
        [ 0xa5, 0xc9, 0x1d, 0xa7, 0xac, 0xaa, 0x4d, 0xde, ]/_,
        [ 0x71, 0x65, 0x95, 0x87, 0x66, 0x50, 0xa2, 0xa6, ]/_,
        [ 0x28, 0xef, 0x49, 0x5c, 0x53, 0xa3, 0x87, 0xad, ]/_,
        [ 0x42, 0xc3, 0x41, 0xd8, 0xfa, 0x92, 0xd8, 0x32, ]/_,
        [ 0xce, 0x7c, 0xf2, 0x72, 0x2f, 0x51, 0x27, 0x71, ]/_,
        [ 0xe3, 0x78, 0x59, 0xf9, 0x46, 0x23, 0xf3, 0xa7, ]/_,
        [ 0x38, 0x12, 0x05, 0xbb, 0x1a, 0xb0, 0xe0, 0x12, ]/_,
        [ 0xae, 0x97, 0xa1, 0x0f, 0xd4, 0x34, 0xe0, 0x15, ]/_,
        [ 0xb4, 0xa3, 0x15, 0x08, 0xbe, 0xff, 0x4d, 0x31, ]/_,
        [ 0x81, 0x39, 0x62, 0x29, 0xf0, 0x90, 0x79, 0x02, ]/_,
        [ 0x4d, 0x0c, 0xf4, 0x9e, 0xe5, 0xd4, 0xdc, 0xca, ]/_,
        [ 0x5c, 0x73, 0x33, 0x6a, 0x76, 0xd8, 0xbf, 0x9a, ]/_,
        [ 0xd0, 0xa7, 0x04, 0x53, 0x6b, 0xa9, 0x3e, 0x0e, ]/_,
        [ 0x92, 0x59, 0x58, 0xfc, 0xd6, 0x42, 0x0c, 0xad, ]/_,
        [ 0xa9, 0x15, 0xc2, 0x9b, 0xc8, 0x06, 0x73, 0x18, ]/_,
        [ 0x95, 0x2b, 0x79, 0xf3, 0xbc, 0x0a, 0xa6, 0xd4, ]/_,
        [ 0xf2, 0x1d, 0xf2, 0xe4, 0x1d, 0x45, 0x35, 0xf9, ]/_,
        [ 0x87, 0x57, 0x75, 0x19, 0x04, 0x8f, 0x53, 0xa9, ]/_,
        [ 0x10, 0xa5, 0x6c, 0xf5, 0xdf, 0xcd, 0x9a, 0xdb, ]/_,
        [ 0xeb, 0x75, 0x09, 0x5c, 0xcd, 0x98, 0x6c, 0xd0, ]/_,
        [ 0x51, 0xa9, 0xcb, 0x9e, 0xcb, 0xa3, 0x12, 0xe6, ]/_,
        [ 0x96, 0xaf, 0xad, 0xfc, 0x2c, 0xe6, 0x66, 0xc7, ]/_,
        [ 0x72, 0xfe, 0x52, 0x97, 0x5a, 0x43, 0x64, 0xee, ]/_,
        [ 0x5a, 0x16, 0x45, 0xb2, 0x76, 0xd5, 0x92, 0xa1, ]/_,
        [ 0xb2, 0x74, 0xcb, 0x8e, 0xbf, 0x87, 0x87, 0x0a, ]/_,
        [ 0x6f, 0x9b, 0xb4, 0x20, 0x3d, 0xe7, 0xb3, 0x81, ]/_,
        [ 0xea, 0xec, 0xb2, 0xa3, 0x0b, 0x22, 0xa8, 0x7f, ]/_,
        [ 0x99, 0x24, 0xa4, 0x3c, 0xc1, 0x31, 0x57, 0x24, ]/_,
        [ 0xbd, 0x83, 0x8d, 0x3a, 0xaf, 0xbf, 0x8d, 0xb7, ]/_,
        [ 0x0b, 0x1a, 0x2a, 0x32, 0x65, 0xd5, 0x1a, 0xea, ]/_,
        [ 0x13, 0x50, 0x79, 0xa3, 0x23, 0x1c, 0xe6, 0x60, ]/_,
        [ 0x93, 0x2b, 0x28, 0x46, 0xe4, 0xd7, 0x06, 0x66, ]/_,
        [ 0xe1, 0x91, 0x5f, 0x5c, 0xb1, 0xec, 0xa4, 0x6c, ]/_,
        [ 0xf3, 0x25, 0x96, 0x5c, 0xa1, 0x6d, 0x62, 0x9f, ]/_,
        [ 0x57, 0x5f, 0xf2, 0x8e, 0x60, 0x38, 0x1b, 0xe5, ]/_,
        [ 0x72, 0x45, 0x06, 0xeb, 0x4c, 0x32, 0x8a, 0x95, ]/_
    ]/_;

    let k0 = 0x_07_06_05_04_03_02_01_00_u64;
    let k1 = 0x_0f_0e_0d_0c_0b_0a_09_08_u64;
    let mut buf : ~[u8] = ~[];
    let mut t = 0;
    let stream_inc = siphash(k0,k1);
    let stream_full = siphash(k0,k1);

    fn to_hex_str(r:[u8]/8) -> ~str {
        let mut s = ~"";
        for vec::each(r) |b| { s += uint::to_str(b as uint, 16u); }
        ret s;
    }

    while t < 64 {
        #debug("siphash test %?", t);
        let vec = #u8to64_le(vecs[t], 0);
        let out = hash_bytes_keyed(buf, k0, k1);
        #debug("got %?, expected %?", out, vec);
        assert vec == out;

        stream_full.reset();
        stream_full.input(buf);
        let f = stream_full.result_str();
        let i = stream_inc.result_str();
        let v = to_hex_str(vecs[t]);
        #debug["%d: (%s) => inc=%s full=%s", t, v, i, f];

        assert f == i && f == v;

        buf += ~[t as u8];
        stream_inc.input(~[t as u8]);

        t += 1;
    }
}
