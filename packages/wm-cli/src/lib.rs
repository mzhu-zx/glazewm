#![warn(clippy::all, clippy::pedantic)]
#![allow(clippy::missing_errors_doc)]

use anyhow::Context;
use wm_common::{ClientResponseData, ServerMessage};
use wm_ipc_client::IpcClient;

pub async fn start(args: Vec<String>) -> anyhow::Result<()> {
  let mut client = IpcClient::connect().await?;

  let message = args[1..].join(" ");
  client
    .send(&message)
    .await
    .context("Failed to send command to IPC server.")?;

  let client_response = client
    .client_response(&message)
    .await
    .context("Failed to receive response from IPC server.")?;

  match client_response.data {
    // For event (`sub`) and word (`hear`) subscriptions, omit the initial
    // response message and continuously output subsequent messages. Both
    // share the `EventSubscribe` acknowledgement, so the incoming stream is
    // matched by message type rather than by the acknowledgement.
    Some(ClientResponseData::EventSubscribe(data)) => loop {
      match client
        .next_message()
        .await
        .context("Failed to receive response from IPC server.")?
      {
        ServerMessage::EventSubscription(msg)
          if msg.subscription_id == data.subscription_id =>
        {
          println!("{}", serde_json::to_string(&msg)?);
        }
        ServerMessage::HearBroadcast(msg)
          if msg.subscription_id == data.subscription_id =>
        {
          println!("{}", serde_json::to_string(&msg)?);
        }
        _ => {}
      }
    },
    // For all other messages, output and exit when the first response
    // message is received.
    _ => {
      println!("{}", serde_json::to_string(&client_response)?);
    }
  }

  Ok(())
}
