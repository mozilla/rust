// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#![feature(box_syntax)]

struct HTMLImageData {
    image: Option<String>
}

struct ElementData {
    kind: Box<ElementKind>
}

enum ElementKind {
    HTMLImageElement(HTMLImageData)
}

enum NodeKind {
    Element(ElementData)
}

struct NodeData {
    kind: Box<NodeKind>,
}

fn main() {
    let mut id = HTMLImageData { image: None };
    let ed = ElementData { kind: box ElementKind::HTMLImageElement(id) };
    let n = NodeData {kind : box NodeKind::Element(ed)};
    // n.b. span could be better
    match n.kind {
        box NodeKind::Element(ed) => match ed.kind { //~ ERROR non-exhaustive patterns
            box ElementKind::HTMLImageElement(ref d) if d.image.is_some() => { true }
        },
        _ => panic!("WAT") //~ ERROR unreachable pattern
    };
}
