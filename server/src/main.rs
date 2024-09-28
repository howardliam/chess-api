use std::{net::SocketAddr, ops::ControlFlow};

use axum::{
    extract::{
        ws::{Message, WebSocket},
        ConnectInfo, WebSocketUpgrade,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use futures::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tracing::{error, info};

const URL: &str = "127.0.0.1:3000";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().init();

    let listener = TcpListener::bind(URL).await?;
    let app = Router::new().route("/ws", get(ws_handler));

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;

    Ok(())
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
) -> impl IntoResponse {
    info!("{addr} connected");
    ws.on_upgrade(move |socket| handle_socket(socket, addr))
}

async fn handle_socket(socket: WebSocket, addr: SocketAddr) {
    let (mut sender, mut receiver) = socket.split();

    let mut send_task = tokio::spawn(async move {
        sender.send(Message::Text("uci".into())).await;
        loop {}
    });

    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if process_message(msg).is_break() {
                break;
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

fn process_message(message: Message) -> ControlFlow<(), ()> {
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
