//! Cursor pages (ADR 0044).

/// The page size: 1 to 200, default 50.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PageLimit(u32);

impl PageLimit {
    pub const DEFAULT: Self = Self(50);
    pub const MAX: u32 = 200;

    pub fn new(limit: u32) -> Option<Self> {
        (1..=Self::MAX).contains(&limit).then_some(Self(limit))
    }

    pub fn get(self) -> u32 {
        self.0
    }
}

impl Default for PageLimit {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// One page of a list. `next` is the cursor of the next page; it is absent on the last page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page<T, C> {
    pub items: Vec<T>,
    pub next: Option<C>,
}
