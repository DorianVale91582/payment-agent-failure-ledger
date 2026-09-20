use payment_agent_failure_ledger::{
    infrai::InfraiClient,
    payment_agent::{process_payment, AgentLoopError, PaymentEvent},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = InfraiClient::from_env()?;
    let event = PaymentEvent {
        event_id: "pay_demo_1042".into(),
        account_id: "acct_demo_7".into(),
        amount_minor: 125_00,
        currency: "USD".into(),
        risk_score: 91,
    };

    match process_payment(&client, &event).await {
        Ok(notice) => println!("audit notice: {:?} ({})", notice.action, notice.reason),
        Err(AgentLoopError::PolicyRejected { event_id, reason }) => {
            println!("audit notice: Reject {event_id} ({reason}); failure captured")
        }
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

