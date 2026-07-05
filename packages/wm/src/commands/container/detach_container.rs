use anyhow::Context;

use crate::{
  commands::container::{flatten_tiling_container, resize_stack_container},
  models::Container,
  traits::{CommonGetters, TilingSizeGetters, MIN_TILING_SIZE},
};

/// Removes a container from the tree.
///
/// If the container is a tiling container, the siblings will be resized to
/// fill the freed up space. Will flatten empty parent split containers.
#[allow(clippy::needless_pass_by_value)]
pub fn detach_container(child_to_remove: Container) -> anyhow::Result<()> {
  // Flatten the parent split container if it'll be empty after removing
  // the child.
  if let Some(split_parent) = child_to_remove
    .parent()
    .and_then(|parent| parent.as_tiling_container().ok())
  {
    if split_parent.child_count() == 1 {
      flatten_tiling_container(split_parent)?;
    }
  }

  let parent = child_to_remove.parent().context("No parent.")?;

  parent
    .borrow_children_mut()
    .retain(|c| c.id() != child_to_remove.id());

  parent
    .borrow_child_focus_order_mut()
    .retain(|id| *id != child_to_remove.id());

  *child_to_remove.borrow_parent_mut() = None;

  // A stack sizes every child as `1.0 - STACK_HEADER_SIZE * n`, so removing
  // a child requires re-sizing the survivors for the new count rather than
  // redistributing the freed space proportionally. Skipping this leaves the
  // remaining windows oversized while still offset by the header, causing
  // vertical overflow.
  if let Some(stack) = parent.as_stack() {
    resize_stack_container(stack);
    return Ok(());
  }

  // Resize the siblings if it is a tiling container.
  if let Ok(child_to_remove) = child_to_remove.as_tiling_container() {
    let tiling_siblings = parent.tiling_children().collect::<Vec<_>>();

    // TODO: Share logic with `resize_tiling_container`.
    let available_size =
      tiling_siblings.iter().fold(0.0, |sum, container| {
        sum + container.tiling_size() - MIN_TILING_SIZE
      });

    // Adjust size of the siblings based on the freed up space.
    for sibling in &tiling_siblings {
      let resize_factor =
        (sibling.tiling_size() - MIN_TILING_SIZE) / available_size;

      let size_delta = resize_factor * child_to_remove.tiling_size();
      sibling.set_tiling_size(sibling.tiling_size() + size_delta);
    }
  }

  Ok(())
}
