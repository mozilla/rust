#include <stdint.h>
#include <assert.h>

#include <stdio.h>

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

/* These symbols are defined by the Rust staticlib built from
 * nonclike.rs. */
extern uint64_t t_add(T a, T b);
extern uint64_t tt_add(TT a, TT b);

int main(int argc, char *argv[]) {
  (void)argc; (void)argv;

  /* This example works. */
  TT xx = { .tag = AA, .aa = { ._0 = 1, ._1 = 2 } };
  TT yy = { .tag = AA, .aa = { ._0 = 10, ._1 = 20 } };
  uint64_t rr = tt_add(xx, yy);
  assert(33 == rr);

  /* This one used to return an incorrect result (see issue #68190). */
  T x = { .tag = A, .a = { ._0 = 1 } };
  T y = { .tag = A, .a = { ._0 = 10 } };
  uint64_t r = t_add(x, y);
  assert(11 == r);

  return 0;
}
