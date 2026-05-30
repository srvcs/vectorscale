use axum::body::Body;
use axum::extract::Json as AxumJson;
use axum::http::{Request, StatusCode};
use axum::routing::post;
use axum::{Json, Router as AxumRouter};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use srvcs_vectorscale::{api::Deps, health, router, telemetry};
use tower::ServiceExt;

const DEAD_URL: &str = "http://127.0.0.1:1";

// --- Computing mocks for every srvcs primitive this family composes over.
//
// Each reads its operands from the request body and returns the *real* answer,
// so the orchestration is genuinely exercised rather than fed a canned value.
// vectorscale only calls floatmultiply; the rest are provided for completeness
// of the family's contract (and to keep the mocks honest about what each
// primitive computes).

/// `srvcs-floatadd`: reads `{a, b}` -> `{"result": a + b}` (as f64).
#[allow(dead_code)]
async fn spawn_floatadd() -> String {
    let app = AxumRouter::new().route(
        "/",
        post(|AxumJson(body): AxumJson<Value>| async move {
            let a = body.get("a").and_then(Value::as_f64).unwrap_or(0.0);
            let b = body.get("b").and_then(Value::as_f64).unwrap_or(0.0);
            Json(json!({ "result": a + b }))
        }),
    );
    serve(app).await
}

/// `srvcs-floatsubtract`: reads `{a, b}` -> `{"result": a - b}` (as f64).
#[allow(dead_code)]
async fn spawn_floatsubtract() -> String {
    let app = AxumRouter::new().route(
        "/",
        post(|AxumJson(body): AxumJson<Value>| async move {
            let a = body.get("a").and_then(Value::as_f64).unwrap_or(0.0);
            let b = body.get("b").and_then(Value::as_f64).unwrap_or(0.0);
            Json(json!({ "result": a - b }))
        }),
    );
    serve(app).await
}

/// `srvcs-floatmultiply`: reads `{a, b}` -> `{"result": a * b}` (as f64).
async fn spawn_floatmultiply() -> String {
    let app = AxumRouter::new().route(
        "/",
        post(|AxumJson(body): AxumJson<Value>| async move {
            let a = body.get("a").and_then(Value::as_f64).unwrap_or(0.0);
            let b = body.get("b").and_then(Value::as_f64).unwrap_or(0.0);
            Json(json!({ "result": a * b }))
        }),
    );
    serve(app).await
}

/// `srvcs-floatdivide`: reads `{a, b}` -> `{"result": a / b}` (as f64).
#[allow(dead_code)]
async fn spawn_floatdivide() -> String {
    let app = AxumRouter::new().route(
        "/",
        post(|AxumJson(body): AxumJson<Value>| async move {
            let a = body.get("a").and_then(Value::as_f64).unwrap_or(0.0);
            let b = body.get("b").and_then(Value::as_f64).unwrap_or(1.0);
            Json(json!({ "result": a / b }))
        }),
    );
    serve(app).await
}

/// `srvcs-sqrt`: reads `{value}` -> `{"result": sqrt(value)}` (as f64).
#[allow(dead_code)]
async fn spawn_sqrt() -> String {
    let app = AxumRouter::new().route(
        "/",
        post(|AxumJson(body): AxumJson<Value>| async move {
            let value = body.get("value").and_then(Value::as_f64).unwrap_or(0.0);
            Json(json!({ "result": value.sqrt() }))
        }),
    );
    serve(app).await
}

/// `srvcs-acos`: reads `{value}` -> `{"result": acos(value)}` (as f64).
#[allow(dead_code)]
async fn spawn_acos() -> String {
    let app = AxumRouter::new().route(
        "/",
        post(|AxumJson(body): AxumJson<Value>| async move {
            let value = body.get("value").and_then(Value::as_f64).unwrap_or(0.0);
            Json(json!({ "result": value.acos() }))
        }),
    );
    serve(app).await
}

/// `srvcs-magnitude`: reads `{vector}` -> `{"result": |vector|}` (the real
/// vector length, as f64).
#[allow(dead_code)]
async fn spawn_magnitude() -> String {
    let app = AxumRouter::new().route(
        "/",
        post(|AxumJson(body): AxumJson<Value>| async move {
            let sum_sq: f64 = body
                .get("vector")
                .and_then(Value::as_array)
                .map(|xs| {
                    xs.iter()
                        .filter_map(Value::as_f64)
                        .map(|x| x * x)
                        .sum::<f64>()
                })
                .unwrap_or(0.0);
            Json(json!({ "result": sum_sq.sqrt() }))
        }),
    );
    serve(app).await
}

/// `srvcs-dotproduct`: reads `{a, b}` -> `{"result": a·b}` (the real dot
/// product, as f64).
#[allow(dead_code)]
async fn spawn_dotproduct() -> String {
    let app = AxumRouter::new().route(
        "/",
        post(|AxumJson(body): AxumJson<Value>| async move {
            let a = body.get("a").and_then(Value::as_array).cloned();
            let b = body.get("b").and_then(Value::as_array).cloned();
            let result = match (a, b) {
                (Some(a), Some(b)) => a
                    .iter()
                    .zip(b.iter())
                    .map(|(x, y)| x.as_f64().unwrap_or(0.0) * y.as_f64().unwrap_or(0.0))
                    .sum::<f64>(),
                _ => 0.0,
            };
            Json(json!({ "result": result }))
        }),
    );
    serve(app).await
}

/// `srvcs-vectorsubtract`: reads `{a, b}` -> `{"result": [a - b]}` (the real
/// component-wise difference array).
#[allow(dead_code)]
async fn spawn_vectorsubtract() -> String {
    let app = AxumRouter::new().route(
        "/",
        post(|AxumJson(body): AxumJson<Value>| async move {
            let a = body.get("a").and_then(Value::as_array).cloned();
            let b = body.get("b").and_then(Value::as_array).cloned();
            let result: Vec<f64> = match (a, b) {
                (Some(a), Some(b)) => a
                    .iter()
                    .zip(b.iter())
                    .map(|(x, y)| x.as_f64().unwrap_or(0.0) - y.as_f64().unwrap_or(0.0))
                    .collect(),
                _ => Vec::new(),
            };
            Json(json!({ "result": result }))
        }),
    );
    serve(app).await
}

/// Spawn a mock returning a fixed status + body (used for error-path tests).
async fn spawn_fixed(status: StatusCode, body: Value) -> String {
    let app = AxumRouter::new().route(
        "/",
        post(move || {
            let body = body.clone();
            async move { (status, Json(body)) }
        }),
    );
    serve(app).await
}

async fn serve(app: AxumRouter) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{addr}")
}

fn app(deps: Deps) -> axum::Router {
    router(telemetry::metrics_handle_for_tests(), deps)
}

async fn real_deps() -> Deps {
    Deps {
        floatmultiply_url: spawn_floatmultiply().await,
    }
}

fn dead_deps() -> Deps {
    Deps {
        floatmultiply_url: DEAD_URL.to_string(),
    }
}

async fn vectorscale(deps: Deps, vector: Value, scalar: Value) -> (StatusCode, Value) {
    let res = app(deps)
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "vector": vector, "scalar": scalar }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

async fn status_of(uri: &str) -> StatusCode {
    app(dead_deps())
        .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
        .await
        .unwrap()
        .status()
}

/// Extract `result` as a `Vec<f64>`, asserting it is a JSON array of numbers.
fn result_vec(body: &Value) -> Vec<f64> {
    body["result"]
        .as_array()
        .expect("result is a JSON array")
        .iter()
        .map(|v| v.as_f64().expect("element is a JSON number"))
        .collect()
}

/// Element-wise float comparison within 1e-9.
fn approx_eq(got: &[f64], expected: &[f64]) {
    assert_eq!(
        got.len(),
        expected.len(),
        "length mismatch: {got:?} vs {expected:?}"
    );
    for (g, e) in got.iter().zip(expected.iter()) {
        assert!((g - e).abs() < 1e-9, "got {g}, expected {e}");
    }
}

// --- Standard endpoints. ---

#[tokio::test]
async fn healthz_ok() {
    assert_eq!(status_of("/healthz").await, StatusCode::OK);
}

#[tokio::test]
async fn readyz_reflects_state() {
    health::set_ready(true);
    assert_eq!(status_of("/readyz").await, StatusCode::OK);
}

#[tokio::test]
async fn metrics_ok() {
    assert_eq!(status_of("/metrics").await, StatusCode::OK);
}

#[tokio::test]
async fn openapi_ok() {
    assert_eq!(status_of("/openapi.json").await, StatusCode::OK);
}

#[tokio::test]
async fn generates_request_id_when_absent() {
    let res = app(dead_deps())
        .oneshot(
            Request::builder()
                .uri("/healthz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert!(
        res.headers().contains_key("x-request-id"),
        "response must carry a generated x-request-id"
    );
}

#[tokio::test]
async fn index_reports_identity() {
    let res = app(dead_deps())
        .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["service"], "srvcs-vectorscale");
    assert_eq!(body["concern"], "vectors: scale by a scalar");
    assert_eq!(body["depends_on"], json!(["srvcs-floatmultiply"]));
}

// --- Correctness cases, against the computing floatmultiply mock. ---

#[tokio::test]
async fn scale_1_2_3_by_2() {
    let (status, body) = vectorscale(real_deps().await, json!([1, 2, 3]), json!(2)).await;
    assert_eq!(status, StatusCode::OK);
    approx_eq(&result_vec(&body), &[2.0, 4.0, 6.0]);
    // input is echoed back verbatim
    assert_eq!(body["vector"], json!([1, 2, 3]));
    assert_eq!(body["scalar"], json!(2));
}

#[tokio::test]
async fn scale_by_zero_is_zero_vector() {
    let (status, body) = vectorscale(real_deps().await, json!([5, -3, 10]), json!(0)).await;
    assert_eq!(status, StatusCode::OK);
    approx_eq(&result_vec(&body), &[0.0, 0.0, 0.0]);
}

#[tokio::test]
async fn scale_by_negative() {
    let (status, body) = vectorscale(real_deps().await, json!([1, -2, 3]), json!(-2)).await;
    assert_eq!(status, StatusCode::OK);
    approx_eq(&result_vec(&body), &[-2.0, 4.0, -6.0]);
}

#[tokio::test]
async fn scale_fractional() {
    let (status, body) = vectorscale(real_deps().await, json!([1.5, 2.0, -4.0]), json!(0.5)).await;
    assert_eq!(status, StatusCode::OK);
    approx_eq(&result_vec(&body), &[0.75, 1.0, -2.0]);
}

#[tokio::test]
async fn scale_empty_vector_makes_no_calls() {
    // Empty vector -> empty result, no dependency calls, so even a dead url is fine.
    let (status, body) = vectorscale(dead_deps(), json!([]), json!(7)).await;
    assert_eq!(status, StatusCode::OK);
    approx_eq(&result_vec(&body), &[]);
}

#[tokio::test]
async fn scale_single_component() {
    let (status, body) = vectorscale(real_deps().await, json!([42]), json!(3)).await;
    assert_eq!(status, StatusCode::OK);
    approx_eq(&result_vec(&body), &[126.0]);
}

// --- Error / edge cases. ---

#[tokio::test]
async fn degraded_when_dependency_dead() {
    let (status, body) = vectorscale(dead_deps(), json!([1, 2, 3]), json!(2)).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(body["dependency"], "srvcs-floatmultiply");
}

#[tokio::test]
async fn forwards_422_from_floatmultiply() {
    let deps = Deps {
        floatmultiply_url: spawn_fixed(
            StatusCode::UNPROCESSABLE_ENTITY,
            json!({ "error": "value is not a number" }),
        )
        .await,
    };
    let (status, body) = vectorscale(deps, json!([1, "nope", 3]), json!(2)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"], "value is not a number");
}

#[tokio::test]
async fn malformed_floatmultiply_result_is_500() {
    let deps = Deps {
        floatmultiply_url: spawn_fixed(StatusCode::OK, json!({ "result": "not-a-number" })).await,
    };
    let (status, body) = vectorscale(deps, json!([1, 2, 3]), json!(2)).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(body["dependency"], "srvcs-floatmultiply");
}
