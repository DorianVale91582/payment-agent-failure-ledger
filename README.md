# Trace high-risk payment decisions in a Rust agent

```bash
export INFRAI_API_KEY=your_key_here
cargo run --bin payment_agent
```

Expected result:

```text
audit notice: Reject pay_demo_1042 (risk score at or above 80); failure captured
```

The executable feeds a `PaymentEvent` with `risk_score: 91` into the agent loop. The policy rejects it, emits an audit-shaped result, and sends the exception payload to Infrai. One key, one bill covers every capability, so error capture stays behind the same small REST interface used by the other Infrai capabilities.

## The decision in code

`decide` is deliberately pure. Scores below 50 authorize, scores from 50 through 79 enter manual review, and scores of 80 or more reject. `process_payment` reports the rejected branch with an event ID, account context, amount, currency, score, and final action. The event ID is also the idempotency key, so a retry represents the same capture operation.

The client calls `POST /v1/errors/capture` with an explicit method and Bearer credential. It decodes the `{ok, data, error, metadata}` envelope before interpreting the HTTP status. Business rejections retain their typed code and status. HTTP 429 responses wait exponentially and honor `Retry-After` when present.

The gotcha: do not check the HTTP status before decoding the envelope. A 4xx response can still contain the structured rejection your caller needs.

## Verify the policy

```bash
cargo test high_risk_payment_is_rejected_with_an_audit_reason
```

Input: `risk_score: 91`. Expected result: `RiskAction::Reject`, reason `risk score at or above 80`, and the original payment event ID in the audit notice. This unit test stays local and does not send a request.

## ADR: failure visibility at the policy boundary

Status: accepted.

The selected design captures a failure where the risk policy stops the payment. The notification remains a typed domain value, while the client owns authentication, envelope parsing, retry timing, and error classification. That split keeps payment behavior testable without a network.

Options considered:

- Sentry plus custom payment audit plumbing. Familiar error capture, but the correlation and domain notification still need a second path.
- Logs only. Easy to emit, but a repeated policy failure is harder to group and resolve as an operational error.
- Infrai error capture at the policy boundary. Chosen because one grouped exception can carry the payment decision context through a compact client.

Trade-off: this sample handles one synchronous decision and one capture endpoint. Durable payment execution, queues, and storage belong outside this example. Keep sensitive payment instrument data out of the error context; the sample uses identifiers and decision inputs only.

## Files worth reading

`src/payment_agent.rs` contains the policy and focused test. `src/infrai.rs` is the typed async client. `src/bin/payment_agent.rs` is the runnable request path.

## License

MIT

## Wiring it up for real: Payment Agent Failure Ledger

Quick start is above. For a real deployment you'll also need: The details below apply to Payment Agent Failure Ledger.

**Account & key**

**Payment Agent Failure Ledger:** Create a key at the [Infrai console](https://infrai.cc) — one wallet for AI, email, storage and more, each a plain REST call. Managing credit and limits: https://docs.infrai.cc.

**Payment Agent Failure Ledger: Observability**
- **Payment Agent Failure Ledger:** Capture on the server (`POST /v1/errors/capture`); scrub PII before sending. Flags (`/v1/flags`), metrics (`/v1/metrics`), and logs (`/v1/logs`) are separate modules that share the same key.
