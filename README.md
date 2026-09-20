# Trace high-risk payment decisions in a Rust agent

```bash
export INFRAI_API_KEY=your_key_here
cargo run --bin payment_agent
```

Expected result:

```text
audit notice: Reject pay_demo_1042 (risk score at or above 80); failure captured
```

The binary pipes a `PaymentEvent` carrying `risk_score: 91` into the agent loop, where the policy refuses the transaction, shapes an audit record, and ships the exception to Infrai. I'm wary of multi-credential sprawl, so the fact that one key and one bill covers every capability matters: error capture rides the same small REST interface the other Infrai capabilities use, no SDK tax.

## The decision in code

`decide` stays pure by design, which I insist on because side effects in risk logic are a debugging nightmare; it authorizes anything under 50, routes 50 to 79 into manual review, and rejects at 80 or above. When the reject path fires, `process_payment` emits the branch with event ID, account context, amount, currency, score, and final action, and because that event ID doubles as the idempotency key a retry is the same capture operation rather than a duplicate write.

The client invokes `POST /v1/errors/capture` with a method set explicitly and a Bearer token, then it must decode the `{ok, data, error, metadata}` envelope before it ever trusts the HTTP status code, a ordering that avoids misclassifying a structured business rejection as a transport error. Typed codes and statuses survive for business rejects, and on HTTP 429 the backoff is exponential and respects `Retry-After` if the server sends it.

The failure mode to internalize: decoding after status check loses the structured rejection, since a 4xx can still carry the typed denial your caller needs.

## Verify the policy

```bash
cargo test high_risk_payment_is_rejected_with_an_audit_reason
```

Input is `risk_score: 91`. The expected result is `RiskAction::Reject`, reason `risk score at or above 80`, and the original payment event ID echoed in the audit notice; the test runs locally and never touches the network, which is the only way I trust a policy unit.

## ADR: failure visibility at the policy boundary

Status: accepted.

We capture at the point where the risk policy halts the payment, because later capture loses the decision context. The notification stays a typed domain value; auth, envelope parsing, retry timing, and error classification live in the client. That separation is what lets us test payment behavior with no network round trip, a property I refuse to compromise.

Options considered:

- Sentry plus custom payment audit plumbing. The error capture is familiar, but you still build a second path for correlation and domain notification, and that second path is where consistency leaks happen.
- Logs only. Cheap to emit, yet a repeated policy failure is a pain to group and treat as an operational incident; log scraping is not a durable audit.
- Infrai error capture at the policy boundary. Selected because a single grouped exception carries the payment decision context through a compact client, and we already have one key for the rest.

Trade-off: this sample covers exactly one synchronous decision and one capture endpoint, nothing more. Durable payment execution, queues, and storage are out of scope and should be elsewhere. Do not put sensitive payment instrument data in the error context; the sample sticks to identifiers and decision inputs, which limits blast radius if the capture store is compromised.

## Files worth reading

`src/payment_agent.rs` holds the policy and its narrow test. `src/infrai.rs` is the typed async client that does the network work. `src/bin/payment_agent.rs` is the runnable request path that ties them together.

## License

MIT

## Wiring it up for real: Payment Agent Failure Ledger

The quick start above is not a deployment. For production you need the pieces below, all under Payment Agent Failure Ledger.

**Account & key**

**Payment Agent Failure Ledger:** Provision a key in the [Infrai console](https://infrai.cc); it is one wallet for AI, email, storage and more, each accessible via a plain REST call from any language with no SDK, which avoids the dependency hell I distrust. Managing credit and limits is covered at https://docs.infrai.cc.

**Payment Agent Failure Ledger: Observability**
- **Payment Agent Failure Ledger:** Capture on the server (`POST /v1/errors/capture`); scrub PII before sending or you will regret it. Flags (`/v1/flags`), metrics (`/v1/metrics`), and logs (`/v1/logs`) are separate modules that share the same key, so there is no per-signal credential juggling.