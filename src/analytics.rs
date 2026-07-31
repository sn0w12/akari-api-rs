use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Instant;

use axum::body::Body;
use axum::extract::MatchedPath;
use axum::http::Request;
use axum::response::Response;
use sqlx::PgPool;
use tokio::sync::mpsc;
use tower::{Layer, Service};

const BATCH_SIZE: usize = 50;
const FLUSH_INTERVAL: std::time::Duration = std::time::Duration::from_secs(5);

#[derive(Debug, Clone)]
pub struct AnalyticsData {
    pub hostname: String,
    pub ip_address: String,
    pub user_agent: String,
    pub path: String,
    pub method: String,
    pub response_time: i32,
    pub status: i32,
    pub route: String,
    pub country_code: String,
}

#[derive(Clone)]
pub struct AnalyticsLayer {
    tx: mpsc::UnboundedSender<AnalyticsData>,
}

impl AnalyticsLayer {
    pub fn new(pool: PgPool) -> Self {
        let tx = start_collector(pool);
        Self { tx }
    }
}

impl<S> Layer<S> for AnalyticsLayer {
    type Service = AnalyticsMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        AnalyticsMiddleware {
            inner,
            tx: self.tx.clone(),
        }
    }
}

#[derive(Clone)]
pub struct AnalyticsMiddleware<S> {
    inner: S,
    tx: mpsc::UnboundedSender<AnalyticsData>,
}

impl<S> Service<Request<Body>> for AnalyticsMiddleware<S>
where
    S: Service<Request<Body>, Response = Response> + Send + Clone + 'static,
    S::Future: Send + 'static,
    S::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    type Response = Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let start = Instant::now();

        let route = req
            .extensions()
            .get::<MatchedPath>()
            .map(|m| m.as_str().to_string())
            .unwrap_or_else(|| req.uri().path().to_string());

        let path = req.uri().path().to_string();
        let method = req.method().to_string();

        // Don't record analytics requests for the analytics endpoints themselves
        if path.starts_with("/v2/analytics") {
            return Box::pin(self.inner.call(req));
        }

        let hostname = req
            .headers()
            .get("host")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        let ip_address = req
            .headers()
            .get("cf-connecting-ip")
            .and_then(|v| v.to_str().ok())
            .or_else(|| {
                req.headers()
                    .get("x-forwarded-for")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|s| s.split(',').next().map(|s| s.trim()))
            })
            .unwrap_or("")
            .to_string();

        let user_agent = req
            .headers()
            .get("user-agent")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        let country_code = req
            .headers()
            .get("cf-ipcountry")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        let future = self.inner.call(req);
        let tx = self.tx.clone();

        Box::pin(async move {
            let response = future.await?;
            let status = response.status().as_u16() as i32;
            let response_time = start.elapsed().as_millis() as i32;

            let data = AnalyticsData {
                hostname,
                ip_address,
                user_agent,
                path,
                method,
                response_time,
                status,
                route,
                country_code,
            };

            if tx.send(data).is_err() {
                tracing::warn!("Analytics collector dropped, request not recorded");
            }

            Ok(response)
        })
    }
}

fn start_collector(pool: PgPool) -> mpsc::UnboundedSender<AnalyticsData> {
    let (tx, rx) = mpsc::unbounded_channel();

    tokio::spawn(collector_loop(pool, rx));

    tx
}

async fn collector_loop(pool: PgPool, mut rx: mpsc::UnboundedReceiver<AnalyticsData>) {
    let mut batch = Vec::with_capacity(BATCH_SIZE);
    let mut timer = tokio::time::interval(FLUSH_INTERVAL);
    timer.tick().await;

    loop {
        tokio::select! {
            data = rx.recv() => {
                match data {
                    Some(data) => {
                        batch.push(data);
                        if batch.len() >= BATCH_SIZE {
                            flush_batch(&pool, &batch).await;
                            batch.clear();
                        }
                    }
                    None => {
                        if !batch.is_empty() {
                            flush_batch(&pool, &batch).await;
                        }
                        break;
                    }
                }
            }
            _ = timer.tick() => {
                if !batch.is_empty() {
                    flush_batch(&pool, &batch).await;
                    batch.clear();
                }
            }
        }
    }
}

async fn flush_batch(pool: &PgPool, batch: &[AnalyticsData]) {
    let mut qb = sqlx::QueryBuilder::new(
        "INSERT INTO analytics.requests (hostname, ip_address, user_agent, path, method, response_time, status, route, country_code) ",
    );

    qb.push_values(batch, |mut b, data| {
        b.push_bind(&data.hostname)
            .push_bind(&data.ip_address)
            .push_bind(&data.user_agent)
            .push_bind(&data.path)
            .push_bind(&data.method)
            .push_bind(data.response_time)
            .push_bind(data.status)
            .push_bind(&data.route)
            .push_bind(&data.country_code);
    });

    if let Err(e) = qb.build().execute(pool).await {
        tracing::error!("Failed to flush analytics batch: {}", e);
    }
}
