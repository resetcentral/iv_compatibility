use iv_compat::solver::CompatibilityProblem;
use serde::{Deserialize, Serialize};

use mysql::{Pool, PooledConn};
use mysql::prelude::*;

use axum::http::StatusCode;
use axum::response::Html;
use axum::{Router, routing::get };
use axum::extract::State;
use axum_extra::extract::Query;

use tower_http::services::ServeDir;
use minijinja::{Environment, context};
use std::collections::HashSet;
use std::sync::Arc;

use iv_compat::db;

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
struct ResultParams {
    num_ivs: u32,
    ivs: String,
    add: Vec<u32>
}

async fn handler_results(state: State<Arc<AppState>>, params: Query<ResultParams>) -> Result<Html<String>, StatusCode> {
    let mut conn = state.pool.get_conn().expect("Failed to connect to DB!");
    let iv_data: Vec<Vec<u32>>= serde_json::from_str(&params.ivs).expect("Invalid JSON data: ivs");

    let problem = load_problem(&mut conn, params.num_ivs, iv_data, &params.add);
    problem.solve();

    Err(StatusCode::INTERNAL_SERVER_ERROR)
}

fn load_problem(conn: &mut PooledConn, num_ivs: u32, iv_data: Vec<Vec<u32>>, additional: &Vec<u32>) -> CompatibilityProblem {
    let infusion_ids = iv_data.iter().flatten().chain(additional.iter()).collect();
    let infusions = db::load_infusions(conn, infusion_ids);
    
    let ivs: Vec<HashSet<u32>> = iv_data.into_iter().map(
        |iv_infusion_ids| {
            HashSet::from_iter(iv_infusion_ids)
        }
    ).collect();

    CompatibilityProblem::new(num_ivs, ivs, infusions)
}

struct AppState {
    env: Environment<'static>,
    pool: Pool,
}

#[tokio::main]
async fn main() {
    let pool = db::connect_db("./conf.d/db.conf");

    let mut env = Environment::new();
    env.add_template("home", include_str!("../templates/home.jinja")).expect("Failed to load template");

    let app_state = Arc::new(AppState { env, pool });
    let app = Router::new()
        .route("/", get(handler_home))
        .route("/results", get(handler_results))
        .nest_service("/static", ServeDir::new("static"))
        .with_state(app_state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080")
        .await
        .expect("Failed to start TCP listener");

    println!("Web server started");

    axum::serve(listener, app).await.unwrap();
}