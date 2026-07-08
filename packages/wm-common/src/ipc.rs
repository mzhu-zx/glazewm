use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{BindingModeConfig, ContainerDto, TilingDirection, WmEvent};

pub const DEFAULT_IPC_PORT: u32 = 6123;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "messageType", rename_all = "snake_case")]
pub enum ServerMessage {
  ClientResponse(ClientResponseMessage),
  EventSubscription(EventSubscriptionMessage),
  HearBroadcast(HearBroadcastMessage),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientResponseMessage {
  pub client_message: String,
  pub data: Option<ClientResponseData>,
  pub error: Option<String>,
  pub success: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ClientResponseData {
  AppMetadata(AppMetadataData),
  BindingModes(BindingModesData),
  Command(CommandData),
  EventSubscribe(EventSubscribeData),
  EventUnsubscribe,
  Focused(FocusedData),
  Say,
  Monitors(MonitorsData),
  TilingDirection(TilingDirectionData),
  Windows(WindowsData),
  Workspaces(WorkspacesData),
  Paused(bool),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppMetadataData {
  pub version: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BindingModesData {
  pub binding_modes: Vec<BindingModeConfig>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandData {
  pub subject_container_id: Uuid,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventSubscribeData {
  pub subscription_id: Uuid,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusedData {
  pub focused: ContainerDto,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorsData {
  pub monitors: Vec<ContainerDto>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TilingDirectionData {
  pub tiling_direction: TilingDirection,
  pub direction_container: ContainerDto,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowsData {
  pub windows: Vec<ContainerDto>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspacesData {
  pub workspaces: Vec<ContainerDto>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventSubscriptionMessage {
  pub data: Option<WmEvent>,
  pub error: Option<String>,
  pub subscription_id: Uuid,
  pub success: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HearBroadcastMessage {
  pub subscription_id: Uuid,
  pub word: String,
}

#[cfg(test)]
mod tests {
  use super::*;

  /// A `HearBroadcast` server message is tagged distinctly and round-trips
  /// so the CLI can route it apart from event subscriptions.
  #[test]
  fn hear_broadcast_round_trip() {
    let subscription_id = Uuid::new_v4();
    let message = ServerMessage::HearBroadcast(HearBroadcastMessage {
      subscription_id,
      word: "hello".to_string(),
    });

    let json = serde_json::to_string(&message).unwrap();
    assert!(json.contains(r#""messageType":"hear_broadcast""#));
    assert!(json.contains(r#""word":"hello""#));

    let decoded = serde_json::from_str::<ServerMessage>(&json).unwrap();
    assert!(matches!(
      decoded,
      ServerMessage::HearBroadcast(msg)
        if msg.subscription_id == subscription_id && msg.word == "hello"
    ));
  }

  /// The `Say` acknowledgement carries no payload and reports success.
  #[test]
  fn say_ack_serializes_to_null_data() {
    let message = ServerMessage::ClientResponse(ClientResponseMessage {
      client_message: "say hello".to_string(),
      data: Some(ClientResponseData::Say),
      error: None,
      success: true,
    });

    let json = serde_json::to_string(&message).unwrap();
    assert!(json.contains(r#""data":null"#));
    assert!(json.contains(r#""success":true"#));
  }
}
