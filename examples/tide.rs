use open_metrics_client::encoding::text::{encode, Encode};
use open_metrics_client::metrics::counter::Counter;
use open_metrics_client::metrics::family::Family;
use open_metrics_client::registry::Registry;

use std::io::Write;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};

use tide::{Middleware, Next, Request, Result};

#[async_std::main]
async fn main() -> std::result::Result<(), std::io::Error> {
    tide::log::start();

    let mut registry = Registry::default();
    let http_requests_total = Family::<Labels, Counter<AtomicU64>>::default();
    registry.register(
        "http_requests_total",
        "Number of HTTP requests",
        http_requests_total.clone(),
    );

    let middleware = MetricsMiddleware {
        http_requests_total,
    };
    let mut app = tide::with_state(State {
        registry: Arc::new(Mutex::new(registry)),
    });

    app.with(middleware);
    app.at("/").get(|_| async { Ok("Hello, world!") });
    app.at("/metrics")
        .get(|req: tide::Request<State>| async move {
            let mut encoded = Vec::new();
            encode(&mut encoded, &req.state().registry.lock().unwrap()).unwrap();
            Ok(String::from_utf8(encoded).unwrap())
        });
    app.listen("127.0.0.1:8080").await?;

    Ok(())
}

#[derive(Clone, Hash, PartialEq, Eq)]
struct Labels {
    method: Method,
    path: String,
}

impl Encode for Labels {
    fn encode(&self, writer: &mut dyn Write) -> std::io::Result<()> {
        let method = match self.method {
            Method::Get => b"method=\"GET\"",
            Method::Put => b"method=\"PUT\"",
        };
        writer.write_all(method)?;

        writer.write_all(b", path=\"")?;
        writer.write_all(self.path.as_bytes())?;
        writer.write_all(b"\"").map(|_| ())
    }
}

#[derive(Clone, Hash, PartialEq, Eq)]
enum Method {
    Get,
    Put,
}

#[derive(Clone)]
struct State {
    registry: Arc<Mutex<Registry<Family<Labels, Counter<AtomicU64>>>>>,
}

#[derive(Default)]
struct MetricsMiddleware {
    http_requests_total: Family<Labels, Counter<AtomicU64>>,
}

#[tide::utils::async_trait]
impl Middleware<State> for MetricsMiddleware {
    async fn handle(&self, req: Request<State>, next: Next<'_, State>) -> Result {
        let method = match req.method() {
            http_types::Method::Get => Method::Get,
            http_types::Method::Put => Method::Put,
            _ => todo!(),
        };
        let path = req.url().path().to_string();
        let _count = self
            .http_requests_total
            .get_or_create(&Labels { method, path })
            .inc();

        let res = next.run(req).await;
        Ok(res)
    }
}
