use std::{
  sync::{Arc, Mutex},
  time::Duration,
};

use anyhow::{Context, Result};
use tokio::{
  sync::mpsc::{self, Receiver, Sender},
  task::JoinHandle,
};
use tracing::{debug, warn};
use uuid::Uuid;
use wm_common::{
  ClientResponseData, ContainerDto, MonitorDto, StackContainerDto,
  WmEvent, WorkspaceDto,
};
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

#[derive(Debug)]
pub struct WindowStatus {
  pub title: String,
  pub activated: bool,
  pub id: Uuid,
}

// subscribe to workspace events
async fn start_watcher(ctx: ServiceContext) -> JoinHandle<()> {
  tokio::spawn(async move { watcher_main(ctx).await.unwrap() })
}

async fn watcher_main(
  ServiceContext {
    tx_refresh,
    tx_repaint,
    stack,
    device_name,
    ..
  }: ServiceContext,
) -> Result<()> {
  loop {
    let Ok(mut client) = IpcClient::connect().await else {
      warn!("cannot connect, retry in 5 seconds");
      tokio::time::sleep(Duration::from_secs(5)).await;
      continue;
    };

    // push the first event to trigger the initial update
    tx_refresh.send(()).await.context("tx_refresh")?;

    let subscription_message = "sub -e workspace_updated workspace_activated focus_changed stack_focus_changed";
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
        | Some(WmEvent::WorkspaceUpdated { .. }) => {
          tx_refresh.send(()).await?
        }
        Some(WmEvent::FocusChanged { focused_container }) => {
          if let ContainerDto::Window(w) = focused_container {
            let mut rec = stack.lock().unwrap();
            if rec.id != w.parent_id {
              debug!("{:?} != {:?}", rec.id, w.parent_id);
              rec.clear();
            }
          }
          tx_refresh.send(()).await?
        }
        Some(WmEvent::StackFocusChanged {
          stack_container:
            ContainerDto::Stack(StackContainerDto {
              id,
              children,
              device_name: Some(d),
              ..
            }),
        }) if d == device_name => {
          debug!("stack focused!");
          {
            *stack.lock().unwrap() = StackRecord {
              id: Some(id),
              children,
            }
          }
          tx_repaint.send(()).await?
        }
        None => {
          warn!("watcher ipc closed. try reconnect ...");
          break;
        }
        _ => {
          warn!("unexpected event: {:?}", event_data);
          continue;
        }
      }
    }
  }
}

async fn start_interpreter(
  rx_refresh: Receiver<()>,
  ServiceContext { tx_cmd, .. }: ServiceContext,
) -> JoinHandle<()> {
  tokio::spawn(async move {
    interpreter_main(rx_refresh, tx_cmd).await.unwrap()
  })
}

async fn interpreter_main(
  mut rx_refresh: Receiver<()>,
  tx_cmd: Sender<MinibarCommand>,
) -> Result<()> {
  loop {
    let () = rx_refresh
      .recv()
      .await
      .context("refresh channel is closed?")?;
    while rx_refresh.try_recv().is_ok() {} // eat repetitive events
    debug!("fresh request received, dispatch to interpreter");
    tx_cmd.send(MinibarCommand::GetWorkspaces).await?;
  }
}

#[derive(Debug)]
pub enum MinibarCommand {
  Raw(String),
  GetWorkspaces,
}

async fn start_cli(
  rx_cmd: Receiver<MinibarCommand>,
  ctx: ServiceContext,
) -> JoinHandle<()> {
  tokio::spawn(async move { cli_main(rx_cmd, ctx).await.unwrap() })
}

async fn cli_main(
  mut rx_cmd: Receiver<MinibarCommand>,
  ServiceContext {
    tx_repaint,
    device_name,
    workspaces: shared_workspaces,
    monitor: shared_monitor,
    ..
  }: ServiceContext,
) -> Result<()> {
  loop {
    let Ok(mut client) = IpcClient::connect().await else {
      warn!("cannot connect, retry in 5 seconds");
      tokio::time::sleep(Duration::from_secs(5)).await;
      continue;
    };

    debug!("begin query monitor");
    let montor = match query_monitors(&mut client, &device_name).await {
      Ok(w) => w,
      Err(e) => {
        warn!("cannot send: {}", e);
        continue;
      }
    };
    // dbg!("new workspaces: {:?}", &workspaces);
    *(shared_monitor.lock().unwrap()) = montor;
    debug!("monitor has been updated");

    loop {
      let cmd =
        rx_cmd.recv().await.context("rx_cmd shouldn't be closed.")?;
      debug!("new command: {:?}", cmd);
      match cmd {
        MinibarCommand::Raw(cmd) => {
          if let Err(e) = client.send(&cmd).await.context("fail to send") {
            warn!("fail to send: {}", e);
            break;
          }
          if let Some(ret) = client.client_response(&cmd).await {
            debug!("{}: {:?}", cmd, ret);
          }
        }
        MinibarCommand::GetWorkspaces => {
          debug!("begin query workspace");
          let workspaces = match query_workspace(&mut client).await {
            Ok(w) => w,
            Err(e) => {
              warn!("cannot send: {}", e);
              break;
            }
          };
          // dbg!("new workspaces: {:?}", &workspaces);
          *(shared_workspaces.lock().unwrap()) = workspaces;
          debug!("workspace has been updated, send repaint signal.");
          tx_repaint.send(()).await?;
        }
      }
    }
  }
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

async fn query_monitors(
  client: &mut IpcClient,
  dev_name: &str,
) -> Result<Option<MonitorDto>> {
  let query_message = "query monitors";

  client
    .send(query_message)
    .await
    .context("Failed to send workspace query command.")?;

  client
    .client_response(query_message)
    .await
    .and_then(|response| match response.data {
      Some(ClientResponseData::Monitors(data)) => Some(data),
      _ => None,
    })
    .map(|data| {
      data
        .monitors
        .into_iter()
        .filter_map(|container| match container {
          ContainerDto::Monitor(mon) if mon.device_name == dev_name => {
            Some(mon)
          }
          _ => None,
        })
        .take(1)
        .collect::<Vec<MonitorDto>>()
        .pop()
    })
    .context("Invalid data in workspace query monitor.")
}

pub struct GlazeWmService {
  tx_cmd: mpsc::Sender<MinibarCommand>,
  rx_repaint: Option<mpsc::Receiver<()>>,
  workspaces: Arc<Mutex<Vec<WorkspaceDto>>>,
  monitor: Arc<Mutex<Option<MonitorDto>>>,
  stack: Arc<Mutex<StackRecord>>,
  handles: Vec<JoinHandle<()>>,
}

struct StackRecord {
  id: Option<Uuid>,
  children: Vec<ContainerDto>,
}

impl StackRecord {
  fn clear(&mut self) {
    self.id = None;
  }

  fn windows(&self) -> Option<&Vec<ContainerDto>> {
    self.id.map(|_| &self.children)
  }
}

#[derive(Clone)]
struct ServiceContext {
  tx_refresh: Sender<()>,
  tx_repaint: Sender<()>,
  tx_cmd: Sender<MinibarCommand>,
  workspaces: Arc<Mutex<Vec<WorkspaceDto>>>,
  stack: Arc<Mutex<StackRecord>>,
  monitor: Arc<Mutex<Option<MonitorDto>>>,
  device_name: String,
}

impl GlazeWmService {
  pub async fn start(device_name: String) -> GlazeWmService {
    let workspaces = Arc::new(Mutex::new(vec![]));
    let stack = Arc::new(Mutex::new(StackRecord {
      id: None,
      children: vec![],
    }));
    let monitor = Arc::new(Mutex::new(None));
    let (tx_cmd, rx_cmd) = mpsc::channel::<MinibarCommand>(32);
    let (tx_refresh, rx_refresh) = mpsc::channel::<()>(32);
    let (tx_repaint, rx_repaint) = mpsc::channel::<()>(32);

    let ctx = ServiceContext {
      tx_refresh: tx_refresh.clone(),
      tx_repaint: tx_repaint.clone(),
      tx_cmd: tx_cmd.clone(),
      workspaces: workspaces.clone(),
      stack: stack.clone(),
      device_name: device_name.clone(),
      monitor: monitor.clone(),
    };

    let cli = start_cli(rx_cmd, ctx.clone()).await;
    let watcher = start_watcher(ctx.clone()).await;
    let interpreter = start_interpreter(rx_refresh, ctx.clone()).await;

    GlazeWmService {
      tx_cmd,
      rx_repaint: Some(rx_repaint),
      workspaces,
      monitor,
      stack,
      handles: vec![cli, watcher, interpreter],
    }
  }

  pub fn stop(self) {
    for handle in self.handles {
      handle.abort();
    }
  }

  fn monitor_id(&self) -> Option<Uuid> {
    self.monitor.clone().lock().unwrap().as_ref().map(|x| x.id)
  }
  pub fn current_workspaces(&self) -> Vec<WorkspaceStatus> {
    let monitor_id = self.monitor_id();

    self
      .workspaces
      .lock()
      .unwrap()
      .iter()
      .filter(|ws| monitor_id == ws.parent_id)
      .map(|ws| WorkspaceStatus {
        name: ws.name.clone(),
        display_name: ws.display_name.clone(),
        activated: ws.is_displayed,
      })
      .collect()
  }

  pub fn current_windows(&self) -> Vec<WindowStatus> {
    fn truncate_title(src: &str) -> String {
      let mut s = (src[..src.len().min(20)]).to_string();
      if src.len() > 20 {
        for _ in 0..3 {
          s.pop();
        }
        s.push_str("...");
      }
      s
    }

    if let Some(w) = self.stack.lock().unwrap().windows() {
      w.iter()
        .filter_map(|c| match c {
          ContainerDto::Window(w) => Some(w),
          _ => None,
        })
        .map(|w| WindowStatus {
          title: truncate_title(&w.title),
          activated: w.has_focus,
          id: w.id,
        })
        .collect()
    } else {
      vec![]
    }
  }

  pub fn send_command(&mut self, cmd: String) -> Result<()> {
    self.tx_cmd.blocking_send(MinibarCommand::Raw(cmd))?;
    Ok(())
  }

  pub fn subscribe_repaint(&mut self) -> Option<mpsc::Receiver<()>> {
    self.rx_repaint.take()
  }
}
