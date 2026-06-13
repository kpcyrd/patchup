use crate::errors::*;
use crate::hub;
use prometheus::{Encoder, IntGauge, Opts, Registry, TextEncoder};
use std::convert::Infallible;
use std::future;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::TcpListener;
use warp::Filter;
use warp::http::StatusCode;
use warp::reject::MethodNotAllowed;

#[derive(Default)]
pub struct Metrics {
    reg: Registry,
}

impl Metrics {
    pub fn gauge(&self, opts: Opts, value: i64) {
        let counter = IntGauge::with_opts(opts).unwrap();
        counter.set(value);
        self.reg.register(Box::new(counter.clone())).unwrap();
    }

    pub fn encode(&self) -> String {
        let mut buffer = vec![];
        let encoder = TextEncoder::new();
        let metric_families = self.reg.gather();
        encoder.encode(&metric_families, &mut buffer).unwrap();
        match String::from_utf8(buffer) {
            Ok(s) => s,
            Err(err) => {
                error!("Failed to convert metrics to UTF-8 string: {err:#}");
                String::new()
            }
        }
    }
}

async fn metrics(shared: Arc<hub::Shared>) -> Box<dyn warp::Reply> {
    let metrics = Metrics::default();

    let opts = Opts::new("hello_world", "Hello world").const_label("hello", "world");
    let state = shared.state.load();
    let count = state.nodes.len() as i64;

    metrics.gauge(opts, count);

    // Encode the metrics
    let buffer = metrics.encode();
    Box::new(buffer)
}

async fn rejection(err: warp::Rejection) -> Result<impl warp::Reply, Infallible> {
    let code;
    let message;

    if err.is_not_found() {
        code = StatusCode::NOT_FOUND;
        message = "404 - file not found\n";
    } else if let Some(_err) = err.find::<MethodNotAllowed>() {
        code = StatusCode::BAD_REQUEST;
        message = "400 - bad request\n";
    } else {
        error!("Unhandled rejection: {:?}", err);
        code = StatusCode::INTERNAL_SERVER_ERROR;
        message = "server error\n";
    }

    Ok(warp::reply::with_status(message, code))
}

pub async fn start(addr: Option<SocketAddr>, shared: Arc<hub::Shared>) -> Result<()> {
    let Some(addr) = addr else {
        return future::pending().await;
    };
    info!("Starting metrics server on {addr}");

    let socket = TcpListener::bind(addr)
        .await
        .context("Failed to bind metrics server")?;

    let filter = warp::path!("metrics")
        .and(warp::any().map(move || shared.clone()))
        .then(metrics);
    let filter = filter.recover(rejection);
    warp::serve(filter).incoming(socket).run().await;

    Ok(())
}
