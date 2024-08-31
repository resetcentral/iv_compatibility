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

        println!("{:#?}", infusion_map);

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

        println!("{:#?}", infusion_map);

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
    }
}

pub mod solver {
    use crate::infusion::Infusion;
    use std::collections::{HashSet, HashMap};
    use petgraph::graph::UnGraph;
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
        graph: UnGraph<(), ()>
    }

    impl CompatibilityProblem {
        pub fn new(num_ivs: u32, ivs: Vec<HashSet<u32>>, infusions: HashMap<u32, Infusion>) -> Self {
            let edges = infusions
                .values()
                .map(|inf| {
                    inf.get_compatible().map(
                        |other_id| {
                            (inf.get_id(), *other_id)
                        }
                    )
                })
                .flatten();
            let graph = UnGraph::from_edges(edges);

            Self {
                num_ivs,
                ivs,
                infusions,
                graph
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
