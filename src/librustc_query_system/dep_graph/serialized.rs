//! The data that we will serialize and deserialize.

use super::{DepKind, DepNode, DepNodeIndex};
use rustc_data_structures::fingerprint::Fingerprint;
use rustc_index::vec::IndexVec;

/// Data for use when recompiling the **current crate**.
#[derive(Debug, RustcEncodable, RustcDecodable)]
pub struct SerializedDepGraph<K: DepKind> {
    /// The set of all DepNodes in the graph
    pub nodes: IndexVec<DepNodeIndex, DepNode<K>>,
    /// The set of all Fingerprints in the graph. Each Fingerprint corresponds to
    /// the DepNode at the same index in the nodes vector.
    pub fingerprints: IndexVec<DepNodeIndex, Fingerprint>,
    /// For each DepNode, stores the list of edges originating from that
    /// DepNode. Encoded as a [start, end) pair indexing into edge_list_data,
    /// which holds the actual DepNodeIndices of the target nodes.
    pub edge_list_indices: IndexVec<DepNodeIndex, (u32, u32)>,
    /// A flattened list of all edge targets in the graph. Edge sources are
    /// implicit in edge_list_indices.
    pub edge_list_data: Vec<DepNodeIndex>,
}

impl<K: DepKind> Default for SerializedDepGraph<K> {
    fn default() -> Self {
        SerializedDepGraph {
            nodes: Default::default(),
            fingerprints: Default::default(),
            edge_list_indices: Default::default(),
            edge_list_data: Default::default(),
        }
    }
}

impl<K: DepKind> SerializedDepGraph<K> {
    #[inline]
    pub fn edge_targets_from(&self, source: DepNodeIndex) -> &[DepNodeIndex] {
        let targets = self.edge_list_indices[source];
        &self.edge_list_data[targets.0 as usize..targets.1 as usize]
    }
}
