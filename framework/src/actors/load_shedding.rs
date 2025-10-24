use crate::actors::health::{HealthActor, ReportRtt};
use crate::actors::page_renderer::{PageRendererActor, RenderMessage};
use actix::prelude::*;
use serde::Serialize;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

const METRICS_WINDOW: Duration = Duration::from_secs(30);
const METRICS_CALCULATION_INTERVAL: Duration = Duration::from_secs(1);
const LATENCY_THRESHOLD_MULTIPLIER: f64 = 2.0;

#[derive(Serialize, Clone, Copy, Debug)]
pub enum HealthStatus {
    Healthy,
    Shedding,
}

struct RequestMetric {
    timestamp: Instant,
    duration_ms: f64,
}

#[derive(Message)]
#[rtype(result = "()")]
struct RecordMetric(f64);

#[derive(Message)]
#[rtype(result = "()")]
struct DecrementActive;


pub struct LoadSheddingActor {
    page_renderer: Addr<PageRendererActor>,
    health_actor: Addr<HealthActor>,
    active_requests: usize,
    latency_data: VecDeque<RequestMetric>,
    status: HealthStatus,
    current_p95_latency_ms: f64,
    baseline_latency_ms: f64,
    concurrency_limit: Option<usize>,
}

impl LoadSheddingActor {
    pub fn new(page_renderer: Addr<PageRendererActor>, health_actor: Addr<HealthActor>) -> Self {
        Self {
            page_renderer,
            health_actor,
            active_requests: 0,
            latency_data: VecDeque::new(),
            status: HealthStatus::Healthy,
            current_p95_latency_ms: 0.0,
            baseline_latency_ms: 0.0,
            concurrency_limit: None,
        }
    }

    fn calculate_metrics(&mut self) {
        // Prune old data
        let now = Instant::now();
        self.latency_data.retain(|metric| now.duration_since(metric.timestamp) < METRICS_WINDOW);

        if self.latency_data.is_empty() {
            self.current_p95_latency_ms = 0.0;
            return;
        }

        // Calculate 95th percentile latency
        let mut durations: Vec<f64> = self.latency_data.iter().map(|m| m.duration_ms).collect();
        durations.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let p95_index = (durations.len() as f64 * 0.95).floor() as usize;
        self.current_p95_latency_ms = durations[p95_index.min(durations.len() - 1)];

        // Update baseline (simple moving average for now)
        if self.baseline_latency_ms == 0.0 {
            self.baseline_latency_ms = self.current_p95_latency_ms;
        } else {
            self.baseline_latency_ms = (self.baseline_latency_ms * 0.9) + (self.current_p95_latency_ms * 0.1);
        }

        // Update state machine
        if self.current_p95_latency_ms > self.baseline_latency_ms * LATENCY_THRESHOLD_MULTIPLIER && self.baseline_latency_ms > 0.0 {
            if matches!(self.status, HealthStatus::Healthy) {
                log::warn!("Hold on tight! The system is under high load (P95 Latency: {:.2}ms). We're activating defense mode to keep things running smoothly.", self.current_p95_latency_ms);
                self.status = HealthStatus::Shedding;
                self.concurrency_limit = Some(self.active_requests);
            }
        } else {
            if matches!(self.status, HealthStatus::Shedding) {
                log::info!("Phew! System load has returned to normal (P95 Latency: {:.2}ms). Deactivating defense mode.", self.current_p95_latency_ms);
                self.status = HealthStatus::Healthy;
                self.concurrency_limit = None;
            }
        }
    }
}

impl Actor for LoadSheddingActor {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        ctx.run_interval(METRICS_CALCULATION_INTERVAL, |act, _| {
            act.calculate_metrics();
        });
    }
}


impl Handler<RecordMetric> for LoadSheddingActor {
    type Result = ();

    fn handle(&mut self, msg: RecordMetric, _ctx: &mut Context<Self>) -> Self::Result {
        self.latency_data.push_back(RequestMetric {
            timestamp: Instant::now(),
            duration_ms: msg.0,
        });
    }
}

impl Handler<DecrementActive> for LoadSheddingActor {
    type Result = ();

    fn handle(&mut self, _msg: DecrementActive, _ctx: &mut Context<Self>) -> Self::Result {
        self.active_requests -= 1;
    }
}


impl Handler<RenderMessage> for LoadSheddingActor {
    type Result = ResponseFuture<Result<String, crate::errors::DetailedError>>;

    fn handle(&mut self, msg: RenderMessage, ctx: &mut Context<Self>) -> Self::Result {
        if let Some(limit) = self.concurrency_limit {
            if self.active_requests >= limit && msg.request_info.path != "/health" {
                return Box::pin(async { Err(crate::errors::DetailedError {
                    error_source: Some(crate::errors::ErrorSource::Python(crate::actors::interpreter::PythonError {
                        message: "Timeout".to_string(),
                        traceback: "".to_string(),
                        line_number: None,
                        filename: None,
                    })),
                    ..Default::default()
                }) });
            }
        }

        self.active_requests += 1;
        let start_time = Instant::now();
        let page_renderer = self.page_renderer.clone();
        let addr = ctx.address();
        let health_addr = self.health_actor.clone();

        Box::pin(async move {
            let result = page_renderer.send(msg).await;
            let duration_ms = start_time.elapsed().as_secs_f64() * 1000.0;
            
            // Fork metrics to both actors
            addr.do_send(RecordMetric(duration_ms));
            health_addr.do_send(ReportRtt(duration_ms));
            
            addr.do_send(DecrementActive);

            match result {
                Ok(Ok(rendered)) => Ok(rendered),
                Ok(Err(e)) => Err(e),
                Err(e) => {
                    log::error!("A mailbox error occurred in the load shedder: {}. This might indicate a problem with the server's internal communication.", e);
                    Err(crate::errors::DetailedError {
                        error_source: Some(crate::errors::ErrorSource::Python(crate::actors::interpreter::PythonError {
                            message: e.to_string(),
                            traceback: format!("{:?}", e),
                            line_number: None,
                            filename: None,
                        })),
                        ..Default::default()
                    })
                }
            }
        })
    }
}