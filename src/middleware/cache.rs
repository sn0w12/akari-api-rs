use axum::http::header;
use axum::response::Response;

/// Applies Cache-Control headers to a response.
/// Call this at the end of a handler, or use as a response middleware.
pub enum CacheControl {
    NoCache,
    Public {
        max_age: u32,
        stale_while_revalidate: u32,
    },
    Private {
        max_age: u32,
        stale_while_revalidate: u32,
    },
}

impl CacheControl {
    pub fn apply(&self, resp: &mut Response) {
        let value = match self {
            CacheControl::NoCache => "no-cache, no-store, must-revalidate".to_string(),
            CacheControl::Public {
                max_age,
                stale_while_revalidate,
            } => {
                format!(
                    "public, max-age={}, stale-while-revalidate={}",
                    max_age, stale_while_revalidate
                )
            }
            CacheControl::Private {
                max_age,
                stale_while_revalidate,
            } => {
                format!(
                    "private, max-age={}, stale-while-revalidate={}",
                    max_age, stale_while_revalidate
                )
            }
        };
        resp.headers_mut()
            .insert(header::CACHE_CONTROL, value.parse().unwrap());
    }
}
