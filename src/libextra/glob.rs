// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.


use std::{os, path, uint, util};

use sort;


/**
 * An iterator that yields Paths from the filesystem that match a particular
 * pattern - see the glob function for more details.
 */
pub struct GlobIterator {
    priv root: Path,
    priv dir_patterns: ~[Pattern],
    priv options: MatchOptions,
    priv todo: ~[Path]
}

/**
 * Call glob_with with the default match options.
 *
 * This equivalent to glob_with(pattern, MatchOptions::new())
 */
pub fn glob(pattern: &str) -> GlobIterator {
    glob_with(pattern, MatchOptions::new())
}

/**
 * Return an iterator that produces all the Paths that match the given pattern,
 * which may be absolute or relative to the current working directory.
 *
 * This function accepts Unix shell style patterns as described by Pattern::new(..).
 * The options given are passed through unchanged to Pattern::matches_with(..) with
 * the exception that require_literal_separator is always set to true regardless of the
 * value passed to this function.
 *
 * Paths are yielded in alphabetical order, as absolute paths.
 */
pub fn glob_with(pattern: &str, options: MatchOptions) -> GlobIterator {

    // note that this relies on the glob meta characters not
    // having any special meaning in actual pathnames
    let path = Path(pattern);
    let dir_patterns = path.components.map(|s| Pattern::new(*s));

    let root = if path.is_absolute() {
        Path {components: ~[], .. path} // preserve windows path host/device
    } else {
        os::getcwd()
    };
    let todo = list_dir_sorted(&root);

    GlobIterator {
        root: root,
        dir_patterns: dir_patterns,
        options: options,
        todo: todo,
    }
}

impl Iterator<Path> for GlobIterator {

    fn next(&mut self) -> Option<Path> {
        loop {
            if self.dir_patterns.is_empty() || self.todo.is_empty() {
                return None;
            }

            let path = self.todo.pop();
            let pattern_index = path.components.len() - self.root.components.len() - 1;
            let ref pattern = self.dir_patterns[pattern_index];

            if pattern.matches_with(*path.components.last(), self.options) {

                if pattern_index == self.dir_patterns.len() - 1 {
                    // it is not possible for a pattern to match a directory *AND* its children
                    // so we don't need to check the children
                    return Some(path);
                } else {
                    self.todo.push_all(list_dir_sorted(&path));
                }
            }
        }
    }

}

fn list_dir_sorted(path: &Path) -> ~[Path] {
    let mut children = os::list_dir_path(path);
    sort::quick_sort(children, |p1, p2| p2.components.last() <= p1.components.last());
    children
}

/**
 * A compiled Unix shell style pattern.
 */
pub struct Pattern {
    priv tokens: ~[PatternToken]
}

enum PatternToken {
    Char(char),
    AnyChar,
    AnySequence,
    AnyWithin(~[char]),
    AnyExcept(~[char])
}

impl Pattern {

    /**
     * This function compiles Unix shell style patterns:
     *   '?' matches any single character.
     *   '*' matches any (possibly empty) sequence of characters.
     *   '[...]' matches any character inside the brackets, unless the first character
     *           is '!' in which case it matches any character except those between
     *           the '!' and the ']'.
     *
     * The metacharacters '?', '*', '[', ']' can be matched by using brackets (e.g. '[?]').
     * When a ']' occurs immediately following '[' or '[!' then it is interpreted as
     * being part of, rather then ending, the character set, so ']' and NOT ']' can be
     * matched by '[]]' and '[!]]' respectively.
     *
     * This function will fail if the given pattern string is malformed - e.g. if it contains
     * a '[...]' sequence that is missing its closing ']'.
     */
    pub fn new(pattern: &str) -> Pattern {
        let mut tokens = ~[];

        let mut pattern_iter = pattern.iter();
        loop {
            let pchar = match pattern_iter.next() {
                None => break,
                Some(c) => c,
            };
            match pchar {
                '?' => {
                    tokens.push(AnyChar);
                }
                '*' => {
                    tokens.push(AnySequence);
                }
                '[' => {

                    // get the next char, or fail with a helpful message
                    let next_pattern_char = || {
                        match pattern_iter.next() {
                            None => fail!("invalid pattern syntax due to unclosed bracket: %s",
                                          pattern),
                            Some(c) => c,
                        }
                    };

                    let c = next_pattern_char();
                    let is_except = (c == '!');

                    let mut chars = ~[];
                    if is_except {
                        chars.push(next_pattern_char());
                    } else {
                        chars.push(c);
                    };

                    loop {
                        match next_pattern_char() {
                            ']' => break,
                            c => chars.push(c),
                        }
                    }
                    tokens.push(if is_except { AnyExcept(chars) } else { AnyWithin(chars) });
                }
                c => {
                    tokens.push(Char(c));
                }
            }
        }

        Pattern { tokens: tokens }
    }

    /**
     * Escape metacharacters within the given string by surrounding them in
     * brackets. The resulting string will, when compiled into a Pattern,
     * match the input string and nothing else.
     */
    pub fn escape(s: &str) -> ~str {
        let mut escaped = ~"";
        foreach c in s.iter() {
            match c {
                // note that ! does not need escaping because it is only special inside brackets
                '?' | '*' | '[' | ']' => {
                    escaped.push_char('[');
                    escaped.push_char(c);
                    escaped.push_char(']');
                }
                c => {
                    escaped.push_char(c);
                }
            }
        }
        escaped
    }

    /**
     * Return if the given str matches this Pattern using the default
     * match options (i.e. MatchOptions::new()).
     */
    pub fn matches(&self, file: &str) -> bool {
        self.matches_with(file, MatchOptions::new())
    }

    /**
     * Return if the given Path, when converted to a str, matches this Pattern
     * using the default match options (i.e. MatchOptions::new()).
     */
    pub fn matches_path(&self, path: &Path) -> bool {
        self.matches(path.to_str())
    }

    /**
     * Return if the given str matches this Pattern using the specified match options.
     */
    pub fn matches_with(&self, file: &str, options: MatchOptions) -> bool {
        self.matches_from(file, 0, options)
    }

    /**
     * Return if the given Path, when converted to a str, matches this Pattern
     * using the specified match options.
     */
    pub fn matches_path_with(&self, path: &Path, options: MatchOptions) -> bool {
        self.matches_with(path.to_str(), options)
    }

    fn matches_from(&self, mut file: &str, i: uint, options: MatchOptions) -> bool {

        for uint::range(i, self.tokens.len()) |ti| {
            match self.tokens[ti] {
                AnySequence => {
                    loop {
                        if self.matches_from(file, ti + 1, options) {
                            return true;
                        }

                        if file.is_empty() {
                            return false;
                        }

                        let (c, next) = file.slice_shift_char();
                        if options.require_literal_separator && is_sep(c) {
                            return false;
                        }

                        file = next;
                    }
                }
                _ => {
                    if file.is_empty() {
                        return false;
                    }

                    let (c, next) = file.slice_shift_char();
                    let require_literal = options.require_literal_separator && is_sep(c);

                    let matches = match self.tokens[ti] {
                        AnyChar => {
                            !require_literal
                        }
                        AnyWithin(ref chars) => {
                            !require_literal &&
                            chars.rposition(|&e| chars_eq(e, c, options.case_sensitive)).is_some()
                        }
                        AnyExcept(ref chars) => {
                            !require_literal &&
                            chars.rposition(|&e| chars_eq(e, c, options.case_sensitive)).is_none()
                        }
                        Char(c2) => {
                            chars_eq(c, c2, options.case_sensitive)
                        }
                        AnySequence => {
                            util::unreachable()
                        }
                    };
                    if !matches {
                        return false;
                    }
                    file = next;
                }
            }
        }

        file.is_empty()
    }

}

/// A helper function to determine if two chars are (possibly case-insensitively) equal.
fn chars_eq(a: char, b: char, case_sensitive: bool) -> bool {
    // FIXME: work with non-ascii chars properly (issue #1347)
    if !case_sensitive && a.is_ascii() && b.is_ascii() {
        a.to_ascii().eq_ignore_case(b.to_ascii())
    } else {
        a == b
    }
}

/// A helper function to determine if a char is a path separator on the current platform.
#[cfg(windows)]
fn is_sep(c: char) -> bool {
    path::windows::is_sep(c)
}
#[cfg(unix)]
fn is_sep(c: char) -> bool {
    path::posix::is_sep(c)
}

/**
 * Configuration options to modify the behaviour of Pattern::matches_with(..)
 */
pub struct MatchOptions {
    /// Whether or not patterns should be matched in a case-sensitive manner.
    case_sensitive: bool,
    /// If this is true then path-component separator characters (e.g. '/' on Posix)
    /// must be matched by a literal '/', rather than by '*' or '?' or '[...]'
    require_literal_separator: bool
}

impl MatchOptions {

    /**
     * Constructs a new MatchOptions with default field values. This is used
     * when calling functions that do not take an explicit MatchOptions parameter.
     *
     * This function always returns this value:
     *     MatchOptions {
     *         case_sensitive: true,
     *         require_literal_separator: false
     *     }
     */
    pub fn new() -> MatchOptions {
        MatchOptions {
            case_sensitive: true,
            require_literal_separator: false
        }
    }

}

#[cfg(test)]
mod test {
    use std::{io, os, unstable};
    use super::*;

    #[test]
    fn test_relative_pattern() {

        fn mk_file(path: &str, directory: bool) {
            if directory {
                os::make_dir(&Path(path), 0xFFFF);
            } else {
                io::mk_file_writer(&Path(path), [io::Create]);
            }
        }

        fn abs_path(path: &str) -> Path {
            os::getcwd().push_many(Path(path).components)
        }

        fn glob_vec(pattern: &str) -> ~[Path] {
            glob(pattern).collect()
        }

        mk_file("tmp", true);
        mk_file("tmp/glob-tests", true);

        do unstable::change_dir_locked(&Path("tmp/glob-tests")) {

            mk_file("aaa", true);
            mk_file("aaa/apple", true);
            mk_file("aaa/orange", true);
            mk_file("aaa/tomato", true);
            mk_file("aaa/tomato/tomato.txt", false);
            mk_file("aaa/tomato/tomoto.txt", false);
            mk_file("bbb", true);
            mk_file("bbb/specials", true);
            mk_file("bbb/specials/!", false);

            // windows does not allow '*' or '?' characters to exist in filenames
            if os::consts::FAMILY != os::consts::windows::FAMILY {
                mk_file("bbb/specials/*", false);
                mk_file("bbb/specials/?", false);
            }

            mk_file("bbb/specials/[", false);
            mk_file("bbb/specials/]", false);
            mk_file("ccc", true);
            mk_file("xyz", true);
            mk_file("xyz/x", false);
            mk_file("xyz/y", false);
            mk_file("xyz/z", false);

            assert_eq!(glob_vec(""), ~[]);
            assert_eq!(glob_vec("."), ~[]);
            assert_eq!(glob_vec(".."), ~[]);

            assert_eq!(glob_vec("aaa"), ~[abs_path("aaa")]);
            assert_eq!(glob_vec("aaa/"), ~[abs_path("aaa")]);
            assert_eq!(glob_vec("a"), ~[]);
            assert_eq!(glob_vec("aa"), ~[]);
            assert_eq!(glob_vec("aaaa"), ~[]);

            assert_eq!(glob_vec("aaa/apple"), ~[abs_path("aaa/apple")]);
            assert_eq!(glob_vec("aaa/apple/nope"), ~[]);

            // windows should support both / and \ as directory separators
            if os::consts::FAMILY == os::consts::windows::FAMILY {
                assert_eq!(glob_vec("aaa\\apple"), ~[abs_path("aaa/apple")]);
            }

            assert_eq!(glob_vec("???/"), ~[
                abs_path("aaa"),
                abs_path("bbb"),
                abs_path("ccc"),
                abs_path("xyz")]);

            assert_eq!(glob_vec("aaa/tomato/tom?to.txt"), ~[
                abs_path("aaa/tomato/tomato.txt"),
                abs_path("aaa/tomato/tomoto.txt")]);

            assert_eq!(glob_vec("xyz/?"), ~[
                abs_path("xyz/x"),
                abs_path("xyz/y"),
                abs_path("xyz/z")]);

            assert_eq!(glob_vec("a*"), ~[abs_path("aaa")]);
            assert_eq!(glob_vec("*a*"), ~[abs_path("aaa")]);
            assert_eq!(glob_vec("a*a"), ~[abs_path("aaa")]);
            assert_eq!(glob_vec("aaa*"), ~[abs_path("aaa")]);
            assert_eq!(glob_vec("*aaa"), ~[abs_path("aaa")]);
            assert_eq!(glob_vec("*aaa*"), ~[abs_path("aaa")]);
            assert_eq!(glob_vec("*a*a*a*"), ~[abs_path("aaa")]);
            assert_eq!(glob_vec("aaa*/"), ~[abs_path("aaa")]);

            assert_eq!(glob_vec("aaa/*"), ~[
                abs_path("aaa/apple"),
                abs_path("aaa/orange"),
                abs_path("aaa/tomato")]);

            assert_eq!(glob_vec("aaa/*a*"), ~[
                abs_path("aaa/apple"),
                abs_path("aaa/orange"),
                abs_path("aaa/tomato")]);

            assert_eq!(glob_vec("*/*/*.txt"), ~[
                abs_path("aaa/tomato/tomato.txt"),
                abs_path("aaa/tomato/tomoto.txt")]);

            assert_eq!(glob_vec("*/*/t[aob]m?to[.]t[!y]t"), ~[
                abs_path("aaa/tomato/tomato.txt"),
                abs_path("aaa/tomato/tomoto.txt")]);

            assert_eq!(glob_vec("aa[a]"), ~[abs_path("aaa")]);
            assert_eq!(glob_vec("aa[abc]"), ~[abs_path("aaa")]);
            assert_eq!(glob_vec("a[bca]a"), ~[abs_path("aaa")]);
            assert_eq!(glob_vec("aa[b]"), ~[]);
            assert_eq!(glob_vec("aa[xyz]"), ~[]);
            assert_eq!(glob_vec("aa[]]"), ~[]);

            assert_eq!(glob_vec("aa[!b]"), ~[abs_path("aaa")]);
            assert_eq!(glob_vec("aa[!bcd]"), ~[abs_path("aaa")]);
            assert_eq!(glob_vec("a[!bcd]a"), ~[abs_path("aaa")]);
            assert_eq!(glob_vec("aa[!a]"), ~[]);
            assert_eq!(glob_vec("aa[!abc]"), ~[]);

            assert_eq!(glob_vec("bbb/specials/[[]"), ~[abs_path("bbb/specials/[")]);
            assert_eq!(glob_vec("bbb/specials/!"), ~[abs_path("bbb/specials/!")]);
            assert_eq!(glob_vec("bbb/specials/[]]"), ~[abs_path("bbb/specials/]")]);

            if os::consts::FAMILY != os::consts::windows::FAMILY {
                assert_eq!(glob_vec("bbb/specials/[*]"), ~[abs_path("bbb/specials/*")]);
                assert_eq!(glob_vec("bbb/specials/[?]"), ~[abs_path("bbb/specials/?")]);
            }

            if os::consts::FAMILY == os::consts::windows::FAMILY {

                assert_eq!(glob_vec("bbb/specials/[![]"), ~[
                    abs_path("bbb/specials/!"),
                    abs_path("bbb/specials/]")]);

                assert_eq!(glob_vec("bbb/specials/[!]]"), ~[
                    abs_path("bbb/specials/!"),
                    abs_path("bbb/specials/[")]);

                assert_eq!(glob_vec("bbb/specials/[!!]"), ~[
                    abs_path("bbb/specials/["),
                    abs_path("bbb/specials/]")]);

            } else {

                assert_eq!(glob_vec("bbb/specials/[![]"), ~[
                    abs_path("bbb/specials/!"),
                    abs_path("bbb/specials/*"),
                    abs_path("bbb/specials/?"),
                    abs_path("bbb/specials/]")]);

                assert_eq!(glob_vec("bbb/specials/[!]]"), ~[
                    abs_path("bbb/specials/!"),
                    abs_path("bbb/specials/*"),
                    abs_path("bbb/specials/?"),
                    abs_path("bbb/specials/[")]);

                assert_eq!(glob_vec("bbb/specials/[!!]"), ~[
                    abs_path("bbb/specials/*"),
                    abs_path("bbb/specials/?"),
                    abs_path("bbb/specials/["),
                    abs_path("bbb/specials/]")]);

                assert_eq!(glob_vec("bbb/specials/[!*]"), ~[
                    abs_path("bbb/specials/!"),
                    abs_path("bbb/specials/?"),
                    abs_path("bbb/specials/["),
                    abs_path("bbb/specials/]")]);

                assert_eq!(glob_vec("bbb/specials/[!?]"), ~[
                    abs_path("bbb/specials/!"),
                    abs_path("bbb/specials/*"),
                    abs_path("bbb/specials/["),
                    abs_path("bbb/specials/]")]);

            }
        };
    }

    #[test]
    fn test_absolute_pattern() {
        // assume that the filesystem is not empty!
        assert!(glob("/*").next().is_some());
        assert!(glob("//").next().is_none());

        // check windows absolute paths with host/device components
        let root_with_device = (Path {components: ~[], .. os::getcwd()}).to_str() + "*";
        assert!(glob(root_with_device).next().is_some());
    }

    #[test]
    fn test_lots_of_files() {
        // this is a good test because it touches lots of differently named files
        for glob("/*/*/*/*").advance |_p| {}
    }

    #[test]
    #[should_fail]
    #[cfg(not(windows))]
    fn test_unclosed_bracket() {
        Pattern::new("abc[def");
    }

    #[test]
    #[should_fail]
    #[cfg(not(windows))]
    fn test_unclosed_bracket_special() {
        Pattern::new("abc[]"); // not valid syntax, '[]]' should be used to match ']'
    }

    #[test]
    #[should_fail]
    #[cfg(not(windows))]
    fn test_unclosed_bracket_special_except() {
        Pattern::new("abc[!]"); // not valid syntax, '[!]]' should be used to match NOT ']'
    }

    #[test]
    fn test_pattern_matches() {
        let txt_pat = Pattern::new("*hello.txt");
        assert!(txt_pat.matches("hello.txt"));
        assert!(txt_pat.matches("gareth_says_hello.txt"));
        assert!(txt_pat.matches("some/path/to/hello.txt"));
        assert!(txt_pat.matches("some\\path\\to\\hello.txt"));
        assert!(txt_pat.matches("/an/absolute/path/to/hello.txt"));
        assert!(!txt_pat.matches("hello.txt-and-then-some"));
        assert!(!txt_pat.matches("goodbye.txt"));

        let dir_pat = Pattern::new("*some/path/to/hello.txt");
        assert!(dir_pat.matches("some/path/to/hello.txt"));
        assert!(dir_pat.matches("a/bigger/some/path/to/hello.txt"));
        assert!(!dir_pat.matches("some/path/to/hello.txt-and-then-some"));
        assert!(!dir_pat.matches("some/other/path/to/hello.txt"));
    }

    #[test]
    fn test_pattern_escape() {
        let s = "_[_]_?_*_!_";
        assert_eq!(Pattern::escape(s), ~"_[[]_[]]_[?]_[*]_!_");
        assert!(Pattern::new(Pattern::escape(s)).matches(s));
    }

    #[test]
    fn test_pattern_matches_case_insensitive() {

        let pat = Pattern::new("aBcDeFg");
        let options = MatchOptions {
            case_sensitive: false,
            require_literal_separator: false
        };

        assert!(pat.matches_with("aBcDeFg", options));
        assert!(pat.matches_with("abcdefg", options));
        assert!(pat.matches_with("ABCDEFG", options));
        assert!(pat.matches_with("AbCdEfG", options));
    }

    #[test]
    fn test_pattern_matches_case_insensitive_range() {

        let pat_within = Pattern::new("[a]");
        let pat_except = Pattern::new("[!a]");

        let options_case_insensitive = MatchOptions {
            case_sensitive: false,
            require_literal_separator: false
        };
        let options_case_sensitive = MatchOptions {
            case_sensitive: true,
            require_literal_separator: false
        };

        assert!(pat_within.matches_with("a", options_case_insensitive));
        assert!(pat_within.matches_with("A", options_case_insensitive));
        assert!(!pat_within.matches_with("A", options_case_sensitive));

        assert!(!pat_except.matches_with("a", options_case_insensitive));
        assert!(!pat_except.matches_with("A", options_case_insensitive));
        assert!(pat_except.matches_with("A", options_case_sensitive));
    }

    #[test]
    fn test_pattern_matches_require_literal_separator() {

        let options_requires_literal = MatchOptions {
            case_sensitive: true,
            require_literal_separator: true
        };
        let options_not_requires_literal = MatchOptions {
            case_sensitive: true,
            require_literal_separator: false
        };

        assert!(Pattern::new("abc/def").matches_with("abc/def", options_requires_literal));
        assert!(!Pattern::new("abc?def").matches_with("abc/def", options_requires_literal));
        assert!(!Pattern::new("abc*def").matches_with("abc/def", options_requires_literal));
        assert!(!Pattern::new("abc[/]def").matches_with("abc/def", options_requires_literal));

        assert!(Pattern::new("abc/def").matches_with("abc/def", options_not_requires_literal));
        assert!(Pattern::new("abc?def").matches_with("abc/def", options_not_requires_literal));
        assert!(Pattern::new("abc*def").matches_with("abc/def", options_not_requires_literal));
        assert!(Pattern::new("abc[/]def").matches_with("abc/def", options_not_requires_literal));
    }
}

