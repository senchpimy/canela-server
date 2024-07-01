use crate::db;
use serde::{de::DeserializeOwned, Deserialize, Serialize};

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

pub type SINGLEUsersConnected = Arc<Mutex<HashMap<String, Arc<Vec<IncomingMessage>>>>>;
use core::str;
//Hashmap of current users
//online and a reference to a
//variable that checks for uncoming messages
//
use std::{
    collections::HashMap,
    fmt::Debug,
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

#[derive(Debug)]
pub enum ChatErrors {
    UserDontExist,
    BadFormat,
}

struct ReciverFeedback {
    closed: bool,
    error: Option<ChatErrors>,
}

impl ReciverFeedback {
    fn new() -> Self {
        ReciverFeedback {
            closed: false,
            error: None,
        }
    }
}

struct MessageSend {}
impl MessageSend {
    fn from_str(str: &str) -> Self {
        Self {}
    }
}

#[derive(Serialize)]
struct MessageSendError {
    kind: String,
}
impl MessageSendError {
    fn from_error(str: &str) -> Self {
        Self {
            kind: String::from_str(str).unwrap(),
        }
    }
    fn format(&self) -> String {
        serde_json::to_string(self).unwrap()
    }
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
    state: Arc<RwLock<ReciverFeedback>>,
    personal_vec: Arc<Vec<IncomingMessage>>,
) {
    //let mut index: u64 = 0;
    loop {
        if state.read().unwrap().closed {
            return;
        }

        let mut m: Option<warp::ws::Message> = None;
        if let Some(err) = &state.read().unwrap().error {
            let r = match err {
                ChatErrors::BadFormat => warp::filters::ws::Message::text(
                    MessageSendError::from_error("Bad format!: Can serialize the JSON").format(),
                ),
                ChatErrors::UserDontExist => warp::filters::ws::Message::text(
                    MessageSendError::from_error("User Dont exist").format(),
                ),
            };
            m = Some(r);
        };
        if let Some(message) = m {
            match tx.send(message).await {
                Ok(_) => {} //Enviado con exito
                Err(_) => {
                    let _ = tx.close().await;
                } //Error al enviar
            };
            println!("Mensaje Enviado");
        };
        tokio::time::sleep(Duration::from_secs(1)).await;
    }
}

async fn receving(
    mut rx: SplitStream<warp::ws::WebSocket>,
    connected_users: SINGLEUsersConnected,
    current_id: String,
    state: Arc<RwLock<ReciverFeedback>>,
) {
    loop {
        if let Some(val) = rx.next().await {
            match val {
                Ok(v) => {
                    if v.is_close() {
                        println!("Cerrado");
                        let mut s = state.write().unwrap();
                        s.closed = true;
                        return;
                    }
                    if v.is_text() {
                        let err = handle_sending::<TextMessageRecivedRaw>(
                            v,
                            &connected_users,
                            &current_id,
                        );
                        continue;
                    }
                    if v.is_binary() {
                        let err = handle_sending::<BLOBMessageRecivedRaw>(
                            v,
                            &connected_users,
                            &current_id,
                        );
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
) -> Option<ChatErrors>
where
    T: DeserializeOwned + Debug + ToProcessed,
{
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
                    //let v: &mut Vec<IncomingMessage> = vector.as_mut();
                    v.push(processed);
                    dbg!("Mensaje enviado a que");
                    None
                }
                None => {
                    //User is online but it dosent exists? probably Unreachable
                    //c.insert(val.destination, vec![inc_message]);
                    //
                    match processed {
                        IncomingMessage::Text(v) => db::save_message_text(&v),
                        IncomingMessage::Binary(v) => db::save_message_binary(&v),
                    };
                    None
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
            Some(ChatErrors::BadFormat)
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
    let connection_state = Arc::new(RwLock::new(ReciverFeedback::new()));
    let personal_vec = Arc::clone(connected_users.lock().unwrap().get_mut("").unwrap());
    let tx = tokio::spawn(sending(tx, Arc::clone(&connection_state), personal_vec));
    let rx = tokio::spawn(receving(
        rx,
        connected_users,
        current.id,
        Arc::clone(&connection_state),
    ));
    let res = tokio::try_join!(tx, rx);
}
