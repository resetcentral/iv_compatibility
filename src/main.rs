use serde::{Deserialize, Serialize};

use mysql::{Pool, PooledConn};
use mysql::prelude::*;

use axum::http::StatusCode;
use axum::response::Html;
use axum::{Router, routing::get };
use axum::extract::{State};
use axum_extra::extract::Query;

use tower_http::services::ServeDir;
use minijinja::{Environment, context};
use std::collections::HashMap;
use std::sync::Arc;
use serde_urlencoded;

use iv_compat::db;
use iv_compat::infusions::{Infusion, Iv};

async fn handler_home(state: State<Arc<AppState>>) -> Result<Html<String>, StatusCode> {
    #[derive(Serialize, Deserialize, Debug)]
    pub struct SimpleInfusion {
        id: u32,
        name: String,
        inf_type: u32,
    }

    let template = state.env.get_template("home").expect("Template not found!");

    let mut conn = state.pool.get_conn().expect("Failed to connect to DB!");
    let infusions: Vec<SimpleInfusion> = conn
        .query_map(
            "SELECT id, name, type FROM infusion ORDER BY id",
            |(id, name, inf_type)| {
                SimpleInfusion { id, name, inf_type }
        })
        .expect("[DB] Couldn't read from infusion table!");

    let rendered = template
        .render(context!(inf => infusions))
        .expect("Unable to render home page");

    Ok(Html(rendered))
}

#[derive(Serialize, Deserialize, Debug)]
struct Params {
    num_ivs: u32,
    ivs: String,
    add: Vec<u32>
}

// impl Params {
//     fn new(num_ivs: u32, ivs_json: &str, add: Vec<u32>) -> Self {
//         Self {
//             num_ivs,
//             ivs: serde_json::from_str(ivs_json).unwrap(),
//             add,
//         }
//     }
// }

async fn handler_results(state: State<Arc<AppState>>, params: Query<Params>) -> Result<Html<String>, StatusCode> {
    // let data = vec![ vec![1,2,3], vec![4,5,6]];
    // let add = vec![1,2,3];
    // let params = Params {
    //     num_ivs: 2,
    //     ivs: serde_json::to_string(&data).unwrap(),
    //     add: add
    // };
    let mut conn = state.pool.get_conn().expect("Failed to connect to DB!");
    let iv_data: Vec<Vec<u32>>= serde_json::from_str(&params.ivs).expect("Invalid JSON data: ivs");

    let infusion_ids = iv_data.iter().flatten().chain(params.add.iter());

    let infusions = db::get_infusions_by_id(&mut conn, infusion_ids.collect());
    //db::get_compatibility()


    let mut ivs = Vec::new();

    for infusion_data in iv_data {
        let mut iv = Iv::new();
        for infusion_id in infusion_data {
            iv.add_infusion(infusions.get(&infusion_id).unwrap());
        }
        ivs.push(iv);
    }

    let additional_infusions: Vec<&Infusion> = infusions.values().filter(|inf| params.add.contains(&inf.get_id())).collect();

    // iv_compat::infusions::solve_compatibility(params.num_ivs, ivs, additional_infusions);

    println!("{:#?}", params);

    return Err(StatusCode::INTERNAL_SERVER_ERROR);
}

struct AppState {
    env: Environment<'static>,
    pool: Pool,
}

#[tokio::main]
async fn main() {
    let pool = db::connect_db();

    let mut env = Environment::new();
    env.add_template("home", include_str!("../templates/home.jinja")).expect("Failed to load template");

    let app_state = Arc::new(AppState { env, pool });
    let app = Router::new()
        .route("/", get(handler_home))
        .route("/results", get(handler_results))
        .nest_service("/static", ServeDir::new("static"))
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080")
        .await
        .expect("Failed to start TCP listener");

    println!("Web server started");

    axum::serve(listener, app).await.unwrap();
}