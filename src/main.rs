use clap::Parser;
use jwt_simple::prelude::*;
use rusqlite::{Connection, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use warp::{filters::header::header, Filter};

use core::panic;
////
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, RwLock},
};

use futures_util::{FutureExt, StreamExt};

type ValidConnections<S> = Arc<Mutex<HashMap<S, Vec<u8>>>>;

////
///
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(long, default_value_t = 3030)]
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
    session_token: String,
}

struct DbSearch {
    id: i32,
    connections_left: i32,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse(); // Assume Args is defined elsewhere
    let connection = connect_to_db()?; // Assume connect_to_db() is defined elsewhere
    let valid_connections: ValidConnections<String> = Arc::new(Mutex::new(HashMap::new()));
    let conn = Arc::new(Mutex::new(connection));

    let connection_req = {
        let conn = Arc::clone(&conn);
        let valid_connections = Arc::clone(&valid_connections);
        move |val: ConnectionAttempt| {
            validate_connection(val, Arc::clone(&conn), Arc::clone(&valid_connections))
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
                let result = is_valid_connection(Arc::clone(&valid_connections), header_rx, jwt);
                ws.on_upgrade(move |socket| handle_connection(socket, result))
            })
    };

    let routes = warp::get().and(hello.or(ws));
    warp::serve(routes).run(([127, 0, 0, 1], args.port)).await;
    Ok(())
}

async fn handle_connection(websocket: warp::ws::WebSocket, valid_conn: bool) {
    if !valid_conn {
        let _ = websocket.close().await;
        println!("Coneccion InValida");
        return;
    }
    //Coneccion valida
    let (mut tx, mut rx) = websocket.split();
    let mut index = 0;
    loop {
        //let m = warp::filters::ws::Message::text(format!("AAAA {}", index));
        //match tx.send(m).await {
        //    Ok(_) => {} //Enviado con exito
        //    Err(_) => {
        //        let _ = tx.close().await;
        //    } //Error al enviar
        //};
        //index += 1;

        match rx.next().now_or_never() {
            Some(val) => match val {
                Some(v) => {
                    println!("Recived Message");
                    match v {
                        Ok(v) => {
                            dbg!(v);
                        }
                        Err(_) => {}
                    }
                    loop {}
                }
                None => {}
            },
            None => {
                //println!("No hay valor q leer");
            }
        };
    }
}

fn validate_connection(
    data: ConnectionAttempt,
    db: Arc<Mutex<Connection>>,
    valid_conn: ValidConnections<String>,
) -> impl warp::Reply {
    let server_require_password = true; //TODO Get from config
    let server_can_generate_new_tokens = true; //TODO Get from config
    let server_password = String::from(""); //TODO Get from config
    if server_require_password {
        if data.password != server_password {
            return Err("Wrong password");
        }
    }
    let r = db.lock().unwrap();
    //Case where user is not registered, so we add them to the whitelist and send back the token
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
        let user_token = token.to_string();
        let session_token = add_valid_connection(valid_conn, &user_token);
        let response = NewUserResgistered {
            token: user_token,
            session_token,
        };
        return Ok(warp::reply::json(&response)); //reply the token and connections left
    }

    //Case where the user is registered
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
        let session_token = add_valid_connection(valid_conn, &data.token);
        let response = NewUserResgistered {
            token: "Connection stablished".to_string(),
            session_token,
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
    conn.execute(
        "CREATE TABLE IF NOT EXISTS undelivered_messages (
    id INTEGER PRIMARY KEY,
    user_id INTEGER NOT NULL,
    message TEXT NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users (id));",
        [],
    )?;
    Ok(conn)
}

fn add_valid_connection(valid_conn: ValidConnections<String>, user_token: &String) -> String {
    loop {
        match valid_conn.lock() {
            Ok(mut conn) => {
                let key = HS256Key::generate();
                let claims = Claims::create(Duration::from_millis(1000));
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
    valid_conn: ValidConnections<String>,
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
                        Ok(_) => {
                            return true;
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
