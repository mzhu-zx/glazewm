use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use tokio::sync::mpsc::{self, Receiver, Sender};
use tracing::{debug, warn};
use wm_common::{ClientResponseData, ContainerDto, WmEvent, WorkspaceDto};
use wm_ipc_client::IpcClient;

#[derive(Debug)]
pub struct WorkspaceStatus {
  pub name: String,
  display_name: Option<String>,
  pub activated: bool,
}

impl WorkspaceStatus {
  pub fn fullname(&self) -> String {
    if let Some(dn) = self.display_name.as_ref() {
      format!("{}:{}", self.name, dn)
    } else {
      self.name.clone()
    }
  }
}

// subscribe to workspace events
async fn start_watcher(tx_refresh: Sender<()>) {
  tokio::spawn(async move {
    let mut client = IpcClient::connect().await?;

    // push the first event to trigger the initial update
    tx_refresh.send(()).await?;

    let subscription_message =
      "sub -e workspace_updated workspace_activated focus_changed";
    client
      .send(subscription_message)
      .await
      .context("Failed to send subscribe command to IPC server.")?;
    let subscription_id = client
      .client_response(subscription_message)
      .await
      .and_then(|response| match response.data {
        Some(ClientResponseData::EventSubscribe(data)) => {
          Some(data.subscription_id)
        }
        _ => None,
      })
      .context("No subscription ID in watcher event subscription.")?;
    loop {
      let event_data = client
        .event_subscription(&subscription_id)
        .await
        .and_then(|event| event.data);
      match event_data {
        Some(WmEvent::WorkspaceActivated { .. })
        | Some(WmEvent::WorkspaceUpdated { .. })
        | Some(WmEvent::FocusChanged { .. }) => {
          tx_refresh.send(()).await?
        }
        _ => {
          warn!("unexpected event: {:?}", event_data);
          continue;
        }
      }
    }
    anyhow::Ok(())
  });
}

async fn start_interpreter(
  mut rx_refresh: Receiver<()>,
  tx_cmd: Sender<MinibarCommand>,
) {
  tokio::spawn(async move {
    // let mut client = IpcClient::connect().await?;
    loop {
      let () = rx_refresh
        .recv()
        .await
        .context("refresh channel is closed?")?;
      while let Ok(_) = rx_refresh.try_recv() {} // eat repetitive events
      debug!("fresh request received, dispatch to interpreter");
      tx_cmd.send(MinibarCommand::GetWorkspaces).await?;
    }
    anyhow::Ok(())
  });
}

#[derive(Debug)]
pub enum MinibarCommand {
  Raw(String),
  GetWorkspaces,
}

async fn start_cli(
  mut rx_cmd: Receiver<MinibarCommand>,
  tx_repaint: Sender<()>,
  shared_workspaces: Arc<Mutex<Vec<WorkspaceDto>>>,
) {
  tokio::spawn(async move {
    let mut client = IpcClient::connect().await?;
    loop {
      let cmd = rx_cmd.recv().await.context("cli closed")?;
      debug!("new command: {:?}", cmd);
      match cmd {
        MinibarCommand::Raw(cmd) => {
          client.send(&cmd).await.context("fail to send")?;
          if let Some(ret) = client.client_response(&cmd).await {
            debug!("{}: {:?}", cmd, ret);
          }
        }
        MinibarCommand::GetWorkspaces => {
          debug!("begin query workspace");
          let workspaces = query_workspace(&mut client).await?;
          // dbg!("new workspaces: {:?}", &workspaces);
          *(shared_workspaces.lock().unwrap()) = workspaces;
          debug!("workspace has been updated, send repaint signal.");
          tx_repaint.send(()).await?;
        }
      }
    }
    anyhow::Ok(())
  });
}

async fn query_workspace(
  client: &mut IpcClient,
) -> Result<Vec<WorkspaceDto>> {
  let query_message = "query workspaces";

  client
    .send(query_message)
    .await
    .context("Failed to send workspace query command.")?;

  client
    .client_response(query_message)
    .await
    .and_then(|response| match response.data {
      Some(ClientResponseData::Workspaces(data)) => Some(data),
      _ => None,
    })
    .map(|data| {
      data
        .workspaces
        .into_iter()
        .filter_map(|container| match container {
          ContainerDto::Workspace(ws) => Some(ws),
          _ => None,
        })
        .collect::<Vec<_>>()
    })
    .context("Invalid data in workspace query response.")
}

pub struct GlazeWmService {
  tx_cmd: mpsc::Sender<MinibarCommand>,
  rx_repaint: Option<mpsc::Receiver<()>>,
  workspaces: Arc<Mutex<Vec<WorkspaceDto>>>,
}

impl GlazeWmService {
  pub async fn start() -> GlazeWmService {
    let workspaces = Arc::new(Mutex::new(vec![]));
    let (tx_cmd, rx_cmd) = mpsc::channel::<MinibarCommand>(32);
    let (tx_refresh, rx_refresh) = mpsc::channel::<()>(32);
    let (tx_repaint, rx_repaint) = mpsc::channel::<()>(32);
    start_cli(rx_cmd, tx_repaint.clone(), workspaces.clone()).await;
    start_watcher(tx_refresh).await;
    start_interpreter(rx_refresh, tx_cmd.clone()).await;
    GlazeWmService {
      tx_cmd,
      rx_repaint: Some(rx_repaint),
      workspaces,
    }
  }
  pub fn current_workspaces(&self) -> Vec<WorkspaceStatus> {
    self
      .workspaces
      .clone()
      .lock()
      .unwrap()
      .iter()
      .map(|ws| WorkspaceStatus {
        name: ws.name.clone(),
        display_name: ws.display_name.clone(),
        activated: ws.is_displayed,
      })
      .collect()
  }
  pub fn send_command(&mut self, cmd: String) -> Result<()> {
    self.tx_cmd.blocking_send(MinibarCommand::Raw(cmd))?;
    Ok(())
  }

  pub fn subscribe_repaint(&mut self) -> Option<mpsc::Receiver<()>> {
    self.rx_repaint.take()
  }
}
