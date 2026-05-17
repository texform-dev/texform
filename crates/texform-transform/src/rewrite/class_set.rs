//! Bitset of rewrite rule classes. Const-friendly and runtime-mutable.

use super::rule::RuleClass;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct RuleClassSet(u8);

impl RuleClassSet {
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

    pub const fn contains(self, class: RuleClass) -> bool {
        let bit = match class {
            RuleClass::Standard => 1 << 0,
            RuleClass::Expand => 1 << 1,
            RuleClass::Drop => 1 << 2,
            RuleClass::Equiv => 1 << 3,
        };
        self.0 & bit != 0
    }

    pub fn iter(self) -> impl Iterator<Item = RuleClass> {
        const ORDER: [RuleClass; 4] = [
            RuleClass::Standard,
            RuleClass::Expand,
            RuleClass::Drop,
            RuleClass::Equiv,
        ];
        ORDER.into_iter().filter(move |class| self.contains(*class))
    }
}

impl Default for RuleClassSet {
    fn default() -> Self {
        Self::empty()
    }
}

impl std::ops::BitOr for RuleClassSet {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self {
        self.union(rhs)
    }
}

impl std::ops::BitOrAssign for RuleClassSet {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl From<RuleClass> for RuleClassSet {
    fn from(class: RuleClass) -> Self {
        match class {
            RuleClass::Standard => Self::STANDARD,
            RuleClass::Expand => Self::EXPAND,
            RuleClass::Drop => Self::DROP,
            RuleClass::Equiv => Self::EQUIV,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn union_combines_bits() {
        let set = RuleClassSet::STANDARD | RuleClassSet::EXPAND;
        assert!(set.contains(RuleClass::Standard));
        assert!(set.contains(RuleClass::Expand));
        assert!(!set.contains(RuleClass::Drop));
    }

    #[test]
    fn iter_emits_in_canonical_order() {
        let set = RuleClassSet::DROP | RuleClassSet::STANDARD;
        assert_eq!(
            set.iter().collect::<Vec<_>>(),
            vec![RuleClass::Standard, RuleClass::Drop]
        );
    }

    #[test]
    fn all_preset_contains_every_class() {
        for class in [
            RuleClass::Standard,
            RuleClass::Expand,
            RuleClass::Drop,
            RuleClass::Equiv,
        ] {
            assert!(RuleClassSet::all().contains(class));
        }
    }
}
