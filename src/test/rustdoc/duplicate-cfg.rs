// ignore-tidy-linelength

#![crate_name = "foo"]
#![feature(doc_cfg)]

// @has 'foo/index.html'
// @matches '-' '//*[@class="module-item"]//*[@class="stab portability"]' '^sync$'
// @has '-' '//*[@class="module-item"]//*[@class="stab portability"]/@title' 'This is supported on crate feature `sync` only'

// @has 'foo/struct.Foo.html'
// @has '-' '//*[@class="stab portability"]' 'sync'
#[doc(cfg(feature = "sync"))]
#[doc(cfg(feature = "sync"))]
pub struct Foo;

// @has 'foo/bar/index.html'
// @matches '-' '//*[@class="module-item"]//*[@class="stab portability"]' '^sync$'
// @has '-' '//*[@class="module-item"]//*[@class="stab portability"]/@title' 'This is supported on crate feature `sync` only'

// @has 'foo/bar/struct.Bar.html'
// @has '-' '//*[@class="stab portability"]' 'This is supported on crate feature sync only.'
#[doc(cfg(feature = "sync"))]
pub mod bar {
    #[doc(cfg(feature = "sync"))]
    pub struct Bar;
}

// @has 'foo/baz/index.html'
// @matches '-' '//*[@class="module-item"]//*[@class="stab portability"]' '^sync and send$'
// @has '-' '//*[@class="module-item"]//*[@class="stab portability"]/@title' 'This is supported on crate features `sync` and `send` only'

// @has 'foo/baz/struct.Baz.html'
// @has '-' '//*[@class="stab portability"]' 'This is supported on crate features sync and send only.'
#[doc(cfg(all(feature = "sync", feature = "send")))]
pub mod baz {
    #[doc(cfg(feature = "sync"))]
    pub struct Baz;
}

// @has 'foo/qux/struct.Qux.html'
// @has '-' '//*[@class="stab portability"]' 'This is supported on crate features sync and send only.'
#[doc(cfg(feature = "sync"))]
pub mod qux {
    #[doc(cfg(all(feature = "sync", feature = "send")))]
    pub struct Qux;
}

// @has 'foo/quux/index.html'
// @matches '-' '//*[@class="module-item"]//*[@class="stab portability"]' '^sync and send and foo and bar$'
// @has '-' '//*[@class="module-item"]//*[@class="stab portability"]/@title' 'This is supported on crate feature `sync` and crate feature `send` and `foo` and `bar` only'

// @has 'foo/quux/struct.Quux.html'
// @has '-' '//*[@class="stab portability"]' 'This is supported on crate feature sync and crate feature send and foo and bar only.'
#[doc(cfg(all(feature = "sync", feature = "send", foo)))]
pub mod quux {
    #[doc(cfg(all(feature = "send", feature = "sync", bar)))]
    pub struct Quux;
}
