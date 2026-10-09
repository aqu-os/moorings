//! How many times a slot has been reused: [`Generation`].

use core::fmt;
use core::num::NonZero;

/// A slot's generation: goes up every time the slot is reused, so a key to an
/// earlier value never matches a later one. Only [`Self::FIRST`] and
/// [`Self::next`] make one, so it can't go backwards.
///
/// `None` is [`Self::RETIRED`]: the generations ran out. Counting starts at 1
/// so that `None` can take 0's place - still 64 bits, and every use has to
/// say what happens to a retired slot.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Generation(Option<NonZero<u64>>);

impl Generation {
    /// A fresh slot's.
    pub(crate) const FIRST: Self = Self(Some(NonZero::<u64>::MIN));
    /// No generation left: the slot is retired, never reused, so no key
    /// handed out for it can ever match again. Never carried by a key.
    pub(crate) const RETIRED: Self = Self(None);

    /// The generation after this one - [`Self::RETIRED`] once they run out
    /// (and after it).
    pub(crate) const fn next(self) -> Self {
        match self.0 {
            Some(generation) => Self(generation.checked_add(1)),
            None => Self::RETIRED,
        }
    }

    /// The generation's number - `None` once the slot is out of them (see
    /// [`Self::RETIRED`]), so whoever asks has to say what happens then.
    pub(crate) const fn live(self) -> Option<NonZero<u64>> {
        self.0
    }

    /// Generation `generation` (0 being [`Self::RETIRED`]) - for tests that
    /// fast-forward a slot.
    #[cfg(test)]
    pub(crate) const fn new(generation: u64) -> Self {
        Self(NonZero::new(generation))
    }
}

// Meant to be read, as part of a key's "slot 3, generation 2".
impl fmt::Display for Generation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Some(generation) => write!(f, "{generation}"),
            None => write!(f, "retired"),
        }
    }
}

impl fmt::Debug for Generation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

#[cfg(test)]
mod tests {
    use std::string::ToString;

    use super::*;

    #[test]
    fn retired_takes_zeros_place_so_a_generation_stays_64_bits() {
        assert_eq!(size_of::<Generation>(), size_of::<u64>());
    }

    #[test]
    fn generations_only_go_up_until_they_run_out() {
        assert_eq!(Generation::FIRST, Generation::new(1));
        assert_eq!(Generation::FIRST.next(), Generation::new(2));
        let last = Generation::new(u64::MAX);
        assert_eq!(last.live(), NonZero::new(u64::MAX));
        assert_eq!(last.next(), Generation::RETIRED);
        assert_eq!(Generation::RETIRED.live(), None);
        assert_eq!(Generation::RETIRED.next(), Generation::RETIRED);
    }

    #[test]
    fn generations_display_as_their_number() {
        assert_eq!(Generation::new(2).to_string(), "2");
        assert_eq!(Generation::RETIRED.to_string(), "retired");
    }
}
