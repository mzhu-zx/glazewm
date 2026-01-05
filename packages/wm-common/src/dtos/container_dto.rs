use serde::{Deserialize, Serialize};

use super::{
  MonitorDto, RootContainerDto, SplitContainerDto, WindowDto, WorkspaceDto,
};
use crate::StackContainerDto;

/// User-friendly representation of a container.
///
/// Used for IPC and debug logging.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContainerDto {
  Root(RootContainerDto),
  Monitor(MonitorDto),
  Workspace(WorkspaceDto),
  Split(SplitContainerDto),
  Stack(StackContainerDto),
  Window(WindowDto),
}
