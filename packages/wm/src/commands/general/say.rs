use crate::wm_state::WmState;

/// Broadcasts a single word to all active `hear` subscribers.
///
/// The word is dropped silently when there are no subscribers.
pub fn say(word: String, state: &WmState) {
  let _ = state.word_tx.send(word);
}
