
import front::ast;
import front::codemap;
import util::common::span;
import util::common::ty_mach;
import std::uint;
import std::term;
import std::io;
import std::map;
import std::option;
import std::option::some;
import std::option::none;
import std::str;

tag os { os_win32; os_macos; os_linux; }

tag arch { arch_x86; arch_x64; arch_arm; }

type config =
    rec(os os,
        arch arch,
        ty_mach int_type,
        ty_mach uint_type,
        ty_mach float_type);

type options =
    rec(bool shared,
        uint optimize,
        bool debuginfo,
        bool verify,
        bool run_typestate,
        bool save_temps,
        bool stats,
        bool time_passes,
        bool time_llvm_passes,
        back::link::output_type output_type,
        vec[str] library_search_paths,
        str sysroot,
        // The crate config requested for the session, which may be combined
        // with additional crate configurations during the compile process
        ast::crate_cfg cfg);

type crate_metadata = rec(str name, vec[u8] data);

fn span_to_str(span sp, codemap::codemap cm) -> str {
    auto lo = codemap::lookup_pos(cm, sp.lo);
    auto hi = codemap::lookup_pos(cm, sp.hi);
    ret #fmt("%s:%u:%u:%u:%u", lo.filename, lo.line, lo.col, hi.line, hi.col);
}

fn emit_diagnostic(option::t[span] sp, str msg, str kind, u8 color,
                   codemap::codemap cm) {
    auto ss = "<input>:0:0:0:0";
    let option::t[@file_lines] maybe_lines = none;
    alt (sp) {
        case (some(?ssp)) {
            ss = span_to_str(ssp, cm);
            maybe_lines = some(span_to_lines(ssp, cm));
        }
        case (none) { }
    }
    io::stdout().write_str(ss + ": ");
    if (term::color_supported()) {
        term::fg(io::stdout().get_buf_writer(), color);
    }
    io::stdout().write_str(#fmt("%s:", kind));
    if (term::color_supported()) {
        term::reset(io::stdout().get_buf_writer());
    }
    io::stdout().write_str(#fmt(" %s\n", msg));
    alt (maybe_lines) {
        case (some(?lines)) {
            auto rdr = io::file_reader(lines.name);
            auto file = str::unsafe_from_bytes(rdr.read_whole_stream());
            auto fm = codemap::get_filemap(cm, lines.name);
            for (uint line in lines.lines) {
                io::stdout().write_str(#fmt("%s:%u ", fm.name, line + 1u));
                auto s = codemap::get_line(fm, line as int, file);
                if (!str::ends_with(s, "\n")) {
                    s += "\n";
                }
                io::stdout().write_str(s);
            }
        }
        case (_) {}
    }
}

type file_lines = rec(str name, vec[uint] lines);

fn span_to_lines(span sp, codemap::codemap cm) -> @file_lines {
    auto lo = codemap::lookup_pos(cm, sp.lo);
    auto hi = codemap::lookup_pos(cm, sp.hi);
    auto lines = [];
    for each (uint i in uint::range(lo.line - 1u, hi.line as uint)) {
        lines += [i];
    }
    ret @rec(name=lo.filename, lines=lines);
}

obj session(ast::crate_num cnum,
            @config targ_cfg,
            @options opts,
            map::hashmap[int, crate_metadata] crates,
            mutable vec[str] used_crate_files,
            mutable vec[str] used_libraries,
            codemap::codemap cm,
            mutable uint err_count) {
    fn get_targ_cfg() -> @config { ret targ_cfg; }
    fn get_opts() -> @options { ret opts; }
    fn get_targ_crate_num() -> ast::crate_num { ret cnum; }
    fn span_fatal(span sp, str msg) -> ! {
        // FIXME: Use constants, but rustboot doesn't know how to export them.

        emit_diagnostic(some(sp), msg, "error", 9u8, cm);
        fail;
    }
    fn fatal(str msg) -> ! {
        emit_diagnostic(none[span], msg, "error", 9u8, cm);
        fail;
    }
    fn span_err(span sp, str msg) {
        emit_diagnostic(some(sp), msg, "error", 9u8, cm);
        err_count += 1u;
    }
    fn err(str msg) {
        emit_diagnostic(none, msg, "error", 9u8, cm);
        err_count += 1u;
    }
    fn abort_if_errors() {
        if (err_count > 0u) {
            self.fatal("aborting due to previous errors");
        }
    }
    fn span_warn(span sp, str msg) {
        // FIXME: Use constants, but rustboot doesn't know how to export them.

        emit_diagnostic(some(sp), msg, "warning", 11u8, cm);
    }
    fn warn(str msg) {
        emit_diagnostic(none[span], msg, "warning", 11u8, cm);
    }
    fn span_note(span sp, str msg) {
        // FIXME: Use constants, but rustboot doesn't know how to export them.

        emit_diagnostic(some(sp), msg, "note", 10u8, cm);
    }
    fn note(str msg) {
        emit_diagnostic(none, msg, "note", 10u8, cm);
    }
    fn span_bug(span sp, str msg) -> ! {
        self.span_fatal(sp, #fmt("internal compiler error %s", msg));
    }
    fn bug(str msg) -> ! {
        self.fatal(#fmt("internal compiler error %s", msg));
    }
    fn span_unimpl(span sp, str msg) -> ! {
        self.span_bug(sp, "unimplemented " + msg);
    }
    fn unimpl(str msg) -> ! { self.bug("unimplemented " + msg); }
    fn get_external_crate(int num) -> crate_metadata { ret crates.get(num); }
    fn set_external_crate(int num, &crate_metadata metadata) {
        crates.insert(num, metadata);
    }
    fn has_external_crate(int num) -> bool { ret crates.contains_key(num); }
    fn add_used_library(&str lib) {
        if (lib == "") {
            ret;
        }
        // A program has a small number of libraries, so a vector is probably
        // a good data structure in here.
        for (str l in used_libraries) {
            if (l == lib) {
                ret;
            }
        }
        used_libraries += [lib];
    }
    fn get_used_libraries() -> vec[str] {
       ret used_libraries;
    }
    fn add_used_crate_file(&str lib) {
        // A program has a small number of crates, so a vector is probably
        // a good data structure in here.
        for (str l in used_crate_files) {
            if (l == lib) {
                ret;
            }
        }
        used_crate_files += [lib];
    }
    fn get_used_crate_files() -> vec[str] {
       ret used_crate_files;
    }
    fn get_codemap() -> codemap::codemap { ret cm; }
    fn lookup_pos(uint pos) -> codemap::loc {
        ret codemap::lookup_pos(cm, pos);
    }
    fn span_str(span sp) -> str { ret span_to_str(sp, self.get_codemap()); }
}
// Local Variables:
// fill-column: 78;
// indent-tabs-mode: nil
// c-basic-offset: 4
// buffer-file-coding-system: utf-8-unix
// compile-command: "make -k -C $RBUILD 2>&1 | sed -e 's/\\/x\\//x:\\//g'";
// End:
