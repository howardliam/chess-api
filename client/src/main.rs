use std::{
    collections::VecDeque,
    ops::ControlFlow,
    sync::{Arc, Mutex},
    time::Duration,
};

use futures::{SinkExt, StreamExt};
use tokio::time;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{error, info};

const URI: &str = "ws://127.0.0.1:3000/ws";

type MessageQueue = Arc<Mutex<VecDeque<String>>>;

#[tokio::main]
async fn main() {
    let queue: MessageQueue = Arc::new(Mutex::new(VecDeque::new()));

    let mut interval = time::interval(Duration::from_secs(1));

    tracing_subscriber::fmt().init();

    let ws_stream = match connect_async(URI).await {
        Ok((stream, _res)) => {
            info!("Handshake completed");
            stream
        }
        Err(err) => {
            error!("Websocket handshake failed with {err}!");
            return;
        }
    };

    let (mut sender, mut receiver) = ws_stream.split();

    let send_queue = queue.clone();
    let mut send_task = tokio::spawn(async move {
        loop {
            interval.tick().await;

            let next_msg = {
                let mut queue = send_queue.lock().unwrap();
                queue.pop_front()
            };

            if let Some(next_msg) = next_msg {
                sender.send(Message::Text(next_msg)).await;
            }
        }
    });

    let recv_queue = queue.clone();
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if process_message(&msg).is_break() {
                break;
            }

            if let Message::Text(t) = msg {
                let res = match t.as_str() {
                    "uci" => "uciok",
                    _ => "unknown command",
                };

                recv_queue.lock().unwrap().push_back(res.to_owned());
            }
        }
    });

    tokio::select! {
        _ = (&mut send_task) => {
            recv_task.abort();
        },
        _ = (&mut recv_task) => {
            send_task.abort();
        }
    }
}

fn process_message(message: &Message) -> ControlFlow<(), ()> {
    match message {
        Message::Text(t) => {
            info!("Received text `{t}`");
        }
        Message::Close(cf) => {
            if let Some(cf) = cf {
                info!(
                    "Received close with code {} and reason {}",
                    cf.code, cf.reason
                );
            }

            return ControlFlow::Break(());
        }
        _ => {}
    }

    ControlFlow::Continue(())
}
