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
            infusion_map.insert(infusion.id(), infusion);
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

        pub fn id(&self) -> u32 {
            self.id
        }

        pub fn name(&self) -> &str {
            &self.name
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
}

pub mod solver {
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
            let all_ids = infusions.keys().collect();
            println!("{:?}", all_ids);
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
            let uncolored_nodes = graph.nodes().collect_vec();
            let mut possible_colors = HashMap::new();
            let mut adjacent_uncolored = HashMap::new();
            for node in uncolored_nodes.iter() {
                possible_colors.insert(*node, HashSet::new());
                adjacent_uncolored.insert(*node, graph.neighbors(*node).collect::<Vec<_>>().len() as u32);
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
            *node_colors
                .into_iter()
                .map(|color| {
                    //let num_used = self.color_usage.get(color).unwrap().len();
                    let num_possible = *self.color_max_count.get(color).unwrap();
                    (color, num_possible)
                })
                .sorted_by_key(|c| { c.1 })
                .collect_vec()
                .pop().unwrap()
                .0
        }

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
            for iv_infusions in &ivs {
                let color = self.add_new_color();
                
                for node in iv_infusions {
                    self.color_node(*node, color)?;
                }
            }
            let preset_nodes = ivs.into_iter().flatten().unique().collect_vec();
            self.uncolored_nodes.retain(|n| { !preset_nodes.contains(n) });

            Ok(())
        }

        pub fn solve(&mut self, ivs: Vec<HashSet<u32>>) -> Result<HashMap<u32, Vec<&Infusion>>, ConflictError> {
            self.init_coloring(ivs)?;

            while self.uncolored_nodes.len() > 0 {
                self.sort_nodes();
                println!("{:?}", self.uncolored_nodes);

                let node = self.uncolored_nodes.pop().unwrap();
                let node_colors = self.possible_colors.get(&node).unwrap();

                println!("Picked Node: {:?}", node);
                println!("Possible Colors: {:?}", node_colors);

                // Pick a color
                let color = if node_colors.is_empty() { self.add_new_color() } else { self.select_color(node_colors) };

                self.color_node(node, color)?;

                println!("{:#?}", self.color_usage);    
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn todo() {
        assert_eq!(3, 4);
    }
}
