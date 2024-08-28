use std::fs::File;
use std::io::{self, BufRead};
use std::collections::HashMap;
use configparser::ini::Ini;
use mysql::*;
use mysql::prelude::*;

#[derive(Debug)]
struct InfusionInput<'a> {
    name: String,
    inf_type: u32,
    compat: HashMap<&'a str, Vec<u32>>
}

fn setup_type_table(conn: &mut PooledConn) {
    let existing: Vec<u32> = conn.query("SELECT id FROM infusion_type").unwrap();
    if existing.len() == 2 {
        return
    }
    println!("Inserting values into infusion_type table");
    conn.query_drop("DELETE FROM infusion_type").expect("Table creation failed on clearing old table values");
    conn.query_drop("INSERT INTO infusion_type (id, type) VALUES 
        (1, 'Drug'),
        (2, 'Solution')").expect("Table creation failed on insertion!");
}

fn get_infusion_id_by_name(conn: &mut PooledConn, name: &str) -> Option<u32> {
    conn.exec_first("SELECT id FROM infusion WHERE name=?", (name,)).expect("DB Query failed!")
}

fn import_infusions(conn: &mut PooledConn, data: Vec<InfusionInput>) {
    let mut name_id_map = HashMap::new();

    for infusion in &data {
        conn.exec_drop("INSERT IGNORE INTO infusion (name, type) VALUES (?, ?)",
                    (infusion.name.as_str(), infusion.inf_type))
                    .expect("DB insert failed!");

        let id = get_infusion_id_by_name(conn, infusion.name.as_str()).unwrap();

        name_id_map.insert(infusion.name.as_str(), id);
    }

    for infusion in &data {
        let id = *name_id_map.get(infusion.name.as_str()).expect("Invalid compatibility data, infusion not found!");
        let ic = &infusion.compat;
        for (other_name, compat_values) in ic.into_iter() {
            let other_id = *name_id_map.get(other_name).expect("Invalid compatibility data, other infusion not found!");

            let params = if id < other_id {
                (id, other_id, compat_values[0], compat_values[1], compat_values[2])
            } else {
                (other_id, id, compat_values[0], compat_values[1], compat_values[2])
            };

            conn.exec_drop("INSERT IGNORE INTO infusion_compatibility
                (infusion_a, infusion_b, compatible_results, incompatible_results, mixed_results)
                VALUES (?, ?, ?, ?, ?)",
                params)
                .expect("Infusion compatibility insert failed!");
        }
    }
}

fn connect_db() -> PooledConn {
    let mut config = Ini::new();
    config.load("./db.conf").expect("Failed to load DB config!");
    let host = config.get("db", "host").expect("host not in db.conf!");
    let db_name = config.get("db", "db_name").expect("db_name not in db.conf!");
    let user = config.get("db", "user").expect("user not in db.conf!");
    let password = config.get("db", "password").expect("password not in db.conf!");
    let url = format!("mysql://{}:{}@{}/{}", user, password, host, db_name);

    let pool = Pool::new(url.as_str()).expect("Unable to parse DB url! Fix your code!!!");

    pool.get_conn().expect("Failed to connect to DB!")
}

fn main() {
    let file: File = File::open("./data.csv").expect("Couldn't open file!");
    let lines = io::BufReader::new(file).lines();
    let mut lines = lines.map(|line| { line.expect("Couldn't read from file!")});
    let headers = lines.next().expect("No data in file!");

    let mut data: Vec<InfusionInput> = Vec::new();
    let header_items: Vec<&str> = headers.split(',').skip(2).collect();

    for line in lines {
        println!("{}", line);
        let mut items = line.split(',');
        let name = items.next().expect("Unexpected end of line!").to_string();
        let inf_type = items.next().expect("Unexpected end of line!").parse().expect("Couldn't parse type!");

        let items: Vec<&str> = items.collect();

        if items.len() != header_items.len() {
            panic!("Header and data lines don't have the same size! {}", name);
        }

        let mut infusion = InfusionInput{ name, inf_type, compat: HashMap::new() };
        for (i, item) in items.iter().enumerate() {
            if *item == "" {
                continue;
            }

            let compat_data: Vec<u32> = item.split(':').map(|n| { n.parse().expect("Invalid compatibility data!") }).collect();
            infusion.compat.insert(header_items[i], compat_data);
        }
        data.push(infusion);
    }

    let mut conn = connect_db();
    setup_type_table(&mut conn);

    import_infusions(&mut conn, data);
}