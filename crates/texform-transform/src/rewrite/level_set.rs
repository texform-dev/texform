//! Bitset of rewrite normalization levels. Const-friendly and runtime-mutable.

use super::rule::NormalizationLevel;

/// The set of normalization levels a `Profile` selects, as a packed bitset.
///
/// A rule fires only if its level is in this set, so the set defines how
/// aggressive a profile is: `Authoring` selects `Standard`, `Faithful` adds
/// `Expand`, `Corpus` adds `Drop`, and `Equiv` adds `Equiv`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NormalizationLevelSet(u8);

impl NormalizationLevelSet {
    pub const STANDARD: Self = Self(1 << 0);
    pub const EXPAND: Self = Self(1 << 1);
    pub const DROP: Self = Self(1 << 2);
    pub const EQUIV: Self = Self(1 << 3);

    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn all() -> Self {
        Self(0b1111)
    }

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    pub const fn contains(self, level: NormalizationLevel) -> bool {
        let bit = match level {
            NormalizationLevel::Standard => 1 << 0,
            NormalizationLevel::Expand => 1 << 1,
            NormalizationLevel::Drop => 1 << 2,
            NormalizationLevel::Equiv => 1 << 3,
        };
        self.0 & bit != 0
    }

    pub fn iter(self) -> impl Iterator<Item = NormalizationLevel> {
        const ORDER: [NormalizationLevel; 4] = [
            NormalizationLevel::Standard,
            NormalizationLevel::Expand,
            NormalizationLevel::Drop,
            NormalizationLevel::Equiv,
        ];
        ORDER.into_iter().filter(move |level| self.contains(*level))
    }
}

impl Default for NormalizationLevelSet {
    fn default() -> Self {
        Self::empty()
    }
}

impl std::ops::BitOr for NormalizationLevelSet {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self {
        self.union(rhs)
    }
}

impl std::ops::BitOrAssign for NormalizationLevelSet {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl From<NormalizationLevel> for NormalizationLevelSet {
    fn from(level: NormalizationLevel) -> Self {
        match level {
            NormalizationLevel::Standard => Self::STANDARD,
            NormalizationLevel::Expand => Self::EXPAND,
            NormalizationLevel::Drop => Self::DROP,
            NormalizationLevel::Equiv => Self::EQUIV,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn union_combines_bits() {
        let set = NormalizationLevelSet::STANDARD | NormalizationLevelSet::EXPAND;
        assert!(set.contains(NormalizationLevel::Standard));
        assert!(set.contains(NormalizationLevel::Expand));
        assert!(!set.contains(NormalizationLevel::Drop));
    }

    #[test]
    fn iter_emits_in_canonical_order() {
        let set = NormalizationLevelSet::DROP | NormalizationLevelSet::STANDARD;
        assert_eq!(
            set.iter().collect::<Vec<_>>(),
            vec![NormalizationLevel::Standard, NormalizationLevel::Drop]
        );
    }

    #[test]
    fn all_preset_contains_every_level() {
        for level in [
            NormalizationLevel::Standard,
            NormalizationLevel::Expand,
            NormalizationLevel::Drop,
            NormalizationLevel::Equiv,
        ] {
            assert!(NormalizationLevelSet::all().contains(level));
        }
    }
}
