use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use utoipa::{OpenApi, ToSchema};

use crate::client::{self, DepError};

pub const SERVICE: &str = "srvcs-vectorscale";
pub const CONCERN: &str = "vectors: scale by a scalar";
pub const DEPENDS_ON: &[&str] = &["srvcs-floatmultiply"];

/// Dependency endpoints, injected as router state so tests can point them at
/// mock services.
#[derive(Clone)]
pub struct Deps {
    pub floatmultiply_url: String,
}

#[derive(Serialize, ToSchema)]
pub struct Info {
    pub service: &'static str,
    pub concern: &'static str,
    pub depends_on: Vec<&'static str>,
}

/// `GET /` — service identity (srvcs service standard).
#[utoipa::path(get, path = "/", responses((status = 200, body = Info)))]
pub async fn index() -> Json<Info> {
    Json(Info {
        service: SERVICE,
        concern: CONCERN,
        depends_on: DEPENDS_ON.to_vec(),
    })
}

#[derive(Deserialize, ToSchema)]
pub struct EvalRequest {
    /// The vector to scale. Each element is passed straight into the
    /// dependency's request body as an operand.
    #[schema(value_type = Object)]
    pub vector: Vec<Value>,
    /// The scalar to multiply every component by.
    #[schema(value_type = Object)]
    pub scalar: Value,
}

fn ok(vector: Vec<Value>, scalar: Value, result: Vec<f64>) -> Response {
    (
        StatusCode::OK,
        Json(json!({ "vector": vector, "scalar": scalar, "result": result })),
    )
        .into_response()
}

fn degraded(dependency: &str) -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(json!({ "error": "dependency unavailable", "dependency": dependency })),
    )
        .into_response()
}

fn forward(status: u16, body: Value) -> Response {
    let code = StatusCode::from_u16(status).unwrap_or(StatusCode::BAD_GATEWAY);
    (code, Json(body)).into_response()
}

/// A reachable dependency answered `200` but its body lacked a numeric
/// `result`. That is a contract violation we cannot recover from, so surface a
/// `500` rather than guessing.
fn malformed(dependency: &str) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(
            json!({ "error": "dependency returned a malformed result", "dependency": dependency }),
        ),
    )
        .into_response()
}

/// Call one dependency at `url` with `body`, mapping its outcome to either the
/// numeric `result` (on `200`) or an early-return `Response` the caller should
/// surface verbatim:
///
/// - unreachable / non-`200`/`422` -> `503` degraded
/// - `422` -> forwarded `422` (the dependency rejected the input)
/// - `200` without a numeric `result` -> `500` malformed
async fn ask(url: &str, body: &Value, dependency: &str) -> Result<f64, Response> {
    match client::call(url, body).await {
        Err(DepError::Unreachable) => Err(degraded(dependency)),
        Ok((200, body)) => match body.get("result").and_then(Value::as_f64) {
            Some(r) => Ok(r),
            None => Err(malformed(dependency)),
        },
        Ok((422, body)) => Err(forward(422, body)),
        Ok(_) => Err(degraded(dependency)),
    }
}

/// `POST /` — scale a vector by a scalar.
///
/// This service owns the *control flow* but delegates every multiplication to
/// `srvcs-floatmultiply`. For each component `c` of the vector it asks for
/// `floatmultiply(c, scalar)` and collects the results into a new vector. The
/// empty vector scales to the empty vector and makes no dependency calls.
///
/// Validation is not handled here: this service never calls `srvcs-isnumber`
/// directly. If the dependency is unreachable it reports itself degraded
/// (`503`); if the dependency rejects a component it forwards the `422`.
#[utoipa::path(
    post,
    path = "/",
    request_body = EvalRequest,
    responses(
        (status = 200, description = "the scaled vector (a JSON array of f64 numbers)"),
        (status = 422, description = "a dependency rejected an input (forwarded)"),
        (status = 500, description = "a dependency returned a malformed result"),
        (status = 503, description = "a dependency is unavailable")
    )
)]
pub async fn evaluate(State(deps): State<Deps>, Json(req): Json<EvalRequest>) -> Response {
    let EvalRequest { vector, scalar } = req;

    let mut result: Vec<f64> = Vec::with_capacity(vector.len());
    for c in &vector {
        let r = match ask(
            &deps.floatmultiply_url,
            &json!({ "a": c, "b": scalar }),
            "srvcs-floatmultiply",
        )
        .await
        {
            Ok(v) => v,
            Err(resp) => return resp,
        };
        result.push(r);
    }

    ok(vector, scalar, result)
}

#[derive(OpenApi)]
#[openapi(paths(index, evaluate), components(schemas(Info, EvalRequest)))]
pub struct ApiDoc;

/// Serve OpenAPI document
pub async fn openapi_json() -> Json<utoipa::openapi::OpenApi> {
    Json(ApiDoc::openapi())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openapi_documents_routes() {
        let doc = ApiDoc::openapi();
        let root = doc.paths.paths.get("/").expect("path / present");
        assert!(root.get.is_some());
        assert!(root.post.is_some());
    }

    #[tokio::test]
    async fn index_reports_dependency() {
        let Json(info) = index().await;
        assert_eq!(info.service, "srvcs-vectorscale");
        assert_eq!(info.concern, "vectors: scale by a scalar");
        assert_eq!(info.depends_on, vec!["srvcs-floatmultiply"]);
    }
}
