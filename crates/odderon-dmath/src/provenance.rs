//! Pillar 1 Enforcement: Provenance<T> & zero-heap lineage DAG for uncertainty propagation.
//! No display-facing gauge in Odderon shows a raw number without an EpistemicSource and 1-sigma bar.

use crate::trig::dsqrt;

pub type ProvId = u8;
pub const MAX_PROV_NODES: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum EpistemicSource {
    MeasuredSensor = 0,
    AstronomicalCatalog = 1,
    DerivedConservation = 2,
    EmpiricalModel = 3,
    AssumedByPlayer = 4,
}

#[derive(Clone, Copy, Debug)]
pub struct ProvEdge {
    pub parent: ProvId,
    /// Partial derivative df / dx_parent at the current operating point.
    pub jacobian: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct ProvNode {
    pub value: f64,
    pub sigma: f64,
    pub source: EpistemicSource,
    pub has_player_assumption: bool,
    pub edge_count: u8,
    pub edges: [ProvEdge; 4],
}

impl Default for ProvNode {
    fn default() -> Self {
        Self {
            value: 0.0,
            sigma: 0.0,
            source: EpistemicSource::DerivedConservation,
            has_player_assumption: false,
            edge_count: 0,
            edges: [ProvEdge {
                parent: 0,
                jacobian: 0.0,
            }; 4],
        }
    }
}

pub struct ProvGraph {
    pub nodes: [ProvNode; MAX_PROV_NODES],
    pub len: usize,
}

impl Default for ProvGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl ProvGraph {
    pub const fn new() -> Self {
        let empty = ProvNode {
            value: 0.0,
            sigma: 0.0,
            source: EpistemicSource::DerivedConservation,
            has_player_assumption: false,
            edge_count: 0,
            edges: [ProvEdge {
                parent: 0,
                jacobian: 0.0,
            }; 4],
        };
        Self {
            nodes: [empty; MAX_PROV_NODES],
            len: 0,
        }
    }

    pub fn clear(&mut self) {
        self.len = 0;
    }

    pub fn push_leaf(&mut self, value: f64, sigma: f64, source: EpistemicSource) -> ProvId {
        let idx = self.len;
        self.nodes[idx] = ProvNode {
            value,
            sigma,
            source,
            has_player_assumption: source == EpistemicSource::AssumedByPlayer,
            edge_count: 0,
            edges: [ProvEdge {
                parent: 0,
                jacobian: 0.0,
            }; 4],
        };
        self.len += 1;
        idx as ProvId
    }

    /// Pushes a derived node and automatically propagates sigma = sqrt(sum((df/dx_i * sigma_i)^2) + model_sigma^2).
    pub fn push_derived(
        &mut self,
        value: f64,
        model_sigma: f64,
        source: EpistemicSource,
        deps: &[(ProvId, f64)],
    ) -> ProvId {
        let mut var = model_sigma * model_sigma;
        let mut assumed = source == EpistemicSource::AssumedByPlayer;
        let mut edges = [ProvEdge {
            parent: 0,
            jacobian: 0.0,
        }; 4];
        let count = if deps.len() > 4 { 4 } else { deps.len() };
        let mut i = 0;
        while i < count {
            let (pid, jac) = deps[i];
            let pnode = self.nodes[pid as usize];
            let term = jac * pnode.sigma;
            var += term * term;
            if pnode.has_player_assumption {
                assumed = true;
            }
            edges[i] = ProvEdge {
                parent: pid,
                jacobian: jac,
            };
            i += 1;
        }
        let idx = self.len;
        self.nodes[idx] = ProvNode {
            value,
            sigma: dsqrt(var),
            source,
            has_player_assumption: assumed,
            edge_count: count as u8,
            edges,
        };
        self.len += 1;
        idx as ProvId
    }
}
