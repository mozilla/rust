fn main() {
    alt @{foo: true, bar: some(10), baz: 20} {
      @{foo: true, bar: some(_), _} {}
      @{foo: false, bar: none, _} {}
    }
}
