use super::super::test::TestGraph;

use super::*;

#[test]
fn diamond() {
    let graph = TestGraph::new(0, &[(0, 1), (0, 2), (1, 3), (2, 3)]);

    let dominators = dominators(&graph);
    let immediate_dominators = dominators.all_immediate_dominators();
    assert_eq!(immediate_dominators[0], Some(0));
    assert_eq!(immediate_dominators[1], Some(0));
    assert_eq!(immediate_dominators[2], Some(0));
    assert_eq!(immediate_dominators[3], Some(0));
}

#[test]
fn paper() {
    // example from the paper:
    let graph = TestGraph::new(6,
                               &[(6, 5), (6, 4), (5, 1), (4, 2), (4, 3), (1, 2), (2, 3), (3, 2),
                                 (2, 1)]);

    let dominators = dominators(&graph);
    let immediate_dominators = dominators.all_immediate_dominators();
    assert_eq!(immediate_dominators[0], None); // <-- note that 0 is not in graph
    assert_eq!(immediate_dominators[1], Some(6));
    assert_eq!(immediate_dominators[2], Some(6));
    assert_eq!(immediate_dominators[3], Some(6));
    assert_eq!(immediate_dominators[4], Some(6));
    assert_eq!(immediate_dominators[5], Some(6));
    assert_eq!(immediate_dominators[6], Some(6));
}
