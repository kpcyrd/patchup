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

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct MetricGroup {
    os: String,
    os_id: String,
    arch: String,
    kernel: String,
}

async fn metrics(shared: Arc<hub::Shared>) -> Box<dyn warp::Reply> {
    let metrics = Metrics::default();
    let state = shared.state.load();

    let mut stats = BTreeMap::<MetricGroup, i64>::new();
    let mut pending_kernels = BTreeMap::<String, i64>::new();
    let mut patchup_versions = BTreeMap::<String, i64>::new();

    for node in state.nodes.values() {
        // Count nodes
        let group = MetricGroup {
            os: node.nodeinfo.os.clone(),
            os_id: node.nodeinfo.os_id.clone(),
            arch: node.nodeinfo.arch.clone(),
            kernel: node.nodeinfo.kernel.clone(),
        };
        let num = stats.entry(group).or_default();
        *num = num.saturating_add(1);

        // Count pending kernels
        if let Some(kernel) = &node.nodeinfo.pending_kernel {
            let num = pending_kernels.entry(kernel.clone()).or_default();
            *num = num.saturating_add(1);
        }

        // Count patchup versions
        let num = patchup_versions
            .entry(node.nodeinfo.patchup_version.clone())
            .or_default();
        *num = num.saturating_add(1);
    }

    for (group, count) in stats {
        let opts = Opts::new(
            "patchup_node_count",
            "Number of nodes by OS, architecture, kernel, etc",
        )
        .const_label("os", &group.os)
        .const_label("os_id", &group.os_id)
        .const_label("arch", &group.arch)
        .const_label("kernel", &group.kernel);
        metrics.gauge(opts, count);
    }

    for (public_key, node) in &state.nodes {
        let Some(updates) = &node.nodeinfo.updates else {
            continue;
        };
        for (ecosystem, updates) in updates {
            let opts = Opts::new("patchup_node_updates", "Number of pending updates per node")
                .const_label("node", public_key.to_string())
                .const_label("ecosystem", ecosystem);
            metrics.gauge(opts, updates.pending.len() as i64);
        }
    }

    for (kernel, count) in pending_kernels {
        let opts = Opts::new(
            "patchup_pending_kernels",
            "Number of nodes with a pending kernel update",
        )
        .const_label("kernel", kernel);
        metrics.gauge(opts, count);
    }

    for (version, count) in patchup_versions {
        let opts = Opts::new(
            "patchup_agent_versions",
            "Number of nodes running a specific patchup agent version",
        )
        .const_label("version", version);
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
