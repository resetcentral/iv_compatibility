use crate::infusion::Infusion;
use std::collections::{HashMap, HashSet};
use itertools::Itertools;
use petgraph::prelude::*;
use std::{error, fmt};

#[derive(Debug)]
pub struct ConflictError {
    pub iv: u32,
    pub conflicting_items: (String, String)
}

impl fmt::Display for ConflictError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "There are incompatible infusions in IV #{}", self.iv + 1)
    }
}

impl error::Error for ConflictError {}

/// An IV drug compatibility problem is reduced to a graph coloring problem
/// where each IV infusion is represented by a node and _incompatible_
/// infusions are connected by edges. Nodes of the same color are infusions
/// that go in the same IV.
#[derive(Debug)]
pub struct CompatibilityProblem {
    infusions: HashMap<u32, Infusion>,
    graph: UnGraphMap<u32, ()>,
    uncolored_nodes: Vec<u32>,
    possible_colors: HashMap<u32, HashSet<u32>>,    // node -> set of possible colors
    adjacent_uncolored: HashMap<u32, u32>,          // node -> number of uncolored adjacent nodes
    color_usage: HashMap<u32, Vec<u32>>,            // color -> list of nodes with that color
    color_max_count: HashMap<u32, u32>,             // color -> max number of nodes which _could_ use that color
    colors: Vec<u32>,
}

impl CompatibilityProblem {
    pub fn new(infusions: HashMap<u32, Infusion>) -> Self {
        // Build the graph representation
        let all_ids = infusions.keys().collect();
        let edges = infusions
            .values()
            .map(|inf| {
                inf.get_incompatible(&all_ids).into_iter().map(
                    |other_id| {
                        (inf.id(), other_id)
                    }
                ).collect::<Vec<(u32, u32)>>()
            })
            .flatten();
        let graph = UnGraphMap::from_edges(edges);

        // Initialize color tracking data
        let mut possible_colors = HashMap::new();
        let mut adjacent_uncolored = HashMap::new();
        let uncolored_nodes = graph.nodes().collect_vec();
        for node in uncolored_nodes.iter() {
            possible_colors.insert(*node, HashSet::new());

            let num_neighbors = graph.neighbors(*node).collect::<Vec<_>>().len();
            adjacent_uncolored.insert(*node, num_neighbors as u32);
        }

        Self {
            infusions,
            graph,
            uncolored_nodes,
            possible_colors,
            adjacent_uncolored,
            color_usage: HashMap::new(),
            color_max_count: HashMap::new(),
            colors: Vec::new(),
        }
    }

    /// Sort nodes by number of possible colors descending,
    /// then by number of adjacent uncolored nodes ascending.
    /// 
    /// The most preferred node is at the end of the list for easy .pop() access.
    fn sort_nodes(&mut self) {
        self.uncolored_nodes.sort_unstable_by_key(
            |n| {
                let node_np = self.possible_colors.get(n).unwrap().len();
                let node_au = self.adjacent_uncolored.get(n).unwrap();
                (-(node_np as i32), *node_au)
            }
        )
    }

    fn select_color(&self, node_colors: &HashSet<u32>) -> u32 {
        // Map colors to the max possible number of nodes that could be that color
        let color_potential = node_colors
            .into_iter()
            .map(|color| {
                let num_possible = *self.color_max_count.get(color).unwrap();
                (color, num_possible)
            });

        // Sort and return the color with the maximum potential
        *color_potential.sorted_by_key(|c| { c.1 })
            .collect_vec()
            .pop().unwrap()
            .0
    }

    /// Add a new color to the graph
    fn add_new_color(&mut self) -> u32 {
        let color = self.colors.len() as u32;
        self.colors.push(color);

        self.color_usage.insert(color, Vec::new());
        self.color_max_count.insert(color, self.uncolored_nodes.len() as u32);

        for node in &self.uncolored_nodes {
            self.possible_colors.get_mut(node).unwrap().insert(color);
        }

        color
    }

    fn color_node(&mut self, node: u32, color: u32) -> Result<(), ConflictError> {
        let adjacent_nodes = self.graph.neighbors(node).collect_vec();
        // Check that node is allowed to be this color
        for adj_node in &adjacent_nodes {
            if self.color_usage.get(&color).unwrap().contains(adj_node) {
                let name1 = self.infusions.get(&node).unwrap().name().to_string();
                let name2 = self.infusions.get(adj_node).unwrap().name().to_string();
                return Err(ConflictError { iv: color, conflicting_items: (name1, name2) });
            }
        }

        self.color_usage.get_mut(&color).unwrap().push(node);
        
        // Update color counts
        for other_color in self.possible_colors.get(&node).unwrap() {
            if color != *other_color {
                *self.color_max_count.get_mut(other_color).unwrap() -= 1;
            }
        }

        // Remove possibility of this color from adjacent nodes
        for adj_node in &adjacent_nodes {
            let color_set = self.possible_colors.get_mut(adj_node).unwrap();
            if color_set.remove(&color) {
                *self.color_max_count.get_mut(&color).unwrap() -= 1;
            }
        }

        // Update metrics for adjacent nodes
        for adj_node in &adjacent_nodes {
            *self.adjacent_uncolored.get_mut(adj_node).unwrap() -= 1;
        }

        Ok(())
    }

    fn init_coloring(&mut self, ivs: Vec<HashSet<u32>>) -> Result<(), ConflictError> {
        let preset_nodes = ivs.iter().flatten().unique().collect_vec();
        self.uncolored_nodes.retain(|n| { !preset_nodes.contains(&n) });

        for iv_infusions in &ivs {
            let color = self.add_new_color();
            for node in iv_infusions {
                self.color_node(*node, color)?;
                *self.color_max_count.get_mut(&color).unwrap() += 1;
            }
        }

        Ok(())
    }

    pub fn solve(&mut self, ivs: Vec<HashSet<u32>>) -> Result<HashMap<u32, Vec<&Infusion>>, ConflictError> {
        self.init_coloring(ivs)?;

        while self.uncolored_nodes.len() > 0 {
            self.sort_nodes();
            println!("{:?}", self.uncolored_nodes);

            let node = self.uncolored_nodes.pop().unwrap();
            let node_colors = self.possible_colors.get(&node).unwrap();

            // Pick a color
            let color = if node_colors.is_empty() { self.add_new_color() } else { self.select_color(node_colors) };

            self.color_node(node, color)?;

            println!("{:?}", self.color_usage);    
        }

        // Convert infusion IDs to names
        let mut output = HashMap::new();
        for (iv, inf_id_list) in self.color_usage.iter() {
            let iv_infusions = inf_id_list
                .into_iter()
                .map(|inf_id| {
                    self.infusions.get(&inf_id).unwrap()
                })
                .collect_vec();
            output.insert(*iv, iv_infusions);
        }

        Ok(output)
    }
}