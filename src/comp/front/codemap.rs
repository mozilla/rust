
import std::vec;
import std::str;


/* A codemap is a thing that maps uints to file/line/column positions
 * in a crate. This to make it possible to represent the positions
 * with single-word things, rather than passing records all over the
 * compiler.
 */
type filemap = @rec(str name, uint start_pos, mutable vec[uint] lines);

type codemap = @rec(mutable vec[filemap] files);

type loc = rec(str filename, uint line, uint col);

fn new_codemap() -> codemap {
    let vec[filemap] files = [];
    ret @rec(mutable files=files);
}

fn new_filemap(str filename, uint start_pos) -> filemap {
    ret @rec(name=filename, start_pos=start_pos, mutable lines=[0u]);
}

fn next_line(filemap file, uint pos) { vec::push[uint](file.lines, pos); }

fn lookup_pos(codemap map, uint pos) -> loc {
    auto a = 0u;
    auto b = vec::len[filemap](map.files);
    while (b - a > 1u) {
        auto m = (a + b) / 2u;
        if (map.files.(m).start_pos > pos) { b = m; } else { a = m; }
    }
    auto f = map.files.(a);
    a = 0u;
    b = vec::len[uint](f.lines);
    while (b - a > 1u) {
        auto m = (a + b) / 2u;
        if (f.lines.(m) > pos) { b = m; } else { a = m; }
    }
    ret rec(filename=f.name, line=a + 1u, col=pos - f.lines.(a));
}

fn get_line(filemap fm, int line, &str file) -> str {
    let uint end;
    if ((line as uint) + 1u >= vec::len(fm.lines)) {
        end = str::byte_len(file);
    } else {
        end = fm.lines.(line + 1);
    }
    ret str::slice(file, fm.lines.(line), end);
}

fn get_filemap(codemap cm, str filename) -> filemap {
    for (filemap fm in cm.files) {
        if (fm.name == filename) {
            ret fm;
        }
    }
    //XXjdm the following triggers a mismatched type bug
    //      (or expected function, found _|_)
    fail;// ("asking for " + filename + " which we don't know about");
}
//
// Local Variables:
// mode: rust
// fill-column: 78;
// indent-tabs-mode: nil
// c-basic-offset: 4
// buffer-file-coding-system: utf-8-unix
// compile-command: "make -k -C $RBUILD 2>&1 | sed -e 's/\\/x\\//x:\\//g'";
// End:
//
