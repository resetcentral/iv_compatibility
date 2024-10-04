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