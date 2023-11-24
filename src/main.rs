use async_stream::stream;
use axum::{
    response::sse::{Event, KeepAlive, Sse},
    routing::get,
    Router,
};
use futures_util::stream::Stream;
use std::net::SocketAddr;
use std::{convert::Infallible, process::Stdio, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
    sync::mpsc,
};
use tokio_stream::StreamExt as _;

mod other;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().init();
    let app = Router::new().route("/sse", get(sse_handler)).merge(other::using_serve_dir());
    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::info!("listening on {}", addr);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

pub fn open_proc() -> mpsc::Receiver<String> {
    let (tx, rx) = mpsc::channel::<String>(32);

    let mut command = Command::new("./target/debug/dgen");
    command.stdout(Stdio::piped());
    let mut child = command.spawn().unwrap();
    let stdout = child.stdout.take().unwrap();
    let mut reader = BufReader::new(stdout).lines();

    tokio::spawn(async move {
        loop {
            let line = match reader.next_line().await {
                Err(e) => {
                    tracing::error!("Failed to  get line from stdout reader: {e}");
                    break;
                }
                Ok(l) => l,
            };

            if let Some(l) = line {
                match tx.send(l).await {
                    Ok(..) => {
                        tracing::debug!("Sent message");
                    }
                    Err(e) => {
                        tracing::debug!("Channel closed: {e}");
                        break;
                    }
                }
            } else {
                break;
            }
        }

        match child.kill().await {
            Err(e) => tracing::error!("Failed to kill journalctl child: {e}"),
            _ => {
                tracing::debug!("Killed journalctl child")
            }
        }
    });

    rx
}

async fn sse_handler() -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    // A `Stream` that repeats an event every second
    let mut channel = open_proc();
    let stream = stream! {
        tracing::debug!("Lol");

            while let Some(line) = channel.recv().await {
                tracing::debug!("Got line {line}");
                yield Event::default().data(line);
            }
            yield Event::default()
    }
    .map(Ok)
    .throttle(Duration::from_millis(250));

    Sse::new(stream).keep_alive(KeepAlive::default())
}