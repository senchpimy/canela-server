use clap::Parser;
use jwt_simple::prelude::*;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use serde_json;
use warp::Filter;
mod db;
mod ws;
////
use std::{
    collections::HashMap,
    fs,
    sync::{Arc, Mutex, RwLock},
};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(long, default_value_t = 3030)]
    port: u16,
}

#[derive(Deserialize, Serialize)]
struct ServerConfig {
    register_time_message_sent: bool,
    register_time_message_recived: bool,
}

fn load_config() -> ServerConfig {
    let config_data = fs::read_to_string("config.json").expect("Unable to read config file");
    serde_json::from_str(&config_data).expect("Unable to parse config file")
}

lazy_static! {
    static ref CONFIG: RwLock<ServerConfig> = RwLock::new(load_config());
}

#[derive(Deserialize, Serialize)]
struct NewUserResgistered {
    token: String,
    session_token: String,
}

#[tokio::main]
async fn main() {
    let args = Args::parse(); // Assume Args is defined elsewhere
    let connection = db::prepare_db().unwrap(); // Assume connect_to_db() is defined elsewhere
    let valid_connections: db::ValidConnections<String> = Arc::new(Mutex::new(HashMap::new()));
    let CONNECTED_USERS: ws::SINGLEUsersConnected = Arc::new(Mutex::new(HashMap::new()));
    let conn = Arc::new(Mutex::new(connection));

    let connection_req = {
        let conn = Arc::clone(&conn);
        let valid_connections = Arc::clone(&valid_connections);
        move |val: db::ConnectionAttempt| {
            db::validate_connection(val, Arc::clone(&conn), Arc::clone(&valid_connections))
        }
    };

    let hello = warp::path("connect")
        .and(warp::body::json())
        .map(connection_req);

    let ws = {
        let valid_connections = Arc::clone(&valid_connections);
        warp::path("ws")
            .and(warp::ws())
            .and(warp::header("user_token"))
            .and(warp::header("jwt"))
            .map(move |ws: warp::ws::Ws, header_rx: String, jwt: String| {
                let conn_users = Arc::clone(&CONNECTED_USERS);
                let result = is_valid_connection(Arc::clone(&valid_connections), header_rx, jwt);
                ws.on_upgrade(move |socket| {
                    ws::handle_connection(
                        socket,
                        result,
                        Arc::clone(&conn_users),
                        String::from("sss"), //TODO GET USER
                    )
                })
            })
    };

    let routes = warp::get().and(hello.or(ws));
    warp::serve(routes).run(([127, 0, 0, 1], args.port)).await;
}

fn add_valid_connection(valid_conn: db::ValidConnections<String>, user_token: &String) -> String {
    loop {
        match valid_conn.lock() {
            Ok(mut conn) => {
                let key = HS256Key::generate();
                let claims = Claims::create(jwt_simple::prelude::Duration::from_millis(1000));
                let token = key.authenticate(claims).unwrap();
                dbg!(&token);
                conn.insert((*user_token).clone(), key.to_bytes());
                return token;
            }
            Err(_) => {}
        }
    }
}

fn is_valid_connection(
    valid_conn: db::ValidConnections<String>,
    user_token: String,
    jwt: String,
) -> bool {
    loop {
        match valid_conn.lock() {
            Ok(conn) => {
                let validation = conn.get(&user_token);
                if let Some(res) = validation {
                    let key = HS256Key::from_bytes(res);
                    match key.verify_token::<NoCustomClaims>(&jwt, None) {
                        Ok(o) => {
                            let now = Clock::now_since_epoch();
                            let is_expired = o.expires_at.unwrap() < now;
                            if is_expired {
                                println!("Expirado");
                            }
                            return !is_expired;
                        }
                        Err(_) => {
                            return false;
                        }
                    }
                };
            }
            Err(_) => {}
        }
    }
}
