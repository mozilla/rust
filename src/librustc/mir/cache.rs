use rustc_data_structures::indexed_vec::IndexVec;
use rustc_data_structures::stable_hasher::{HashStable, StableHasher, StableHasherResult};
use rustc_serialize::{Encodable, Encoder, Decodable, Decoder};
use crate::ich::StableHashingContext;
use crate::mir::{Body, BasicBlock};

#[derive(Clone, Debug)]
pub struct Cache {
    predecessors: Option<IndexVec<BasicBlock, Vec<BasicBlock>>>
}


impl rustc_serialize::Encodable for Cache {
    fn encode<S: Encoder>(&self, s: &mut S) -> Result<(), S::Error> {
        Encodable::encode(&(), s)
    }
}

impl rustc_serialize::Decodable for Cache {
    fn decode<D: Decoder>(d: &mut D) -> Result<Self, D::Error> {
        Decodable::decode(d).map(|_v: ()| Self::new())
    }
}

impl<'a> HashStable<StableHashingContext<'a>> for Cache {
    fn hash_stable<W: StableHasherResult>(&self,
                                          _: &mut StableHashingContext<'a>,
                                          _: &mut StableHasher<W>) {
        // Do nothing.
    }
}

impl Cache {
    pub fn new() -> Self {
        Cache {
            predecessors: None
        }
    }

    pub fn invalidate(&mut self) {
        // FIXME: consider being more fine-grained
        self.predecessors = None;
    }

    pub fn predecessors_ref(&self) -> &IndexVec<BasicBlock, Vec<BasicBlock>> {
        assert!(self.predecessors.is_some());
        self.predecessors.as_ref().unwrap()
    }

    pub fn predecessors_mut(
        &mut self,
        body: &Body<'_>
    ) -> &mut IndexVec<BasicBlock, Vec<BasicBlock>> {
        if self.predecessors.is_none() {
            self.predecessors = Some(calculate_predecessors(body));
        }

        self.predecessors.as_mut().unwrap()
    }
}

fn calculate_predecessors(body: &Body<'_>) -> IndexVec<BasicBlock, Vec<BasicBlock>> {
    let mut result = IndexVec::from_elem(vec![], body.basic_blocks());
    for (bb, data) in body.basic_blocks().iter_enumerated() {
        if let Some(ref term) = data.terminator {
            for &tgt in term.successors() {
                result[tgt].push(bb);
            }
        }
    }

    result
}

CloneTypeFoldableAndLiftImpls! {
    Cache,
}
