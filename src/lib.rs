pub mod db {
    use mysql::{Pool, PooledConn};
    use mysql::prelude::*;
    use configparser::ini::Ini;
    use std::collections::HashMap;
    use std::rc::Rc;
    use crate::infusion::{CompatibilityData, Infusion, InfusionType};

    pub fn connect_db(config_path: &str) -> Pool {
        let mut config = Ini::new();
        config.load(config_path).expect("Failed to load DB config!");
        let host = config.get("db", "host").expect("host not in db.conf!");
        let db_name = config.get("db", "db_name").expect("db_name not in db.conf!");
        let user = config.get("db", "user").expect("user not in db.conf!");
        let password = config.get("db", "password").expect("password not in db.conf!");
        let url = format!("mysql://{}:{}@{}/{}", user, password, host, db_name);
    
        let pool = Pool::new(url.as_str()).expect("Unable to parse DB url! Fix your code!!!");
    
        pool
    }

    pub fn load_infusions(conn: &mut PooledConn, ids: Vec<&u32>) -> HashMap<u32, Infusion> {
        let mut infusion_map = HashMap::new();

        // load basic infusion info
        // no risk of SQL injection since we know all values are u32
        let ids_param = ids.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(",");
        let results = conn
            .query(format!("SELECT id, name, type FROM infusion WHERE id IN ({ids_param})"))
            .expect("Failed loading infusion data from DB");

        for (id, name, inf_type_id) in results {
            let inf_type = match inf_type_id {
                1 => InfusionType::Drug,
                2 => InfusionType::Solution,
                _ => panic!("Unknown infusion type")
            };
            let infusion = Infusion::new(id, name, inf_type);
            infusion_map.insert(infusion.get_id(), infusion);
        }

        // load infusion compatibility info
        // no risk of SQL injection since we know all values are u32
        let ids_param = infusion_map.keys().map(|i| { i.to_string() }).collect::<Vec<_>>().join(",");
        let results = conn
            .query(
                format!(
                    "SELECT infusion_a, infusion_b, compatible_results, incompatible_results, mixed_results
                    FROM infusion_compatibility
                    WHERE infusion_a IN ({ids_param}) AND infusion_b in ({ids_param})"
                )
            ).expect("Failed loading compatibility data from DB");

        for (id1, id2, compatible, incompatible, mixed) in results {
            let compat_data = Rc::new(CompatibilityData::new(compatible, incompatible, mixed));
            
            let infusion1 = infusion_map.get_mut(&id1).unwrap();
            infusion1.add_compatibility_data(id2, &compat_data);

            let infusion2 = infusion_map.get_mut(&id2).unwrap();
            infusion2.add_compatibility_data(id1, &compat_data);
        }

        infusion_map
    }
}

pub mod infusion {
    use std::collections::HashMap;
    use std::rc::Rc;

    #[derive(Debug)]
    pub enum InfusionType {
        Drug,
        Solution
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]            // let infusion1: &'a mut Infusion = infusions.get_mut(&id1).unwrap();
    pub enum Compatibility {
        Compatible,
        Incompatible,
    }

    #[derive(Debug)]
    pub struct CompatibilityData {
        compatible: u32,
        incompatible: u32,
        mixed: u32,
        compatibility: Compatibility,
    }

    impl CompatibilityData {
        pub fn new(compatible: u32, incompatible: u32, mixed: u32) -> CompatibilityData {
            let compatibility = if compatible > 0 && incompatible == 0 && mixed == 0 {
                Compatibility::Compatible
            } else {
                Compatibility::Incompatible
            };

            Self {
                compatible,
                incompatible,
                mixed,
                compatibility,
            }
        }
    }

    #[derive(Debug)]
    pub struct Infusion {
        id: u32,
        name: String,
        infusion_type: InfusionType,
        compatibility: HashMap<u32, Rc<CompatibilityData>>, // Infusion.id -> CompatibilityData
    }

    impl Infusion {
        pub fn new(id: u32, name: String, infusion_type: InfusionType) -> Self {
            Self {
                id,
                name,
                infusion_type,
                compatibility: HashMap::new(),
            }
        }

        pub fn get_id(&self) -> u32 {
            self.id
        }

        pub fn add_compatibility_data(&mut self, other_id: u32, compat_data: &Rc<CompatibilityData>) {
            self.compatibility.insert(other_id, Rc::clone(compat_data));
        }

        pub fn get_compatible(&self) -> impl Iterator<Item = &u32> {
            self.compatibility.keys().filter(
                |id| {
                    let compat = self.compatibility.get(id).unwrap();
                    compat.compatibility == Compatibility::Compatible
                }
            )
        }

        

        pub fn get_incompatible(&self, all_ids: &Vec<&u32>) -> Vec<u32> {
            let compat: Vec<_> = self.get_compatible().map(|i| { i.clone() }).collect();
            let mut incompat = Vec::new();

            for id in all_ids {
                let id = *id;
                if !compat.contains(id) {
                    incompat.push(id.clone());
                }
            }
            
            incompat
        }
    }
    use std::ops::Range;
}

pub mod solver {
    use crate::infusion::Infusion;
    use std::{collections::{HashMap, HashSet}, hash::Hash};
    use itertools::Itertools;
    use petgraph::prelude::*;
    use std::{error, fmt};

    #[derive(Debug)]
    struct UnsolvableError;

    impl fmt::Display for UnsolvableError {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "Unable to solve")
        }
    }

    impl error::Error for UnsolvableError {}


    #[derive(Debug)]
    pub struct CompatibilityProblem {
        num_ivs: u32,
        ivs: Vec<HashSet<u32>>,
        infusions: HashMap<u32, Infusion>,
        graph: UnGraphMap<u32, ()>
    }

    impl CompatibilityProblem {
        pub fn new(num_ivs: u32, ivs: Vec<HashSet<u32>>, infusions: HashMap<u32, Infusion>) -> Self {
            let all_ids = infusions.keys().collect();
            println!("{:?}", all_ids);
            let edges = infusions
                .values()
                .map(|inf| {
                    inf.get_incompatible(&all_ids).into_iter().map(
                        |other_id| {
                            (inf.get_id(), other_id)
                        }
                    ).collect::<Vec<(u32, u32)>>()
                })
                .flatten();

            let graph = UnGraphMap::from_edges(edges);

            Self {
                num_ivs,
                ivs,
                infusions,
                graph
            }
        }

        fn sort_nodes(nodes: &mut Vec<u32>, possible_colors: &HashMap<u32, HashSet<u32>>, adjacent_uncolored: &HashMap<u32, usize>) {
            nodes.sort_unstable_by_key(
                |n| {
                    let node_np = possible_colors.get(n).unwrap().len();
                    let node_au = adjacent_uncolored.get(n).unwrap();
                    (node_np,
                    *node_au)
                }
            )
        }

        fn select_color(node_colors: &HashSet<u32>, color_usage: &HashMap<u32, Vec<u32>>, color_possibilities: &HashMap<u32, usize>, num_nodes_left: usize) -> u32 {
            *node_colors.iter().next().unwrap()
        }

        pub fn solve(&self) {
            let mut possible_colors = HashMap::new();
            let mut adjacent_uncolored = HashMap::new();
            let mut color_usage = HashMap::new();
            let mut color_possibilities = HashMap::new();
            let mut colors_count = 0;

            let mut nodes_left: Vec<u32> = self.graph.nodes().collect();
            for node in &nodes_left {
                possible_colors.insert(*node, HashSet::new());
                adjacent_uncolored.insert(*node, self.graph.neighbors(*node).collect::<Vec<_>>().len());
            }

            while nodes_left.len() > 0 {
                Self::sort_nodes(&mut nodes_left, &possible_colors, &adjacent_uncolored);
                println!("{:?}", nodes_left);

                let node = nodes_left.pop().unwrap();
                let node_colors = possible_colors.get(&node).unwrap();

                println!("Picked Node: {:?}", node);
                println!("Colors: {:?}", node_colors);

                let adjacent_nodes = self.graph.neighbors(node).collect_vec();

                // Pick a color
                let color;
                // let mut color = colors_count;
                if node_colors.is_empty() {
                    color = colors_count;
                    color_usage.insert(color, vec![node]);
                    colors_count += 1;

                    // Update non-adjacent nodes with the possibility of the new color
                    let non_adjacent_uncolored = nodes_left.iter().filter(|n| { !adjacent_nodes.contains(n)}).collect_vec();
                    color_possibilities.insert(color, non_adjacent_uncolored.len());
                    for non_adj_node in non_adjacent_uncolored {
                        possible_colors.get_mut(non_adj_node).unwrap().insert(color);
                    }
                } else {
                    color = Self::select_color(node_colors ,&color_usage, &color_possibilities, nodes_left.len());
                    color_usage.get_mut(&color).unwrap().push(node);
                    // Remove possibility of this color from adjacent nodes
                    for adj_node in &adjacent_nodes {
                        if let Some(color_set) = possible_colors.get_mut(adj_node) {
                            if color_set.remove(&color) {
                                *color_possibilities.get_mut(&color).unwrap() -= 1;
                            }
                        }
                    }
                }

                // Update metrics for adjacent nodes
                for adj_node in adjacent_nodes {
                    *adjacent_uncolored.get_mut(&adj_node).unwrap() -= 1;
                }
                println!("{:#?}", color_usage);
            }
            
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn todo() {
        assert_eq!(3, 4);
    }
}
