//! Structural analysis over the observed contextual control-flow graph.
//!
//! Results preserve [`ContextualPoint`] identity. They describe the graph recovered by abstract
//! interpretation and carry explicit uncertainty whenever missing edges or collapsed contexts may
//! make a structural conclusion incomplete.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use petgraph::{
    algo::{dominators::simple_fast, kosaraju_scc},
    graph::{DiGraph, NodeIndex},
    Direction,
};

use super::context::{ContextualCfg, ContextualPoint};

/// Why structural results may be incomplete.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ControlFlowUncertainty {
    /// A jump may have successors not represented in the graph.
    UnresolvedJump,
    /// Analysis stopped before all queued points were executed.
    AnalysisBudgetExhausted,
    /// Distinct calling contexts were merged by the local context budget.
    ContextCollapsed,
    /// An older continuation context was discarded by the depth bound.
    ContextTruncated,
    /// At least one observed point cannot reach a modeled exit.
    NoReachableExit,
}

/// A strongly connected component in deterministic contextual-point order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StronglyConnectedComponent {
    /// Contextual points in this component.
    pub members: BTreeSet<ContextualPoint>,
    /// Members with at least one predecessor outside this component.
    pub entries: BTreeSet<ContextualPoint>,
    /// Whether the component has multiple external entries and is therefore irreducible.
    pub irreducible: bool,
}

/// A natural loop induced by one or more back edges to a shared header.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NaturalLoop {
    /// Loop header dominating every latch.
    pub header: ContextualPoint,
    /// Sources of back edges targeting `header`.
    pub latches: BTreeSet<ContextualPoint>,
    /// Complete observed natural-loop body, including the header and latches.
    pub members: BTreeSet<ContextualPoint>,
}

/// Dominance, post-dominance, SCC, and natural-loop analysis of a contextual CFG.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ContextualControlFlow {
    /// Immediate dominator for each root-reachable point. Roots map to `None`.
    pub immediate_dominators: BTreeMap<ContextualPoint, Option<ContextualPoint>>,
    /// Immediate post-dominator for points able to reach a modeled exit.
    pub immediate_post_dominators: BTreeMap<ContextualPoint, Option<ContextualPoint>>,
    /// Strongly connected components, including singleton components.
    pub sccs: Vec<StronglyConnectedComponent>,
    /// Reducible natural loops merged by header.
    pub natural_loops: Vec<NaturalLoop>,
    /// Reasons these observed-graph results may be incomplete.
    pub uncertainty: BTreeSet<ControlFlowUncertainty>,
}

impl ContextualControlFlow {
    /// Whether `dominator` dominates `point` in the observed root-reachable graph.
    pub fn dominates(&self, dominator: &ContextualPoint, point: &ContextualPoint) -> bool {
        dominates_in(&self.immediate_dominators, dominator, point)
    }

    /// Whether `post_dominator` post-dominates `point` on paths to modeled exits.
    pub fn post_dominates(
        &self,
        post_dominator: &ContextualPoint,
        point: &ContextualPoint,
    ) -> bool {
        dominates_in(&self.immediate_post_dominators, post_dominator, point)
    }

    /// Whether all structural results are exact for the recovered graph and analysis bounds.
    pub fn is_exact(&self) -> bool {
        self.uncertainty.is_empty()
    }
}

/// Analyze structural properties of the contextual graph without flattening calling contexts.
pub fn analyze_contextual_control_flow(cfg: &ContextualCfg) -> ContextualControlFlow {
    let points = all_points(cfg);
    if points.is_empty() {
        return ContextualControlFlow::default()
    }

    let mut graph = DiGraph::<(), ()>::new();
    let nodes =
        points.iter().map(|point| (point.clone(), graph.add_node(()))).collect::<BTreeMap<_, _>>();
    for edge in &cfg.edges {
        if let (Some(&source), Some(&target)) = (nodes.get(&edge.source), nodes.get(&edge.target)) {
            if graph.find_edge(source, target).is_none() {
                graph.add_edge(source, target, ());
            }
        }
    }

    let immediate_dominators = dominators(cfg, &points, &nodes, &mut graph);
    let immediate_post_dominators = post_dominators(cfg, &points, &nodes, &graph);
    let sccs = components(&points, &nodes, &graph);
    let natural_loops = loops(&points, &nodes, &graph, &immediate_dominators, &sccs);
    let uncertainty = uncertainty(cfg, &points, &immediate_post_dominators);

    ContextualControlFlow {
        immediate_dominators,
        immediate_post_dominators,
        sccs,
        natural_loops,
        uncertainty,
    }
}

fn all_points(cfg: &ContextualCfg) -> Vec<ContextualPoint> {
    let mut points = BTreeSet::new();
    points.extend(cfg.initial_states.keys().cloned());
    points.extend(cfg.entry_states.keys().cloned());
    points.extend(cfg.exit_states.keys().cloned());
    for edge in &cfg.edges {
        points.insert(edge.source.clone());
        points.insert(edge.target.clone());
    }
    points.into_iter().collect()
}

fn dominators(
    cfg: &ContextualCfg,
    points: &[ContextualPoint],
    nodes: &BTreeMap<ContextualPoint, NodeIndex>,
    graph: &mut DiGraph<(), ()>,
) -> BTreeMap<ContextualPoint, Option<ContextualPoint>> {
    let root = graph.add_node(());
    for point in cfg.initial_states.keys() {
        if let Some(&node) = nodes.get(point) {
            graph.add_edge(root, node, ());
        }
    }
    let result = simple_fast(&*graph, root);
    points
        .iter()
        .filter_map(|point| {
            let node = nodes[point];
            result.dominators(node)?;
            let parent = result
                .immediate_dominator(node)
                .filter(|&parent| parent != root)
                .and_then(|parent| points.get(parent.index()).cloned());
            Some((point.clone(), parent))
        })
        .collect()
}

fn post_dominators(
    cfg: &ContextualCfg,
    points: &[ContextualPoint],
    nodes: &BTreeMap<ContextualPoint, NodeIndex>,
    graph: &DiGraph<(), ()>,
) -> BTreeMap<ContextualPoint, Option<ContextualPoint>> {
    let mut reverse = DiGraph::<(), ()>::new();
    for _ in points {
        reverse.add_node(());
    }
    for edge in graph.edge_indices() {
        if let Some((source, target)) = graph.edge_endpoints(edge) {
            if source.index() < points.len() && target.index() < points.len() {
                reverse.add_edge(target, source, ());
            }
        }
    }
    let exit = reverse.add_node(());
    for point in points {
        let node = nodes[point];
        let has_successor = graph
            .neighbors_directed(node, Direction::Outgoing)
            .any(|target| target.index() < points.len());
        let incomplete_sink =
            cfg.unresolved_jumps.contains(point) || cfg.budget_exhausted_points.contains(point);
        if !has_successor && !incomplete_sink {
            reverse.add_edge(exit, node, ());
        }
    }
    let result = simple_fast(&reverse, exit);
    points
        .iter()
        .filter_map(|point| {
            let node = nodes[point];
            result.dominators(node)?;
            let parent = result
                .immediate_dominator(node)
                .filter(|&parent| parent != exit)
                .and_then(|parent| points.get(parent.index()).cloned());
            Some((point.clone(), parent))
        })
        .collect()
}

fn components(
    points: &[ContextualPoint],
    nodes: &BTreeMap<ContextualPoint, NodeIndex>,
    graph: &DiGraph<(), ()>,
) -> Vec<StronglyConnectedComponent> {
    let mut components = kosaraju_scc(graph)
        .into_iter()
        .filter_map(|component| {
            let members = component
                .into_iter()
                .filter_map(|node| points.get(node.index()).cloned())
                .collect::<BTreeSet<_>>();
            if members.is_empty() {
                return None
            }
            let entries = members
                .iter()
                .filter(|point| {
                    graph
                        .neighbors_directed(nodes[*point], Direction::Incoming)
                        .filter_map(|node| points.get(node.index()))
                        .any(|predecessor| !members.contains(predecessor))
                })
                .cloned()
                .collect::<BTreeSet<_>>();
            Some(StronglyConnectedComponent { irreducible: entries.len() > 1, members, entries })
        })
        .collect::<Vec<_>>();
    components.sort_by(|left, right| left.members.first().cmp(&right.members.first()));
    components
}

fn loops(
    points: &[ContextualPoint],
    nodes: &BTreeMap<ContextualPoint, NodeIndex>,
    graph: &DiGraph<(), ()>,
    immediate_dominators: &BTreeMap<ContextualPoint, Option<ContextualPoint>>,
    sccs: &[StronglyConnectedComponent],
) -> Vec<NaturalLoop> {
    let irreducible = sccs
        .iter()
        .filter(|scc| scc.irreducible)
        .flat_map(|scc| &scc.members)
        .collect::<BTreeSet<_>>();
    let mut by_header = BTreeMap::<ContextualPoint, NaturalLoop>::new();
    for source in points {
        for target_node in graph.neighbors_directed(nodes[source], Direction::Outgoing) {
            let Some(target) = points.get(target_node.index()) else { continue };
            if irreducible.contains(target) || !dominates_in(immediate_dominators, target, source) {
                continue
            }
            let mut members = BTreeSet::from([target.clone(), source.clone()]);
            let mut queue = VecDeque::from([source.clone()]);
            while let Some(member) = queue.pop_front() {
                if member == *target {
                    continue
                }
                for predecessor_node in
                    graph.neighbors_directed(nodes[&member], Direction::Incoming)
                {
                    let Some(predecessor) = points.get(predecessor_node.index()) else { continue };
                    if dominates_in(immediate_dominators, target, predecessor) &&
                        members.insert(predecessor.clone())
                    {
                        queue.push_back(predecessor.clone());
                    }
                }
            }
            let natural_loop = by_header.entry(target.clone()).or_insert_with(|| NaturalLoop {
                header: target.clone(),
                latches: BTreeSet::new(),
                members: BTreeSet::new(),
            });
            natural_loop.latches.insert(source.clone());
            natural_loop.members.extend(members);
        }
    }
    by_header.into_values().collect()
}

fn uncertainty(
    cfg: &ContextualCfg,
    points: &[ContextualPoint],
    post_dominators: &BTreeMap<ContextualPoint, Option<ContextualPoint>>,
) -> BTreeSet<ControlFlowUncertainty> {
    let mut result = BTreeSet::new();
    if !cfg.unresolved_jumps.is_empty() {
        result.insert(ControlFlowUncertainty::UnresolvedJump);
    }
    if !cfg.budget_exhausted_points.is_empty() {
        result.insert(ControlFlowUncertainty::AnalysisBudgetExhausted);
    }
    if !cfg.collapsed_contexts.is_empty() {
        result.insert(ControlFlowUncertainty::ContextCollapsed);
    }
    if points.iter().any(|point| point.context.is_truncated()) {
        result.insert(ControlFlowUncertainty::ContextTruncated);
    }
    if points.iter().any(|point| !post_dominators.contains_key(point)) {
        result.insert(ControlFlowUncertainty::NoReachableExit);
    }
    result
}

fn dominates_in(
    immediate: &BTreeMap<ContextualPoint, Option<ContextualPoint>>,
    dominator: &ContextualPoint,
    point: &ContextualPoint,
) -> bool {
    let mut current = Some(point);
    let mut remaining = immediate.len().saturating_add(1);
    while let Some(candidate) = current {
        if candidate == dominator {
            return true
        }
        if remaining == 0 {
            return false
        }
        remaining -= 1;
        current = immediate.get(candidate).and_then(Option::as_ref);
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{
        analysis::AbstractState,
        context::{AnalysisContext, ContextualEdge, ContextualEdgeKind},
        hardfork::HardFork,
        opcodes,
        program::Program,
    };

    fn cfg(node_count: usize, edges: &[(usize, usize)]) -> (ContextualCfg, Vec<ContextualPoint>) {
        let mut bytecode = Vec::new();
        for _ in 0..node_count {
            bytecode.extend([opcodes::JUMPDEST, opcodes::STOP]);
        }
        let program = Program::decode(&bytecode, HardFork::Latest);
        let root = program.blocks[0].id;
        let points = program
            .blocks
            .iter()
            .take(node_count)
            .map(|block| ContextualPoint { block: block.id, context: AnalysisContext::new(root) })
            .collect::<Vec<_>>();
        let mut cfg = ContextualCfg::default();
        cfg.initial_states.insert(points[0].clone(), AbstractState::default());
        for point in &points {
            cfg.entry_states.insert(point.clone(), AbstractState::default());
        }
        for &(source, target) in edges {
            cfg.edges.insert(ContextualEdge {
                source: points[source].clone(),
                target: points[target].clone(),
                kind: ContextualEdgeKind::Jump,
            });
        }
        (cfg, points)
    }

    #[test]
    fn computes_dominators_and_post_dominators_for_a_diamond() {
        let (cfg, points) = cfg(4, &[(0, 1), (0, 2), (1, 3), (2, 3)]);
        let flow = analyze_contextual_control_flow(&cfg);

        assert_eq!(flow.immediate_dominators[&points[0]], None);
        assert_eq!(flow.immediate_dominators[&points[1]], Some(points[0].clone()));
        assert_eq!(flow.immediate_dominators[&points[2]], Some(points[0].clone()));
        assert_eq!(flow.immediate_dominators[&points[3]], Some(points[0].clone()));
        assert_eq!(flow.immediate_post_dominators[&points[0]], Some(points[3].clone()));
        assert_eq!(flow.immediate_post_dominators[&points[1]], Some(points[3].clone()));
        assert!(flow.dominates(&points[0], &points[3]));
        assert!(flow.post_dominates(&points[3], &points[0]));
        assert!(flow.is_exact());
    }

    #[test]
    fn recovers_a_natural_loop_and_its_scc() {
        let (cfg, points) = cfg(4, &[(0, 1), (1, 2), (2, 1), (1, 3)]);
        let flow = analyze_contextual_control_flow(&cfg);

        assert_eq!(flow.natural_loops.len(), 1);
        assert_eq!(flow.natural_loops[0].header, points[1]);
        assert_eq!(flow.natural_loops[0].latches, BTreeSet::from([points[2].clone()]));
        assert_eq!(
            flow.natural_loops[0].members,
            BTreeSet::from([points[1].clone(), points[2].clone()])
        );
        assert!(flow.sccs.iter().any(|scc| {
            scc.members == BTreeSet::from([points[1].clone(), points[2].clone()]) &&
                !scc.irreducible
        }));
    }

    #[test]
    fn marks_a_multi_entry_cycle_irreducible() {
        let (cfg, points) = cfg(4, &[(0, 1), (0, 2), (1, 2), (2, 1), (2, 3)]);
        let flow = analyze_contextual_control_flow(&cfg);
        let cycle = flow.sccs.iter().find(|scc| scc.members.len() == 2).unwrap();

        assert_eq!(cycle.entries, BTreeSet::from([points[1].clone(), points[2].clone()]));
        assert!(cycle.irreducible);
        assert!(flow.natural_loops.is_empty());
    }

    #[test]
    fn does_not_invent_an_exit_for_unresolved_nontermination() {
        let (mut cfg, points) = cfg(2, &[(0, 1), (1, 1)]);
        cfg.unresolved_jumps.insert(points[1].clone());
        let flow = analyze_contextual_control_flow(&cfg);

        assert!(flow.immediate_post_dominators.is_empty());
        assert!(flow.uncertainty.contains(&ControlFlowUncertainty::UnresolvedJump));
        assert!(flow.uncertainty.contains(&ControlFlowUncertainty::NoReachableExit));
    }

    #[test]
    fn synthetic_boundaries_handle_multiple_roots_and_exits() {
        let (mut multiple_roots, points) = cfg(4, &[(0, 2), (1, 2), (2, 3)]);
        multiple_roots.initial_states.insert(points[1].clone(), AbstractState::default());
        let flow = analyze_contextual_control_flow(&multiple_roots);

        assert_eq!(flow.immediate_dominators[&points[0]], None);
        assert_eq!(flow.immediate_dominators[&points[1]], None);
        assert_eq!(flow.immediate_dominators[&points[2]], None);
        assert_eq!(flow.immediate_post_dominators[&points[0]], Some(points[2].clone()));

        let (two_exits, points) = cfg(3, &[(0, 1), (0, 2)]);
        let flow = analyze_contextual_control_flow(&two_exits);
        assert_eq!(flow.immediate_post_dominators[&points[0]], None);
        assert_eq!(flow.immediate_post_dominators[&points[1]], None);
        assert_eq!(flow.immediate_post_dominators[&points[2]], None);
    }

    #[test]
    fn recognizes_a_self_loop() {
        let (cfg, points) = cfg(2, &[(0, 1), (1, 1)]);
        let flow = analyze_contextual_control_flow(&cfg);

        assert_eq!(flow.natural_loops.len(), 1);
        assert_eq!(flow.natural_loops[0].header, points[1]);
        assert_eq!(flow.natural_loops[0].latches, BTreeSet::from([points[1].clone()]));
        assert_eq!(flow.natural_loops[0].members, BTreeSet::from([points[1].clone()]));
        assert!(flow.uncertainty.contains(&ControlFlowUncertainty::NoReachableExit));
    }

    #[test]
    fn output_is_independent_of_edge_insertion_order() {
        let edges = [(0, 1), (0, 2), (1, 3), (2, 3)];
        let (cfg_forward, _) = cfg(4, &edges);
        let (cfg_reverse, _) = cfg(4, &edges.into_iter().rev().collect::<Vec<_>>());

        assert_eq!(
            analyze_contextual_control_flow(&cfg_forward),
            analyze_contextual_control_flow(&cfg_reverse)
        );
    }
}
