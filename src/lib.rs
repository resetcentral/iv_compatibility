use std::{error, fmt};

#[derive(Debug)]
struct UnsolvableError;

impl fmt::Display for UnsolvableError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Unable to solve")
    }
}

impl error::Error for UnsolvableError {}

pub mod db {
    use mysql::{Pool, PooledConn};
    use serde::{Deserialize, Serialize};
    use mysql::prelude::*;
    use configparser::ini::Ini;
    use std::collections::HashMap;
    use crate::infusions::{Infusion, InfusionType};

    pub fn connect_db() -> Pool {
        let mut config = Ini::new();
        config.load("./db.conf").expect("Failed to load DB config!");
        let host = config.get("db", "host").expect("host not in db.conf!");
        let db_name = config.get("db", "db_name").expect("db_name not in db.conf!");
        let user = config.get("db", "user").expect("user not in db.conf!");
        let password = config.get("db", "password").expect("password not in db.conf!");
        let url = format!("mysql://{}:{}@{}/{}", user, password, host, db_name);
    
        let pool = Pool::new(url.as_str()).expect("Unable to parse DB url! Fix your code!!!");
    
        pool
    }

    pub fn get_infusions_by_id<'a>(conn: &mut PooledConn, ids: Vec<&u32>) -> HashMap<u32, Infusion<'a>> {
        let results: Vec<Infusion> = conn
            .exec_map(
                "SELECT id, name, type WHERE id IN ?",
                ids,
            |(id, name, inf_type)| {
                let inf_type = match inf_type {
                    1 => InfusionType::Drug,
                    2 => InfusionType::Solution,
                    _ => panic!("Unknown infusion type")
                };
                Infusion::new(id, name, inf_type)
            })
            .expect("Failed loading infusion data from DB!");

        let mut infusions = HashMap::new();
        for infusion in results {
            infusions.insert(infusion.get_id(), infusion);
        }

        println!("{:#?}", infusions);

        infusions
    }
}

pub mod infusions {
    use serde::{Deserialize, Serialize};
    use std::collections::{ HashMap, HashSet };
    use std::hash::{Hash, Hasher};

    #[derive(Debug)]
    pub enum InfusionType {
        Drug,
        Solution
    }

    #[derive(Clone, Copy, Debug)]
    enum Compatibility {
        Compatible,
        Incompatible,
    }

    #[derive(Debug)]
    struct CompatibilityData {
        compatible: u32,
        incompatible: u32,
        unknown: u32,
        compatibility: Compatibility,
    }

    impl CompatibilityData {
        fn new(compatible: u32, incompatible: u32, unknown: u32) -> CompatibilityData {
            let compatibility = if compatible > 0 && incompatible == 0 && unknown == 0 {
                Compatibility::Compatible
            } else {
                Compatibility::Incompatible
            };

            Self {
                compatible,
                incompatible,
                unknown,
                compatibility,
            }
        }
    }

    #[derive(Debug)]
    pub struct Infusion<'a> {
        id: u32,
        name: String,
        infusion_type: InfusionType,
        compatibility: HashMap<u32, CompatibilityData>, // Infusion.id -> CompatibilityData
        compatible: HashSet<&'a Infusion<'a>>,
        incompatible: HashSet<&'a Infusion<'a>>,
    }

    impl<'a> PartialEq for Infusion<'a> {
        fn eq(&self, other: &Self) -> bool {
            self.id == other.id
        }
    }

    impl<'a> Eq for Infusion<'a> {}

    impl<'a> Hash for Infusion<'a> {
        fn hash<H: Hasher>(&self, state: &mut H) {
            self.id.hash(state);
        }
    }

    impl<'a> Infusion<'a> {
        pub fn new(id: u32, name: String, infusion_type: InfusionType) -> Self {
            Self {
                id,
                name,
                infusion_type,
                compatibility: HashMap::new(),
                compatible: HashSet::new(),
                incompatible: HashSet::new(),
            }
        }

        pub fn get_id(&self) -> u32 {
            self.id
        }

        fn add_compatibility_data(&mut self, id1: u32, id2: u32, compat_data: CompatibilityData) {
            let other_id = if id1 == self.id { id2 } else { id1 };

            self.compatibility.insert(other_id, compat_data);
        }

        fn compute_compatibility(&mut self, infusions: HashSet<&'a Infusion<'a>>) {
            for infusion in infusions {
                let compat = if infusion.id == self.id {
                    Compatibility::Compatible
                } else {
                    match self.compatibility.get(&infusion.id) {
                        None => Compatibility::Incompatible,
                        Some(c) => c.compatibility,
                    }
                };

                match compat {
                    Compatibility::Compatible => {
                        self.compatible.insert(infusion);
                    },
                    Compatibility::Incompatible => {
                        self.incompatible.insert(infusion);
                    }
                }
            }
        }

        // fn compatible_with(&self, drug: &Infusion) -> &CompatibilityData {
        //     self.compatibility.get(&drug.id).unwrap_or(&CompatibilityData::new(0,0,0))
        // }
    }

    pub struct Iv<'a> {
        infusions: HashSet<&'a Infusion<'a>>,
    }

    impl<'a> Iv<'a> {
        pub fn new() -> Self {
            Self {
                infusions: HashSet::<&Infusion>::new(),
            }
        }

        pub fn add_infusion(&mut self, new_infusion: &'a Infusion) {
            // Would this be faster to check in the other direction?
            for infusion in &self.infusions {
                if !infusion.compatible.contains(new_infusion) {
                    panic!("Attempted to add incompatible infusion to IV. Fix your code!");
                }
            }

            self.infusions.insert(new_infusion);
        }
    }
}






// fn solve_compatibility_even<'a>(ivs: Vec<Infusion>, infusions: Vec<Infusion>)
//     -> Result<Vec<Infusion<'a>>, UnsolvableError> {
    
//     return Ok(vec![]);
// }



#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn todo() {
        assert_eq!(3, 4);
    }
}
