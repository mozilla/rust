#include <stdint.h>

/* This is the code generated by cbindgen 0.12.1 for the `enum TT`
 * type in nonclike.rs . */
enum TT_Tag {
  AA,
  BB,
};
typedef uint8_t TT_Tag;

typedef struct {
  uint64_t _0;
  uint64_t _1;
} AA_Body;

typedef struct {
  TT_Tag tag;
  union {
    AA_Body aa;
  };
} TT;

/* This is the code generated by cbindgen 0.12.1 for the `enum T` type
 * in nonclike.rs . */
enum T_Tag {
  A,
  B,
};
typedef uint8_t T_Tag;

typedef struct {
  uint64_t _0;
} A_Body;

typedef struct {
  T_Tag tag;
  union {
    A_Body a;
  };
} T;

TT tt_new(uint64_t a, uint64_t b) {
  TT tt = {
    .tag = AA,
    .aa = {
      ._0 = a,
      ._1 = b,
    },
  };
  return tt;
}

T t_new(uint64_t a) {
  T t = {
    .tag = A,
    .a = {
      ._0 = a,
    },
  };
  return t;
}
