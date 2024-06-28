use crate::db;
use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub enum IncomingMessage {
    Text(TextMessageRecivedProcessed),
    Binary,
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

use chrono::prelude::{DateTime, Utc};

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
    fn to_processed(&self, from: &String) -> TextMessageRecivedProcessed {
        //let config = CONFIG.read().unwrap();
        //let time_recived = if config.register_time_message_recived {
        let time_recived = if true { Some(()) } else { None };
        //let time_sent = if config.register_time_message_sent {
        let time_sent = if true {
            Some(Utc::now().to_string())
        } else {
            None
        };
        let from = from.clone();
        //let payload = self.payload;
        TextMessageRecivedProcessed {
            payload: self.payload.clone(),
            from,
            time_recived,
            time_sent,
        }
    }
}

#[derive(Deserialize, Serialize, Debug)]
pub struct TextMessageRecivedProcessed {
    payload: String,
    from: String,
    time_sent: Option<String>,
    time_recived: Option<()>,
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
                                let mut c = connected_users.lock().unwrap();
                                let inc_message =
                                    IncomingMessage::Text(val.to_processed(&current_id));
                                dbg!(&inc_message);
                                match c.get_mut(&val.destination) {
                                    Some(vector) => {
                                        vector.push(inc_message);
                                        dbg!("MEnsaje enviado a que");
                                    }
                                    None => {
                                        //User is online but it dosent exists? probably Unreachable
                                        //c.insert(val.destination, vec![inc_message]);
                                        db::save_message(&val.destination, &val);
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
