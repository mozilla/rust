native "rust" mod rustrt {
  fn rust_file_is_dir(str path) -> int;
}

fn path_sep() -> str {
  ret _str.unsafe_from_bytes(vec(os_fs.path_sep as u8));
}

type path = str;

fn dirname(path p) -> path {
    auto sep = path_sep();
    check (_str.byte_len(sep) == 1u);
    let int i = _str.rindex(p, sep.(0));
    if (i == -1) {
        ret p;
    }
    ret _str.substr(p, 0u, i as uint);
}

impure fn file_is_dir(path p) -> bool {
  ret rustrt.rust_file_is_dir(p) != 0;
}

impure fn list_dir(path p) -> vec[str] {
  auto pl = _str.byte_len(p);
  if (pl == 0u || p.(pl - 1u) as char != os_fs.path_sep) {
    p += path_sep();
  }
  let vec[str] full_paths = vec();
  for (str filename in os_fs.list_dir(p)) {
    if (!_str.eq(filename, ".")) {if (!_str.eq(filename, "..")) {
      full_paths = _vec.push[str](full_paths, p + filename);
    }}
  }
  ret full_paths;
}
