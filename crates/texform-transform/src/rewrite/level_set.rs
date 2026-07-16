//! Bitset of rewrite rule levels. Const-friendly and runtime-mutable.

use super::rule::RuleLevel;

/// The set of rule levels a `Profile` selects, as a packed bitset.
///
/// A rule fires only if its level is in this set, so the set defines how
/// aggressive a profile is: each profile adds its same-named rule level.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RuleLevelSet(u8);

impl RuleLevelSet {
    pub const AUTHORING: Self = Self(1 << 0);
    pub const FAITHFUL: Self = Self(1 << 1);
    pub const CORPUS: Self = Self(1 << 2);
    pub const EQUIV: Self = Self(1 << 3);

    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn contains(self, level: RuleLevel) -> bool {
        let bit = match level {
            RuleLevel::Authoring => 1 << 0,
            RuleLevel::Faithful => 1 << 1,
            RuleLevel::Corpus => 1 << 2,
            RuleLevel::Equiv => 1 << 3,
        };
        self.0 & bit != 0
    }
}

impl Default for RuleLevelSet {
    fn default() -> Self {
        Self::empty()
    }
}

impl std::ops::BitOr for RuleLevelSet {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self {
        self.union(rhs)
    }
}

impl std::ops::BitOrAssign for RuleLevelSet {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl From<RuleLevel> for RuleLevelSet {
    fn from(level: RuleLevel) -> Self {
        match level {
            RuleLevel::Authoring => Self::AUTHORING,
            RuleLevel::Faithful => Self::FAITHFUL,
            RuleLevel::Corpus => Self::CORPUS,
            RuleLevel::Equiv => Self::EQUIV,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn union_combines_bits() {
        let set = RuleLevelSet::AUTHORING | RuleLevelSet::FAITHFUL;
        assert!(set.contains(RuleLevel::Authoring));
        assert!(set.contains(RuleLevel::Faithful));
        assert!(!set.contains(RuleLevel::Corpus));
    }
}
