//! Sorts items by type

use doc::ItemUtils;

export mk_pass;

fn mk_pass() -> Pass {
    pure fn by_score(item1: &doc::ItemTag, item2: &doc::ItemTag) -> bool {
        pure fn score(item: &doc::ItemTag) -> int {
            match *item {
              doc::ConstTag(_) => 0,
              doc::TyTag(_) => 1,
              doc::EnumTag(_) => 2,
              doc::TraitTag(_) => 3,
              doc::ImplTag(_) => 4,
              doc::FnTag(_) => 5,
              doc::ModTag(_) => 6,
              doc::NmodTag(_) => 7
            }
        }

        score(item1) <= score(item2)
    }

    sort_pass::mk_pass(~"sort_item_type", by_score)
}

#[test]
fn test() {
    let source =
        ~"mod imod { } \
         extern mod inmod { } \
         const iconst: int = 0; \
         fn ifn() { } \
         enum ienum { ivar } \
         trait itrait { fn a(); } \
         impl int { fn a() { } } \
         type itype = int;";
    do astsrv::from_str(source) |srv| {
        let doc = extract::from_srv(srv, ~"");
        let doc = mk_pass().f(srv, doc);
        assert doc.cratemod().items[0].name() == ~"iconst";
        assert doc.cratemod().items[1].name() == ~"itype";
        assert doc.cratemod().items[2].name() == ~"ienum";
        assert doc.cratemod().items[3].name() == ~"itrait";
        assert doc.cratemod().items[4].name() == ~"__extensions__";
        assert doc.cratemod().items[5].name() == ~"ifn";
        assert doc.cratemod().items[6].name() == ~"imod";
        assert doc.cratemod().items[7].name() == ~"inmod";
    }
}
