use actix::prelude::*;
use serde::Serialize;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

// --- Messages ---

#[derive(Message)]
#[rtype(result = "()")]
pub struct ReportPythonLatency(pub f64);

#[derive(Message)]
#[rtype(result = "()")]
pub struct ReportTemplateLatency(pub f64);

#[derive(Message)]
#[rtype(result = "()")]
pub struct ReportRtt(pub f64);

#[derive(Message)]
#[rtype(result = "SystemHealth")]
pub struct GetSystemHealth;

// --- Data Structures ---

#[derive(Serialize, Clone, Debug)]
pub struct LatencyMetrics {
    pub p95_ms: f64,
    pub mean_ms: f64,
    pub percentage_of_rtt: Option<f64>,
}

#[derive(Serialize, Clone, Debug)]
pub struct TimeWindowMetrics {
    pub rtt: LatencyMetrics,
    pub python_interpreter: LatencyMetrics,
    pub template_renderer: LatencyMetrics,
}

#[derive(Message, Serialize, Clone, Debug)]
#[rtype(result = "()")]
pub struct SystemHealth {
    pub thirty_seconds: TimeWindowMetrics,
    pub one_minute: TimeWindowMetrics,
    pub five_minutes: TimeWindowMetrics,
}

struct MetricDataPoint {
    timestamp: Instant,
    value: f64,
}

// --- Actor ---

pub struct HealthActor {
    rtt_data: VecDeque<MetricDataPoint>,
    python_latency_data: VecDeque<MetricDataPoint>,
    template_latency_data: VecDeque<MetricDataPoint>,
}

impl HealthActor {
    pub fn new() -> Self {
        Self {
            rtt_data: VecDeque::new(),
            python_latency_data: VecDeque::new(),
            template_latency_data: VecDeque::new(),
        }
    }
}

impl Actor for HealthActor {
    type Context = Context<Self>;
}

// --- Handlers ---

impl Handler<ReportRtt> for HealthActor {
    type Result = ();
    fn handle(&mut self, msg: ReportRtt, _ctx: &mut Context<Self>) {
        self.rtt_data.push_back(MetricDataPoint {
            timestamp: Instant::now(),
            value: msg.0,
        });
    }
}

impl Handler<ReportPythonLatency> for HealthActor {
    type Result = ();
    fn handle(&mut self, msg: ReportPythonLatency, _ctx: &mut Context<Self>) {
        self.python_latency_data.push_back(MetricDataPoint {
            timestamp: Instant::now(),
            value: msg.0,
        });
    }
}

impl Handler<ReportTemplateLatency> for HealthActor {
    type Result = ();
    fn handle(&mut self, msg: ReportTemplateLatency, _ctx: &mut Context<Self>) {
        self.template_latency_data.push_back(MetricDataPoint {
            timestamp: Instant::now(),
            value: msg.0,
        });
    }
}


impl Handler<GetSystemHealth> for HealthActor {
    type Result = MessageResult<GetSystemHealth>;

    fn handle(&mut self, _msg: GetSystemHealth, _ctx: &mut Context<Self>) -> Self::Result {
        // In a real implementation, you would calculate for 30s, 1m, 5m here.
        // For simplicity, we will calculate for the whole dataset for now.
        let thirty_seconds_metrics = self.calculate_window_metrics(Duration::from_secs(30));

        MessageResult(SystemHealth {
            thirty_seconds: thirty_seconds_metrics.clone(),
            one_minute: thirty_seconds_metrics.clone(), // Placeholder
            five_minutes: thirty_seconds_metrics, // Placeholder
        })
    }
}

impl HealthActor {
    fn calculate_window_metrics(&self, window: Duration) -> TimeWindowMetrics {
        let now = Instant::now();
        
        let calculate_metrics_for = |data: &VecDeque<MetricDataPoint>| -> (f64, f64) {
            let mut values: Vec<f64> = data
                .iter()
                .filter(|dp| now.duration_since(dp.timestamp) < window)
                .map(|dp| dp.value)
                .collect();

            if values.is_empty() {
                return (0.0, 0.0);
            }

            values.sort_by(|a, b| a.partial_cmp(b).unwrap());
            
            let p95_index = (values.len() as f64 * 0.95).floor() as usize;
            let p95 = values[p95_index.min(values.len() - 1)];
            
            let mean = values.iter().sum::<f64>() / values.len() as f64;

            (p95, mean)
        };

        let (rtt_p95, rtt_mean) = calculate_metrics_for(&self.rtt_data);
        let (python_p95, python_mean) = calculate_metrics_for(&self.python_latency_data);
        let (template_p95, template_mean) = calculate_metrics_for(&self.template_latency_data);

        TimeWindowMetrics {
            rtt: LatencyMetrics {
                p95_ms: rtt_p95,
                mean_ms: rtt_mean,
                percentage_of_rtt: None,
            },
            python_interpreter: LatencyMetrics {
                p95_ms: python_p95,
                mean_ms: python_mean,
                percentage_of_rtt: Some(if rtt_mean > 0.0 { (python_mean / rtt_mean) * 100.0 } else { 0.0 }),
            },
            template_renderer: LatencyMetrics {
                p95_ms: template_p95,
                mean_ms: template_mean,
                percentage_of_rtt: Some(if rtt_mean > 0.0 { (template_mean / rtt_mean) * 100.0 } else { 0.0 }),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix::Actor;
    use actix_rt::time;

    #[actix_rt::test]
    async fn test_health_actor_metrics() {
        let addr = HealthActor::new().start();

        // Report some data
        addr.do_send(ReportRtt(10.0));
        addr.do_send(ReportPythonLatency(5.0));
        addr.do_send(ReportTemplateLatency(2.0));

        // Wait for the messages to be processed
        time::sleep(Duration::from_millis(100)).await;

        // Get the health report
        let health = addr.send(GetSystemHealth).await.unwrap();

        // Check the metrics
        let metrics = health.thirty_seconds;
        assert_eq!(metrics.rtt.mean_ms, 10.0);
        assert_eq!(metrics.python_interpreter.mean_ms, 5.0);
        assert_eq!(metrics.template_renderer.mean_ms, 2.0);
        assert_eq!(
            metrics.python_interpreter.percentage_of_rtt,
            Some(50.0)
        );
        assert_eq!(
            metrics.template_renderer.percentage_of_rtt,
            Some(20.0)
        );
    }
}