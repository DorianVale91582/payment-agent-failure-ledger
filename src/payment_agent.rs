use crate::infrai::{CaptureError, InfraiClient, InfraiError};
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentEvent {
    pub event_id: String,
    pub account_id: String,
    pub amount_minor: u64,
    pub currency: String,
    pub risk_score: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RiskAction {
    Authorize,
    ManualReview,
    Reject,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditNotice {
    pub event_id: String,
    pub action: RiskAction,
    pub reason: &'static str,
}

#[derive(Debug, Error)]
pub enum AgentLoopError {
    #[error("risk policy rejected payment {event_id}: {reason}")]
    PolicyRejected { event_id: String, reason: &'static str },
    #[error("failure capture did not complete: {0}")]
    Capture(#[from] InfraiError),
}

pub fn decide(event: &PaymentEvent) -> AuditNotice {
    let (action, reason) = match event.risk_score {
        80..=u8::MAX => (RiskAction::Reject, "risk score at or above 80"),
        50..=79 => (RiskAction::ManualReview, "risk score from 50 through 79"),
        _ => (RiskAction::Authorize, "risk score below 50"),
    };
    AuditNotice { event_id: event.event_id.clone(), action, reason }
}

pub async fn process_payment(
    client: &InfraiClient,
    event: &PaymentEvent,
) -> Result<AuditNotice, AgentLoopError> {
    let notice = decide(event);
    if notice.action == RiskAction::Reject {
        let exception = format!("PolicyRejected: {}", notice.reason);
        client.capture_error(&event.event_id, &CaptureError {
            message: "payment agent rejected a high-risk event",
            level: "warning",
            fingerprint: ["payment-agent", "risk-policy"],
            exception: &exception,
            context: json!({
                "event_id": event.event_id,
                "account_id": event.account_id,
                "amount_minor": event.amount_minor,
                "currency": event.currency,
                "risk_score": event.risk_score,
                "action": "reject"
            }),
        }).await?;
        return Err(AgentLoopError::PolicyRejected {
            event_id: event.event_id.clone(), reason: notice.reason,
        });
    }
    Ok(notice)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn high_risk_payment_is_rejected_with_an_audit_reason() {
        let event = PaymentEvent {
            event_id: "pay_1042".into(),
            account_id: "acct_7".into(),
            amount_minor: 125_00,
            currency: "USD".into(),
            risk_score: 91,
        };
        let notice = decide(&event);
        assert_eq!(notice.action, RiskAction::Reject);
        assert_eq!(notice.reason, "risk score at or above 80");
        assert_eq!(notice.event_id, "pay_1042");
    }
}

