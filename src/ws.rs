use crate::db;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::from_slice;
use std::any::Any;

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
    fmt::Debug,
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
pub struct BLOBMessageRecivedRaw {
    pub payload: Vec<u8>,
    pub destination: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct MessageRecivedRawError {
    error: String,
}

trait Processed {
    fn to(&self) -> &String;
}
impl Processed for IncomingMessage {
    fn to(&self) -> &String {
        match self {
            IncomingMessage::Text(v) => &v.to,
            IncomingMessage::Binary(v) => &v.to,
        }
    }
}

trait ToProcessed {
    fn to_processed(self, from: &String) -> IncomingMessage;
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct TextMessageRecivedProcessed {
    pub payload: String,
    pub from: String,
    pub time_sent: String,
    pub to: String,
}

impl ToProcessed for TextMessageRecivedRaw {
    fn to_processed(self, from: &String) -> IncomingMessage {
        //let config = CONFIG.read().unwrap();
        //let time_sent = if config.register_time_message_sent {
        let time_sent = if true {
            Utc::now().to_string()
        } else {
            String::new()
        };
        let from = from.clone();
        //let payload = self.payload;
        IncomingMessage::Text(TextMessageRecivedProcessed {
            payload: self.payload,
            from,
            time_sent,
            to: self.destination,
        })
    }
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct BLOBMessageRecivedProcessed {
    pub payload: Vec<u8>,
    pub from: String,
    pub time_sent: String,
    pub to: String,
}

impl ToProcessed for BLOBMessageRecivedRaw {
    fn to_processed(self, from: &String) -> IncomingMessage {
        //let config = CONFIG.read().unwrap();
        //let time_sent = if config.register_time_message_sent {
        let time_sent = if true {
            Utc::now().to_string()
        } else {
            String::new()
        };
        let from = from.clone();
        //let payload = self.payload;
        IncomingMessage::Binary(BLOBMessageRecivedProcessed {
            payload: self.payload,
            from,
            time_sent,
            to: self.destination,
        })
    }
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
        if let Some(val) = rx.next().await {
            match val {
                Ok(v) => {
                    if v.is_close() {
                        println!("Cerrado");
                        let mut s = state.write().unwrap();
                        *s = true;
                        return;
                    }
                    if v.is_text() {
                        handle_sending::<TextMessageRecivedRaw>(v, &connected_users, &current_id);
                        break;
                    }
                    if v.is_binary() {
                        handle_sending::<BLOBMessageRecivedRaw>(v, &connected_users, &current_id);
                    }
                }
                Err(_) => {}
            };
        }
    }
}

fn handle_sending<T>(
    v: warp::ws::Message,
    connected_users: &SINGLEUsersConnected,
    current_id: &String,
) where
    T: DeserializeOwned + Debug + ToProcessed,
{
    //let str = v.to_str().unwrap();
    //v.into_bytes().
    let result: Result<T, serde_json::Error> = if v.is_binary() {
        serde_json::from_slice(v.into_bytes().as_slice())
    } else {
        serde_json::from_str(v.to_str().unwrap())
    };
    match result {
        Ok(val) => {
            let mut connection = connected_users.lock().unwrap();
            let processed = val.to_processed(&current_id);
            dbg!(&processed);
            match connection.get_mut(processed.to()) {
                Some(vector) => {
                    vector.push(processed);
                    dbg!("Mensaje enviado a que");
                }
                None => {
                    //User is online but it dosent exists? probably Unreachable
                    //c.insert(val.destination, vec![inc_message]);
                    //
                    match processed {
                        IncomingMessage::Text(v) => {
                            match db::save_message_text(&v) {
                                None => {}
                                Some(error) => {
                                    dbg!(error);
                                }
                            };
                        }
                        IncomingMessage::Binary(v) => {
                            println!("Binario!");
                            match db::save_message_binary(&v) {
                                None => {}
                                Some(error) => {
                                    dbg!(error);
                                }
                            };
                        }
                    };
                    //TODO
                    //HANDLE RESULT
                }
            }
        }
        Err(_) => {
            let error = MessageRecivedRawError {
                error: String::from("Bad format"),
            };
            dbg!(error); // Propagate error to sender
        }
    }
}

pub async fn handle_connection(
    websocket: warp::ws::WebSocket,
    valid_conn: bool,
    connected_users: SINGLEUsersConnected,
    current: String,
) {
    if !valid_conn {
        let _ = websocket.close().await;
        println!("Coneccion InValida");
        return;
    }
    println!("Coneccion Valida");
    let (tx, rx) = websocket.split();
    let current = CurrentUser { id: current }; //TODO Let it be the id in the server
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
