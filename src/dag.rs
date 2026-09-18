//! DAG — a task dependency graph with Kahn topological sort (harvested from
//! forge-task-graph). Returns None on a cycle. One home; lifted from
//! forge-book-v3 dag.rs 2026-09-02 so dead-drop-v3 stops linking the codex.

/// A dependency DAG over `n` integer-id nodes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Dag {
    /// Number of nodes in the DAG.
    pub n: usize,
    /// Edges as (before, after): `before` must precede `after`.
    pub edges: Vec<(usize, usize)>,
}

impl Dag {
    /// Create a new DAG with `n` nodes and no edges.
    pub fn new(n: usize) -> Self {
        Self { n, edges: Vec::new() }
    }

    /// Declare that `node` depends on `on` (so `on` comes first).
    pub fn depend(&mut self, node: usize, on: usize) -> &mut Self {
        if node < self.n && on < self.n {
            self.edges.push((on, node));
        }
        self
    }

    /// Kahn topological order, or None if a cycle exists.
    pub fn topo(&self) -> Option<Vec<usize>> {
        let mut indeg = vec![0usize; self.n];
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); self.n];
        for &(before, after) in &self.edges {
            adj[before].push(after);
            indeg[after] += 1;
        }
        let mut ready: Vec<usize> = (0..self.n).filter(|&i| indeg[i] == 0).collect();
        ready.sort_unstable_by(|a, b| b.cmp(a)); // pop smallest first (stable-ish)
        let mut order = Vec::with_capacity(self.n);
        while let Some(u) = ready.pop() {
            order.push(u);
            for &v in &adj[u] {
                indeg[v] -= 1;
                if indeg[v] == 0 {
                    ready.push(v);
                    ready.sort_unstable_by(|a, b| b.cmp(a));
                }
            }
        }
        (order.len() == self.n).then_some(order)
    }

    /// Does a valid ordering exist (no cycle)?
    pub fn is_acyclic(&self) -> bool {
        self.topo().is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn topo_respects_dependencies() {
        // 0 <- 1 <- 2 (2 depends on 1 depends on 0)
        let mut d = Dag::new(3);
        d.depend(1, 0).depend(2, 1);
        let order = d.topo().unwrap();
        let pos = |x| order.iter().position(|&v| v == x).unwrap();
        assert!(pos(0) < pos(1));
        assert!(pos(1) < pos(2));
    }

    #[test]
    fn cycle_has_no_order() {
        let mut d = Dag::new(2);
        d.depend(1, 0).depend(0, 1);
        assert!(d.topo().is_none());
        assert!(!d.is_acyclic());
    }

    #[test]
    fn independent_nodes_all_appear() {
        let d = Dag::new(4);
        assert_eq!(d.topo().unwrap().len(), 4);
    }
}
