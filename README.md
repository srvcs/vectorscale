# srvcs-vectorscale

The vector-scaling orchestrator of the srvcs.cloud distributed standard library.

Its single concern: **vectors: scale by a scalar.** It owns the *control flow* —
iterating over a vector's components — but does no arithmetic of its own. For
each component it asks
[`srvcs-floatmultiply`](https://github.com/srvcs/floatmultiply) to multiply that
component by the scalar, and collects the results into a new vector.

```
vectorscale(vector, scalar):
    result = []
    for c in vector:
        result.push(floatmultiply(c, scalar))
    return result
```

The result is a JSON array of `f64` numbers, which may be fractional. For
example `vectorscale([1, 2, 3], 2) == [2.0, 4.0, 6.0]`.

Validation is not handled here. This service never calls `srvcs-isnumber`
directly; instead its dependency validates its own operands, and any `422` it
raises is forwarded verbatim.

## API

| Method | Path | Purpose |
| --- | --- | --- |
| `GET` | `/` | Service identity, concern, and dependency list |
| `POST` | `/` | Scale `vector` by `scalar`, component by component |
| `GET` | `/healthz` `/readyz` `/metrics` `/openapi.json` | srvcs service standard surface |

```sh
curl -s -X POST localhost:8080/ -H 'content-type: application/json' \
  -d '{"vector": [1, 2, 3], "scalar": 2}'
# {"vector":[1,2,3],"scalar":2,"result":[2.0,4.0,6.0]}
```

Responses:

- `200 {"vector": [..], "scalar": n, "result": [..]}` — evaluated; `result` is
  a JSON array of floats.
- `422` — the dependency rejected a component (forwarded verbatim).
- `500` — a reachable dependency returned a `200` without a numeric `result`
  (a contract violation).
- `503` — the dependency is unavailable.

## Dependencies

- [`srvcs-floatmultiply`](https://github.com/srvcs/floatmultiply)

## Configuration

| Variable | Default | Purpose |
| --- | --- | --- |
| `SRVCS_BIND_ADDR` | `0.0.0.0:8080` | Bind address |
| `SRVCS_FLOATMULTIPLY_URL` | `http://127.0.0.1:8081` | Base URL of `srvcs-floatmultiply` |
| `SRVCS_ENV` | `development` | Environment label for logs |
| `RUST_LOG` | `info,tower_http=info` | Tracing filter |

## Local checks

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Orchestration tests stand up *computing* mock dependency services in-process —
they read the request body and return the real `a * b`, so the composition is
genuinely exercised against the asserted cases (compared element-wise within
`1e-9`, since the result is a float). See
[`srvcs/platform`](https://github.com/srvcs/platform) for the shared standard.

> Note: the `cargoHash` in `flake.nix` is inherited from the template and must be
> refreshed with a `nix build` before the Nix gates pass.
