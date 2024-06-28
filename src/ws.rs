use crate::db;
use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub enum IncomingMessage {
    Text(TextMessageRecivedProcessed),
    Binary(BLOBMessageRecivedProcessed),
}

use futures_util::{
    stream::{SplitSink, SplitStream},
    SinkExt, StreamExt,
};
struct CurrentUser {
    id: String,
}

pub type SINGLEUsersConnected = Arc<Mutex<HashMap<String, Vec<IncomingMessage>>>>; //Hashmap of current users
                                                                                   //online and a reference to a
                                                                                   //variable that checks for uncoming messages
                                                                                   //
use std::{
    collections::HashMap,
    str::FromStr,
    sync::{Arc, Mutex, RwLock},
    time::Duration,
};

use chrono::prelude::Utc;

#[derive(Deserialize, Serialize, Debug)]
pub struct TextMessageRecivedRaw {
    pub payload: String,
    pub destination: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct TextMessageRecivedRawError {
    error: String,
}

impl TextMessageRecivedRaw {
    fn to_processed(self, from: &String) -> TextMessageRecivedProcessed {
        //let config = CONFIG.read().unwrap();
        //let time_sent = if config.register_time_message_sent {
        let time_sent = if true {
            Utc::now().to_string()
        } else {
            String::new()
        };
        let from = from.clone();
        //let payload = self.payload;
        TextMessageRecivedProcessed {
            payload: self.payload,
            from,
            time_sent,
            to: self.destination,
        }
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub struct TextMessageRecivedProcessed {
    pub payload: String,
    pub from: String,
    pub time_sent: String,
    pub to: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct BLOBMessageRecivedProcessed {
    pub payload: Vec<u8>,
    pub from: String,
    pub time_sent: String,
    pub to: String,
}

async fn sending(
    mut tx: SplitSink<warp::ws::WebSocket, warp::ws::Message>,
    state: Arc<RwLock<bool>>,
) {
    let mut index: u64 = 0;
    loop {
        if *(state.read().unwrap()) {
            println!("Cerrado desde El sender");
            return;
        }
        let m = warp::filters::ws::Message::text(format!("AAAA {}", index));
        match tx.send(m).await {
            Ok(_) => {} //Enviado con exito
            Err(_) => {
                let _ = tx.close().await;
            } //Error al enviar
        };
        index += 1;
        tokio::time::sleep(Duration::from_secs(1)).await;
        println!("mimido");
    }
}

async fn receving(
    mut rx: SplitStream<warp::ws::WebSocket>,
    connected_users: SINGLEUsersConnected,
    current_id: String,
    state: Arc<RwLock<bool>>,
) {
    loop {
        match rx.next().await {
            Some(val) => match val {
                Ok(v) => {
                    if v.is_close() {
                        println!("Cerrado");
                        let mut s = state.write().unwrap();
                        *s = true;
                        return;
                    }
                    if v.is_text() {
                        let s: Result<TextMessageRecivedRaw, serde_json::Error> =
                            serde_json::from_str(v.to_str().unwrap());
                        match s {
                            Ok(val) => {
                                dbg!(&val);
                                let mut connection = connected_users.lock().unwrap();
                                let processed = val.to_processed(&current_id);
                                match connection.get_mut(&processed.to) {
                                    Some(vector) => {
                                        vector.push(IncomingMessage::Text(processed));
                                        dbg!("Mensaje enviado a que");
                                    }
                                    None => {
                                        //User is online but it dosent exists? probably Unreachable
                                        //c.insert(val.destination, vec![inc_message]);
                                        let _result = db::save_message_text(&processed);
                                        //TODO
                                        //HANDLE RESULT
                                    }
                                };
                            }
                            Err(_) => {
                                let error = String::from_str("Bad format").unwrap();
                                let error = TextMessageRecivedRawError { error };
                                dbg!(error); //Propagate Error to sender
                            }
                        }
                    }
                    if v.is_binary() {
                        let _s: Result<BLOBMessageRecivedProcessed, serde_json::Error> =
                            serde_json::from_str(v.to_str().unwrap());
                    }
                }
                Err(_) => {}
            },
            None => {}
        };
    }
}

pub async fn handle_connection(
    websocket: warp::ws::WebSocket,
    valid_conn: bool,
    connected_users: SINGLEUsersConnected,
    cuurent: String,
) {
    if !valid_conn {
        let _ = websocket.close().await;
        println!("Coneccion InValida");
        return;
    }
    println!("Coneccion Valida");
    let (tx, rx) = websocket.split();
    let current = CurrentUser { id: cuurent }; //TODO Let it be the id in the server
    let connection_state = Arc::new(RwLock::new(false));
    let tx = tokio::spawn(sending(tx, Arc::clone(&connection_state)));
    let rx = tokio::spawn(receving(
        rx,
        connected_users,
        current.id,
        Arc::clone(&connection_state),
    ));
    let res = tokio::try_join!(tx, rx);
}
