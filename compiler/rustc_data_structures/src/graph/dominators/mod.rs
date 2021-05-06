//! Finding the dominators in a control-flow graph.

use super::iterate::reverse_post_order;
use super::ControlFlowGraph;
use rustc_index::vec::{Idx, IndexVec};
use std::cmp::Ordering;

#[cfg(test)]
mod tests;

pub fn dominators<G: ControlFlowGraph>(graph: G) -> Dominators<G::Node> {
    let start_node = graph.start_node();
    let rpo = reverse_post_order(&graph, start_node);
    dominators_given_rpo(graph, &rpo)
}

fn dominators_given_rpo<G: ControlFlowGraph>(graph: G, rpo: &[G::Node]) -> Dominators<G::Node> {
    let start_node = graph.start_node();
    assert_eq!(rpo[0], start_node);

    // compute the post order index (rank) for each node
    let mut post_order_rank = IndexVec::from_elem_n(0, graph.num_nodes());
    for (index, node) in rpo.iter().rev().cloned().enumerate() {
        post_order_rank[node] = index;
    }

    // We take a slightly interesting approach to storing the dominators. Rather
    // than storing in the "actual" index of the graph node, we store in the
    // post order rank index. This means that when we need to find the closest
    // dominator to a given node after computing the dominator sets, we can
    // simply find the 2nd set bit (the first is the node itself).
    let mut dominators =
        IndexVec::from_elem_n(BitSet::new_filled(graph.num_nodes().index()), graph.num_nodes());

    // The start node should have just the start node filled.
    dominators[graph.start_node()].clear();
    dominators[graph.start_node()].insert(post_order_rank[graph.start_node()]);

    let mut changed = true;
    let mut temp = BitSet::new_empty(graph.num_nodes());
    while changed {
        changed = false;

        for &node in &rpo[1..] {
            let mut preds = graph.predecessors(node);
            // There must be a predecessor, as this is a non-root node in a RPO
            // traversal.
            let first = preds.next().unwrap();
            if let Some(pred) = preds.next() {
                // there are two predecessors, so intersect the first two into
                // temp.
                temp.intersect_dest(&dominators[first], &dominators[pred]);
                while let Some(pred) = preds.next() {
                    temp.intersect_no_change(&dominators[pred]);
                }
                temp.insert(post_order_rank[node]);
                if temp != dominators[node] {
                    std::mem::swap(&mut dominators[node], &mut temp);
                    changed = true;
                }
            } else {
                let pred = first;
                if node != pred {
                    let (doms_node, doms_pred) = dominators.pick2_mut(node, pred);
                    // There is only one predecessor.
                    doms_node.remove(post_order_rank[node]);
                    if doms_node != doms_pred {
                        doms_node.clone_from(&doms_pred);
                        changed = true;
                    }
                    doms_node.insert(post_order_rank[node]);
                }
            }
        }
    }

    // At this point for each node we have the full set of dominators (up to the
    // root). We want to find the immediate dominators.

    let mut immediate_dominators = IndexVec::from_elem_n(None, graph.num_nodes());
    immediate_dominators[start_node] = Some(start_node);

    for &node in &rpo[1..] {
        // Our 'dominators' set contains the nodes that dominate this one (as
        // computed above). The immediate dominator is the one closest to us.
        //
        // As noted above, the dominators are indexed by the postorder rank of
        // each node, so we actually know that the idom is the 2nd bit set
        // (where the first bit set is this node, as we dominate ourselves).

        let post_order_idx = dominators[node].next_after(post_order_rank[node]).unwrap();
        immediate_dominators[node] = Some(rpo[rpo.len() - 1 - post_order_idx]);
    }

    Dominators { post_order_rank, immediate_dominators }
}

#[derive(Clone, Debug)]
pub struct Dominators<N: Idx> {
    post_order_rank: IndexVec<N, usize>,
    immediate_dominators: IndexVec<N, Option<N>>,
}

impl<Node: Idx> Dominators<Node> {
    pub fn dummy() -> Self {
        Self { post_order_rank: IndexVec::new(), immediate_dominators: IndexVec::new() }
    }

    pub fn is_reachable(&self, node: Node) -> bool {
        self.immediate_dominators[node].is_some()
    }

    pub fn immediate_dominator(&self, node: Node) -> Node {
        assert!(self.is_reachable(node), "node {:?} is not reachable", node);
        self.immediate_dominators[node].unwrap()
    }

    pub fn dominators(&self, node: Node) -> Iter<'_, Node> {
        assert!(self.is_reachable(node), "node {:?} is not reachable", node);
        Iter { dominators: self, node: Some(node) }
    }

    pub fn is_dominated_by(&self, node: Node, dom: Node) -> bool {
        // FIXME -- could be optimized by using post-order-rank
        self.dominators(node).any(|n| n == dom)
    }

    /// Provide deterministic ordering of nodes such that, if any two nodes have a dominator
    /// relationship, the dominator will always precede the dominated. (The relative ordering
    /// of two unrelated nodes will also be consistent, but otherwise the order has no
    /// meaning.) This method cannot be used to determine if either Node dominates the other.
    pub fn rank_partial_cmp(&self, lhs: Node, rhs: Node) -> Option<Ordering> {
        self.post_order_rank[lhs].partial_cmp(&self.post_order_rank[rhs])
    }
}

pub struct Iter<'dom, Node: Idx> {
    dominators: &'dom Dominators<Node>,
    node: Option<Node>,
}

impl<'dom, Node: Idx> Iterator for Iter<'dom, Node> {
    type Item = Node;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(node) = self.node {
            let dom = self.dominators.immediate_dominator(node);
            if dom == node {
                self.node = None; // reached the root
            } else {
                self.node = Some(dom);
            }
            Some(node)
        } else {
            None
        }
    }
}
