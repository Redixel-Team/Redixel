//! Per-handle decode state shared by the native and web backends.

use redixel_core::{ClipState, SoundId};

/// Every clip a backend was asked to load, indexed by [`SoundId`].
///
/// Handles are issued sequentially, but not every handle reaches a backend —
/// `load_sound_file` hands one out without bytes when the read fails — so a
/// slot skipped over like that is `Failed`, never `Pending`.
pub(crate) struct ClipRegistry<T> {
    slots: Vec<Slot<T>>,
}

enum Slot<T> {
    Pending,
    Ready(T),
    Failed,
}

impl<T> Default for ClipRegistry<T> {
    fn default() -> Self {
        Self { slots: Vec::new() }
    }
}

impl<T> ClipRegistry<T> {
    /// Marks `id` as decoding.
    pub(crate) fn begin(&mut self, id: SoundId) {
        self.set(id, Slot::Pending);
    }

    /// Records the outcome of decoding `id`: the decoded clip, or `None` if
    /// decoding failed.
    pub(crate) fn finish(&mut self, id: SoundId, clip: Option<T>) {
        self.set(id, clip.map_or(Slot::Failed, Slot::Ready));
    }

    /// The loading state of `id`.
    pub(crate) fn state(&self, id: SoundId) -> ClipState {
        match self.slots.get(id.index() as usize) {
            Some(Slot::Pending) => ClipState::Pending,
            Some(Slot::Ready(..)) => ClipState::Ready,
            Some(Slot::Failed) | None => ClipState::Failed,
        }
    }

    /// The decoded clip for `id`, if it is ready.
    pub(crate) fn get(&self, id: SoundId) -> Option<&T> {
        match self.slots.get(id.index() as usize) {
            Some(Slot::Ready(clip)) => Some(clip),
            _ => None,
        }
    }

    fn set(&mut self, id: SoundId, slot: Slot<T>) {
        let index: usize = id.index() as usize;
        if index >= self.slots.len() {
            self.slots.resize_with(index + 1, || Slot::Failed);
        }
        self.slots[index] = slot;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_id_never_loaded_is_failed() {
        let registry: ClipRegistry<u8> = ClipRegistry::default();
        assert_eq!(registry.state(SoundId::new(3)), ClipState::Failed);
    }

    #[test]
    fn a_clip_is_pending_until_it_finishes() {
        let mut registry: ClipRegistry<u8> = ClipRegistry::default();
        registry.begin(SoundId::new(0));

        assert_eq!(registry.state(SoundId::new(0)), ClipState::Pending);
        assert_eq!(registry.get(SoundId::new(0)), None);

        registry.finish(SoundId::new(0), Some(7));

        assert_eq!(registry.state(SoundId::new(0)), ClipState::Ready);
        assert_eq!(registry.get(SoundId::new(0)), Some(&7));
    }

    #[test]
    fn a_failed_decode_is_failed() {
        let mut registry: ClipRegistry<u8> = ClipRegistry::default();
        registry.begin(SoundId::new(0));
        registry.finish(SoundId::new(0), None);

        assert_eq!(registry.state(SoundId::new(0)), ClipState::Failed);
    }

    #[test]
    fn ids_skipped_over_are_failed_not_pending() {
        let mut registry: ClipRegistry<u8> = ClipRegistry::default();
        registry.begin(SoundId::new(2));

        assert_eq!(
            registry.state(SoundId::new(1)),
            ClipState::Failed,
            "a handle whose bytes never reached the backend must not wait forever"
        );
        assert_eq!(registry.state(SoundId::new(2)), ClipState::Pending);
    }
}
