#![deny(automatic_links)]

/// [http://a.com](http://a.com)
//~^ ERROR unneeded long form for URL
/// [http://b.com]
//~^ ERROR unneeded long form for URL
///
/// [http://b.com]: http://b.com
///
/// [http://c.com][http://c.com]
pub fn a() {}

/// https://somewhere.com
//~^ ERROR this URL is not a hyperlink
/// https://somewhere.com/a
//~^ ERROR this URL is not a hyperlink
/// https://www.somewhere.com
//~^ ERROR this URL is not a hyperlink
/// https://www.somewhere.com/a
//~^ ERROR this URL is not a hyperlink
/// https://subdomain.example.com
//~^ ERROR not a hyperlink
/// https://somewhere.com?
//~^ ERROR this URL is not a hyperlink
/// https://somewhere.com/a?
//~^ ERROR this URL is not a hyperlink
/// https://somewhere.com?hello=12
//~^ ERROR this URL is not a hyperlink
/// https://somewhere.com/a?hello=12
//~^ ERROR this URL is not a hyperlink
/// https://example.com?hello=12#xyz
//~^ ERROR this URL is not a hyperlink
/// https://example.com/a?hello=12#xyz
//~^ ERROR this URL is not a hyperlink
/// https://example.com#xyz
//~^ ERROR this URL is not a hyperlink
/// https://example.com/a#xyz
//~^ ERROR this URL is not a hyperlink
/// https://somewhere.com?hello=12&bye=11
//~^ ERROR this URL is not a hyperlink
/// https://somewhere.com/a?hello=12&bye=11
//~^ ERROR this URL is not a hyperlink
/// https://somewhere.com?hello=12&bye=11#xyz
//~^ ERROR this URL is not a hyperlink
/// hey! https://somewhere.com/a?hello=12&bye=11#xyz
//~^ ERROR this URL is not a hyperlink
pub fn c() {}

/// <https://somewhere.com>
/// [a](http://a.com)
/// [b]
///
/// [b]: http://b.com
pub fn everything_is_fine_here() {}

#[allow(automatic_links)]
pub mod foo {
    /// https://somewhere.com/a?hello=12&bye=11#xyz
    pub fn bar() {}
}
