use crate::errors::*;
use crate::hub;
use prometheus::{Encoder, IntGauge, Opts, Registry, TextEncoder};
use std::collections::BTreeMap;
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

fn bump_stats(map: &mut BTreeMap<String, i64>, key: &str) {
    let num = map.entry(key.to_string()).or_default();
    *num = num.saturating_add(1);
}

async fn metrics(shared: Arc<hub::Shared>) -> Box<dyn warp::Reply> {
    let metrics = Metrics::default();
    let state = shared.state.load();

    let mut stats_os = BTreeMap::new();
    let mut stats_os_id = BTreeMap::new();
    let mut stats_arch = BTreeMap::new();
    let mut stats_kernel = BTreeMap::new();

    for (_public_key, node) in &state.nodes {
        bump_stats(&mut stats_os, &node.nodeinfo.os);
        bump_stats(&mut stats_os_id, &node.nodeinfo.os_id);
        bump_stats(&mut stats_arch, &node.nodeinfo.arch);
        bump_stats(&mut stats_kernel, &node.nodeinfo.kernel);
    }

    let opts = Opts::new("node_count_online", "Number of online nodes");
    metrics.gauge(opts, state.nodes.len() as i64);

    for (os, count) in stats_os {
        let opts = Opts::new("node_count_os", "Number of nodes by OS").const_label("os", os);
        metrics.gauge(opts, count);
    }

    for (os_id, count) in stats_os_id {
        let opts =
            Opts::new("node_count_os_id", "Number of nodes by OS ID").const_label("os_id", os_id);
        metrics.gauge(opts, count);
    }

    for (arch, count) in stats_arch {
        let opts = Opts::new("node_count_arch", "Number of nodes by architecture")
            .const_label("arch", arch);
        metrics.gauge(opts, count);
    }

    for (kernel, count) in stats_kernel {
        let opts = Opts::new("node_count_kernel", "Number of nodes by kernel")
            .const_label("kernel", kernel);
        metrics.gauge(opts, count);
    }

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
