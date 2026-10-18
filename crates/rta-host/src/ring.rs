//! Fixed-capacity ring buffers. Pushing past capacity overwrites the oldest entry. Nothing
//! here allocates after construction.

/// The last `N` values pushed, oldest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ring<T: Copy, const N: usize> {
    items: [T; N],
    /// Index of the oldest entry.
    head: usize,
    len: usize,
    /// Entries overwritten since construction.
    dropped: u64,
}

impl<T: Copy, const N: usize> Ring<T, N> {
    /// `fill` initializes the slots and is never returned by `iter`.
    pub const fn new(fill: T) -> Self {
        Self {
            items: [fill; N],
            head: 0,
            len: 0,
            dropped: 0,
        }
    }

    pub fn push(&mut self, value: T) {
        if N == 0 {
            self.dropped = self.dropped.saturating_add(1);
            return;
        }
        let at = (self.head + self.len) % N;
        if let Some(slot) = self.items.get_mut(at) {
            *slot = value;
        }
        if self.len < N {
            self.len += 1;
        } else {
            self.head = (self.head + 1) % N;
            self.dropped = self.dropped.saturating_add(1);
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub const fn capacity(&self) -> usize {
        N
    }

    /// Entries overwritten because the ring was full.
    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    /// Oldest first.
    pub fn iter(&self) -> impl Iterator<Item = &T> + '_ {
        (0..self.len).filter_map(move |offset| self.items.get((self.head + offset) % N))
    }

    pub fn last(&self) -> Option<&T> {
        self.len
            .checked_sub(1)
            .and_then(|offset| self.items.get((self.head + offset) % N))
    }
}

impl<T: Copy + Default, const N: usize> Default for Ring<T, N> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

/// Bytes kept from one status text. MAVLink `STATUSTEXT.text` is 50 bytes.
pub const STATUS_TEXT_BYTES: usize = 50;

/// One status text, truncated to [`STATUS_TEXT_BYTES`] and stored inline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatusText {
    bytes: [u8; STATUS_TEXT_BYTES],
    len: usize,
}

impl Default for StatusText {
    fn default() -> Self {
        Self::EMPTY
    }
}

impl StatusText {
    pub const EMPTY: Self = Self {
        bytes: [0; STATUS_TEXT_BYTES],
        len: 0,
    };

    /// Copy at most [`STATUS_TEXT_BYTES`] bytes. A cut that splits a UTF-8 character drops
    /// the partial character.
    pub fn new(text: &[u8]) -> Self {
        let mut len = text.len().min(STATUS_TEXT_BYTES);
        while len > 0 && std::str::from_utf8(text.get(..len).unwrap_or_default()).is_err() {
            len -= 1;
        }
        let mut bytes = [0; STATUS_TEXT_BYTES];
        if let (Some(to), Some(from)) = (bytes.get_mut(..len), text.get(..len)) {
            to.copy_from_slice(from);
        }
        Self { bytes, len }
    }

    /// The stored text. Invalid UTF-8 from the wire is shown as `""`.
    pub fn as_str(&self) -> &str {
        std::str::from_utf8(self.bytes.get(..self.len).unwrap_or_default()).unwrap_or_default()
    }
}

/// Status texts kept per vehicle state.
pub const STATUS_LOG_CAPACITY: usize = 16;

/// The last [`STATUS_LOG_CAPACITY`] status texts.
pub type StatusLog = Ring<StatusText, STATUS_LOG_CAPACITY>;

pub const fn status_log() -> StatusLog {
    Ring::new(StatusText::EMPTY)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_full_ring_overwrites_the_oldest_and_counts_it() {
        let mut ring: Ring<u32, 3> = Ring::new(0);
        assert!(ring.is_empty());
        for value in 1..=5 {
            ring.push(value);
        }
        assert_eq!(ring.len(), 3);
        assert_eq!(ring.iter().copied().collect::<Vec<_>>(), [3, 4, 5]);
        assert_eq!(ring.last(), Some(&5));
        assert_eq!(ring.dropped(), 2);
        assert_eq!(ring.capacity(), 3);
    }

    #[test]
    fn a_long_status_text_is_cut_on_a_character_boundary() {
        let long = "é".repeat(40);
        let text = StatusText::new(long.as_bytes());
        assert_eq!(text.as_str(), "é".repeat(25));
        assert_eq!(StatusText::new(b"gps fail").as_str(), "gps fail");
        assert_eq!(StatusText::new(&[0xff, 0xfe]).as_str(), "");
    }

    #[test]
    fn a_status_flood_stays_at_capacity() {
        let mut log = status_log();
        for _ in 0..10_000 {
            log.push(StatusText::new(b"flood"));
        }
        assert_eq!(log.len(), STATUS_LOG_CAPACITY);
        assert_eq!(log.dropped(), 10_000 - STATUS_LOG_CAPACITY as u64);
    }
}
