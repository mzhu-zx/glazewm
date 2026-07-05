// idea: check the current container.
// if its is a tiling, surround it with a stack.
// if it's a stack, unstack it?

use std::collections::VecDeque;

use anyhow::Context;
use wm_common::TilingDirection;

use crate::{
  commands::container::{flatten_tiling_container, resize_stack_container},
  models::{
    Container, DirectionContainer, StackContainer, TilingContainer,
    TilingWindow,
  },
  traits::{CommonGetters, TilingSizeGetters},
  user_config::UserConfig,
  wm_state::WmState,
};

pub fn toggle_stack(
  container: Container,
  state: &mut WmState,
  config: &UserConfig,
) -> anyhow::Result<()> {
  let direction_container = match container {
    Container::TilingWindow(tiling_window) => {
      toggle_window_stack(tiling_window, config)
    }
    // the rest cases are a noop.
    _ => return Ok(()),
  }?;

  state
    .pending_sync
    .queue_container_to_redraw(direction_container);

  Ok(())
}

fn toggle_window_stack(
  tiling_window: TilingWindow,
  config: &UserConfig,
) -> anyhow::Result<DirectionContainer> {
  let parent = tiling_window
    .direction_container()
    .context("No direction container.")?;

  if let DirectionContainer::Stack(stack) = &parent {
    return flatten_tiling_container(stack.as_tiling_container()?);
  }

  // Create a new split container to wrap the window.
  let stack_container = StackContainer::new(
    TilingDirection::Vertical,
    config.value.gaps.clone(),
  );

  wrap_in_stack_container(
    &stack_container,
    &parent.into(),
    &[tiling_window.into()],
  )?;

  Ok(stack_container.into())
}

pub fn wrap_in_stack_container(
  stack_container: &StackContainer,
  target_parent: &Container,
  target_children: &[TilingContainer],
) -> anyhow::Result<()> {
  let starting_index = target_children
    .iter()
    .map(CommonGetters::index)
    .min()
    .context("Failed to get starting index.")?;

  target_parent
    .borrow_children_mut()
    .insert(starting_index, stack_container.clone().into());

  let starting_focus_index = target_children
    .iter()
    .map(CommonGetters::focus_index)
    .min()
    .context("Failed to get starting focus index.")?;

  target_parent
    .borrow_child_focus_order_mut()
    .insert(starting_focus_index, stack_container.id());

  // Get the total tiling size amongst all children.
  let max_tiling_size = target_children
    .iter()
    .map(TilingSizeGetters::tiling_size)
    .sum();

  let target_children_ids = target_children
    .iter()
    .map(CommonGetters::id)
    .collect::<Vec<_>>();

  let sorted_focus_ids = target_parent
    .borrow_child_focus_order()
    .iter()
    .filter(|id| target_children_ids.contains(id))
    .copied()
    .collect::<VecDeque<_>>();

  // Set the split container's parent and tiling size.
  *stack_container.borrow_parent_mut() = Some(target_parent.clone());
  stack_container.set_tiling_size(max_tiling_size);

  // Move the children from their original parent to the split container.
  for target_child in target_children {
    *target_child.borrow_parent_mut() =
      Some(stack_container.clone().into());

    stack_container
      .borrow_children_mut()
      .push_back(target_child.clone().into());

    target_parent
      .borrow_children_mut()
      .retain(|child| child != &target_child.clone().into());

    target_parent
      .borrow_child_focus_order_mut()
      .retain(|id| id != &target_child.id());
  }

  // Add original focus order to split container.
  *stack_container.borrow_child_focus_order_mut() = sorted_focus_ids;

  // Shrink each stacked window to leave room for the header/bar offset
  // applied in `PositionGetters`. Without this the window keeps a full
  // `tiling_size` of 1.0 while still being offset downwards, overflowing
  // the parent along the stacking axis.
  resize_stack_container(stack_container);

  Ok(())
}
