// This code is borrowed from `agent-rs` crate. We cannot get is a dependency because `agent-rs`
// cannot be compiled to wasm32.
//
// This module requires `agent` feature to be enabled on `ic-helpers` dependency.

use crate::management::CallSignature;
use candid::{CandidType, Decode};
use garcon::Waiter;
use ic_agent::agent::{PollResult, Replied, RequestStatusResponse};
use ic_agent::{Agent, AgentError, RequestId};
use serde::Deserialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AgentErrorExt {
    #[error("Agent error: {0}")]
    AgentError(#[from] ic_agent::AgentError),
    #[error("Deserialization error: {0}")]
    DeserializationError(#[from] candid::Error),
}

pub async fn proxy_update<T>(agent: &Agent, call: CallSignature) -> Result<T, AgentErrorExt>
where
    for<'a> T: CandidType + Deserialize<'a>,
{
    let waiter = garcon::Delay::builder()
        .throttle(std::time::Duration::from_millis(500))
        .timeout(std::time::Duration::from_secs(60 * 5))
        .build();
    let result_bytes = proxy_update_raw(agent, call, waiter).await?;
    let result = Decode!(&result_bytes, T)?;

    Ok(result)
}

async fn proxy_update_raw<W: Waiter>(
    agent: &Agent,
    call: CallSignature,
    mut waiter: W,
) -> Result<Vec<u8>, AgentError> {
    let request_id = agent
        .update_signed(call.recipient, call.content.clone())
        .await?;

    waiter.start();
    let mut request_accepted = false;
    loop {
        match get_poll_result(
            agent
                .request_status_signed(
                    &request_id,
                    call.recipient,
                    call.status_request_content.clone(),
                    false,
                )
                .await?,
            request_id,
        )? {
            PollResult::Submitted => {}
            PollResult::Accepted => {
                if !request_accepted {
                    // The system will return RequestStatusResponse::Unknown
                    // (PollResult::Submitted) until the request is accepted
                    // and we generally cannot know how long that will take.
                    // State transitions between Received and Processing may be
                    // instantaneous. Therefore, once we know the request is accepted,
                    // we should restart the waiter so the request does not time out.

                    waiter
                        .restart()
                        .map_err(|_| AgentError::WaiterRestartError())?;
                    request_accepted = true;
                }
            }
            PollResult::Completed(result) => return Ok(result),
        };

        waiter
            .async_wait()
            .await
            .map_err(|_| AgentError::TimeoutWaitingForResponse())?;
    }
}

fn get_poll_result(
    status_response: RequestStatusResponse,
    request_id: RequestId,
) -> Result<PollResult, AgentError> {
    match status_response {
        RequestStatusResponse::Unknown => Ok(PollResult::Submitted),

        RequestStatusResponse::Received | RequestStatusResponse::Processing => {
            Ok(PollResult::Accepted)
        }

        RequestStatusResponse::Replied {
            reply: Replied::CallReplied(arg),
        } => Ok(PollResult::Completed(arg)),

        RequestStatusResponse::Rejected {
            reject_code,
            reject_message,
        } => Err(AgentError::ReplicaError {
            reject_code,
            reject_message,
        }),
        RequestStatusResponse::Done => Err(AgentError::RequestStatusDoneNoReply(String::from(
            request_id,
        ))),
    }
}
