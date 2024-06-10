use rusqlite::{Connection, Result};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use uuid::Uuid;
//use tokio::sync::Mutex;

use clap::Parser;
use warp::Filter;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long, default_value_t = 3030)]
    port: u16,
}

#[derive(Deserialize, Serialize)]
struct ConnectionAttempt {
    password: String,
    user: String,
    token: String,
}

#[derive(Deserialize, Serialize)]
struct NewUserResgistered {
    token: String,
}

struct DbSearch {
    id: i32,
    connections_left: i32,
}

#[tokio::main]
async fn main() -> Result<(), rusqlite::Error> {
    let args = Args::parse();
    let connection = connect_to_db()?;
    let conn = Arc::new(Mutex::new(connection));
    let connection_req = move |val: ConnectionAttempt| validate_connection(val, Arc::clone(&conn));
    let hello = warp::path("connect")
        .and(warp::body::json())
        .map(connection_req);

    warp::serve(hello).run(([127, 0, 0, 1], args.port)).await;
    Ok(())
}

fn validate_connection(data: ConnectionAttempt, db: Arc<Mutex<Connection>>) -> impl warp::Reply {
    let server_require_password = true; //TODO Get from config
    let server_can_generate_new_tokens = true; //TODO Get from config
    let server_password = String::from(""); //TODO Get from config
    if server_require_password {
        if data.password != server_password {
            return Err("Wrong password");
        }
    }
    let r = db.lock().unwrap();
    if data.token.len() == 0 && server_can_generate_new_tokens {
        let token = Uuid::new_v4();
        let res = r.execute(
            &format!(
                "INSERT INTO users (token, connections_left) VALUES ('{}', {})",
                token,
                20 //TODO Get from config
            ),
            [],
        );
        match res {
            Ok(_) => {}
            Err(_) => {
                return Err("Error in the db");
            }
        }
        let response = NewUserResgistered {
            token: token.to_string(),
        };
        return Ok(warp::reply::json(&response)); //reply the token and connections left
    }
    let mut search = r
        .prepare("SELECT connections_left,id FROM users WHERE token=?")
        .unwrap();
    let rows = search
        .query_map(&[&data.token], |row| {
            Ok(DbSearch {
                connections_left: row.get(0).unwrap(),
                id: row.get(1).unwrap(),
            })
        })
        .unwrap();
    for r in rows {
        let r = r.unwrap();
        dbg!(r.connections_left);
        dbg!(r.id);
        let response = NewUserResgistered {
            token: "Connection stablished".to_string(),
        };
        return Ok(warp::reply::json(&response)); //reply the token and connections left
    }
    Err("Unreachable")
}

fn connect_to_db() -> Result<Connection, rusqlite::Error> {
    let conn = Connection::open("canela-server.db")?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS users (
                  id INTEGER PRIMARY KEY,
                  token TEXT NOT NULL,
                  connections_left INTEGER NOT NULL)",
        [],
    )?;
    Ok(conn)
}
