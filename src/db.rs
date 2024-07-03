use crate::{
    add_valid_connection,
    ws::{BLOBMessageRecivedProcessed, ChatErrors, TextMessageRecivedProcessed},
    NewUserResgistered,
};
use rusqlite::{params, Connection, Result};
static DB_NAME: &str = "canela-server.db";
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

#[derive(Deserialize, Serialize)]
pub struct ConnectionAttempt {
    password: String,
    user: String,
    token: String,
}

pub type ValidConnections<S> = Arc<Mutex<HashMap<S, (Vec<u8>, S)>>>;

struct DbSearch {
    id: usize,
    connections_left: i32,
}

pub fn prepare_db() -> Result<Connection, rusqlite::Error> {
    let conn = Connection::open(DB_NAME)?;
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
    date TEXT NOT NULL,
    sender TEXT NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users (id));",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS undelivered_messages_blob (
    id INTEGER PRIMARY KEY,
    user_id INTEGER NOT NULL,
    data blob NOT NULL,
    date TEXT NOT NULL,
    sender TEXT NOT NULL,
    FOREIGN KEY (user_id) REFERENCES users (id));",
        [],
    )?;
    Ok(conn)
}

pub fn connect_to_db() -> Result<Connection, rusqlite::Error> {
    let conn = Connection::open(DB_NAME)?;
    Ok(conn)
}

pub fn save_message_text(recieved: &TextMessageRecivedProcessed) -> Option<ChatErrors> {
    let conn = connect_to_db().unwrap();
    let mut search = conn.prepare("SELECT 1 FROM users WHERE id=?").unwrap();
    let rows = search.query_map(&[&recieved.to], |_| Ok(())).unwrap();
    for _r in rows {
        let dbg = conn.execute(
            "INSERT INTO undelivered_messages (user_id, message,sender,date) VALUES (?1, ?2, ?3, ?4)",
            [
                &recieved.to,
                &recieved.payload,
                &recieved.from,
                &recieved.time_sent,
            ],
        );

        match dbg {
            Ok(a) => {
                dbg!(a);
            }
            Err(error) => {
                dbg!(error);
                //return Some(());
            }
        }
        return None;
    }
    Some(ChatErrors::UserDontExist)
}

pub fn save_message_binary(recieved: &BLOBMessageRecivedProcessed) -> Option<ChatErrors> {
    let conn = connect_to_db().unwrap();
    let mut search = conn.prepare("SELECT 1 FROM users WHERE id=?").unwrap();
    let rows = search.query_map(&[&recieved.to], |_| Ok(())).unwrap();
    for _r in rows {
        //let blob = rusqlite::blob::Blob::; //::write_all(recieved.payload.as_slice(), 0).unwrap();
        //
        let dbg = conn.execute(
            "INSERT INTO undelivered_messages_blob (user_id, data,sender,date) VALUES (?1, ?2, ?3)",
            params![
                &recieved.to,
                &recieved.payload,
                &recieved.from,
                &recieved.time_sent,
            ],
        );

        match dbg {
            Ok(a) => {
                dbg!(a);
            }
            Err(error) => {
                dbg!(error);
                //return Some(());
            }
        }
        return None;
    }
    Some(ChatErrors::UserDontExist)
}

pub fn validate_connection(
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
        let token_str = token.to_string();
        let res = r.execute(
            "INSERT INTO users (token, connections_left) VALUES (?1, ?2)",
            params![&token_str, 20],
        );
        match res {
            Ok(_) => {}
            Err(e) => {
                dbg!(e);
                return Err("Error inserting new user the db");
            }
        }
        let mut res = r.prepare("SELECT id FROM users WHERE token=?").unwrap();
        let row = res
            .query_map(&[&token_str], |r| Ok(r.get::<usize, usize>(0).unwrap()))
            .unwrap();
        for r in row {
            let str_id = r.unwrap();
            let user_token = token_str;
            let session_token = add_valid_connection(valid_conn, &user_token, str_id.to_string()); //TODO get id of
                                                                                                   //inserted user
            let response = NewUserResgistered {
                token: user_token,
                session_token,
            };
            return Ok(warp::reply::json(&response)); //reply the token and connections left
        }
        return Err("Weird unreacheble error?");
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
        dbg!(&r.id);
        let session_token = add_valid_connection(valid_conn, &data.token, r.id.to_string());
        let response = NewUserResgistered {
            token: "Connection stablished".to_string(),
            session_token,
        };
        return Ok(warp::reply::json(&response)); //reply the token and connections left
    }
    Err("User not In the Server") //Its actually reachable
}
