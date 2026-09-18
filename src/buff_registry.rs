//! Buff/debuff registry — duration-tracked stat modifiers with stacking policies.
//! Formula: ((Base + Sum(Flat)) * (10000 + Sum(Permyriad))) / 10000
//! Extends tinctures.rs TinctureBuff with generalized stat targeting.

/// Stat to target with a buff or debuff.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StatTarget {
    /// Strength.
    Str,
    /// Dexterity.
    Dex,
    /// Constitution.
    Con,
    /// Intelligence.
    Int,
    /// Vitality.
    Vit,
    /// Speed.
    Spd,
    /// Max HP.
    MaxHp,
    /// Max entropy.
    MaxEntropy,
}

/// Whether a buff enhances or impairs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuffEffect {
    /// Enhancing effect (positive modifiers).
    Buff,
    /// Impairing effect (negative modifiers).
    Debuff,
}

/// Rule for combining multiple buffs targeting the same stat.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StackingPolicy {
    /// Stack all active buff modifiers.
    Stack,
    /// Replace active buff with new one.
    Replace,
    /// Cap stacked modifiers at a limit.
    Cap,
}

/// Duration-tracked stat modifier with active duration and stacking rules.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BuffRegistry {
    /// Whether this buff enhances or impairs.
    pub effect: BuffEffect,
    /// Total duration in ticks (immutable once created).
    pub duration: u16,
    /// Ticks remaining before expiration.
    pub ticks_remaining: u16,
    /// How multiple buffs stack when targeting the same stat.
    pub stacking_policy: StackingPolicy,
    /// Which stat this buff modifies.
    pub stat_target: StatTarget,
    /// Flat bonus to apply (before permyriad).
    pub flat_bonus: i32,
    /// Permyriad bonus to apply (multiplicative, in 1/10000 units).
    pub permyriad_bonus: i32,
}

impl BuffRegistry {
    /// Whether this buff is still active (ticks remaining > 0).
    pub fn is_active(&self) -> bool { self.ticks_remaining > 0 }

    /// Clear this buff by zeroing ticks remaining.
    pub fn clear(&mut self) { self.ticks_remaining = 0; }
}

/// Apply all active buffs to a base stat using the canonical modifier formula.
pub fn apply_modifier(buffs: &[BuffRegistry], base_stat: u32) -> u32 {
    let total_flat: i32 = buffs
        .iter()
        .filter(|b| b.is_active())
        .map(|b| match b.effect {
            BuffEffect::Buff => b.flat_bonus,
            BuffEffect::Debuff => -b.flat_bonus,
        })
        .sum();

    let total_perm: i32 = buffs
        .iter()
        .filter(|b| b.is_active())
        .map(|b| match b.effect {
            BuffEffect::Buff => b.permyriad_bonus,
            BuffEffect::Debuff => -b.permyriad_bonus,
        })
        .sum();

    let base_with_flat = (base_stat as i64 + total_flat as i64).max(0);
    let effective = (base_with_flat * (10000 + total_perm as i64)) / 10000;
    effective.max(0) as u32
}

/// Advance buff durations by the given tick count, returning expired buffs.
pub fn decay_buffs(buffs: &mut [BuffRegistry], ticks_elapsed: u16) -> Vec<BuffRegistry> {
    let mut expired = Vec::new();
    for buff in buffs.iter_mut() {
        if buff.ticks_remaining > 0 {
            buff.ticks_remaining = buff.ticks_remaining.saturating_sub(ticks_elapsed);
            if buff.ticks_remaining == 0 {
                expired.push(*buff);
                buff.clear();
            }
        }
    }
    expired
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_positive_buff_flat() {
        let buffs = vec![BuffRegistry {
            effect: BuffEffect::Buff,
            duration: 5,
            ticks_remaining: 5,
            stacking_policy: StackingPolicy::Stack,
            stat_target: StatTarget::Str,
            flat_bonus: 10,
            permyriad_bonus: 0,
        }];
        let result = apply_modifier(&buffs, 100);
        assert_eq!(result, 110);
    }

    #[test]
    fn apply_buff_decay_after_3_ticks() {
        let mut buffs = vec![BuffRegistry {
            effect: BuffEffect::Buff,
            duration: 5,
            ticks_remaining: 5,
            stacking_policy: StackingPolicy::Stack,
            stat_target: StatTarget::Str,
            flat_bonus: 10,
            permyriad_bonus: 0,
        }];

        assert_eq!(buffs[0].ticks_remaining, 5);
        let result_before = apply_modifier(&buffs, 100);
        assert_eq!(result_before, 110);

        decay_buffs(&mut buffs, 3);
        assert_eq!(buffs[0].ticks_remaining, 2);
        let result_after_decay = apply_modifier(&buffs, 100);
        assert_eq!(result_after_decay, 110);

        decay_buffs(&mut buffs, 2);
        assert_eq!(buffs[0].ticks_remaining, 0);
        let result_expired = apply_modifier(&buffs, 100);
        assert_eq!(result_expired, 100);
    }

    #[test]
    fn debuff_negates_flat() {
        let buffs = vec![BuffRegistry {
            effect: BuffEffect::Debuff,
            duration: 5,
            ticks_remaining: 5,
            stacking_policy: StackingPolicy::Stack,
            stat_target: StatTarget::Str,
            flat_bonus: 10,
            permyriad_bonus: 0,
        }];
        let result = apply_modifier(&buffs, 100);
        assert_eq!(result, 90);
    }

    #[test]
    fn buff_with_permyriad() {
        let buffs = vec![BuffRegistry {
            effect: BuffEffect::Buff,
            duration: 5,
            ticks_remaining: 5,
            stacking_policy: StackingPolicy::Stack,
            stat_target: StatTarget::Str,
            flat_bonus: 0,
            permyriad_bonus: 5000,
        }];
        let result = apply_modifier(&buffs, 100);
        assert_eq!(result, 150);
    }

    #[test]
    fn stacking_multiple_buffs() {
        let buffs = vec![
            BuffRegistry {
                effect: BuffEffect::Buff,
                duration: 5,
                ticks_remaining: 5,
                stacking_policy: StackingPolicy::Stack,
                stat_target: StatTarget::Str,
                flat_bonus: 10,
                permyriad_bonus: 0,
            },
            BuffRegistry {
                effect: BuffEffect::Buff,
                duration: 5,
                ticks_remaining: 5,
                stacking_policy: StackingPolicy::Stack,
                stat_target: StatTarget::Str,
                flat_bonus: 5,
                permyriad_bonus: 0,
            },
        ];
        let result = apply_modifier(&buffs, 100);
        assert_eq!(result, 115);
    }

    #[test]
    fn decay_expires_and_clears() {
        let mut buffs = vec![BuffRegistry {
            effect: BuffEffect::Buff,
            duration: 3,
            ticks_remaining: 3,
            stacking_policy: StackingPolicy::Stack,
            stat_target: StatTarget::Str,
            flat_bonus: 10,
            permyriad_bonus: 0,
        }];

        let expired = decay_buffs(&mut buffs, 3);
        assert_eq!(buffs[0].ticks_remaining, 0);
        assert_eq!(expired.len(), 1);
        assert_eq!(expired[0].effect, BuffEffect::Buff);
    }

    #[test]
    fn inactive_buff_ignored() {
        let buffs = vec![BuffRegistry {
            effect: BuffEffect::Buff,
            duration: 5,
            ticks_remaining: 0,
            stacking_policy: StackingPolicy::Stack,
            stat_target: StatTarget::Str,
            flat_bonus: 10,
            permyriad_bonus: 0,
        }];
        let result = apply_modifier(&buffs, 100);
        assert_eq!(result, 100);
    }

    #[test]
    fn combined_flat_and_permyriad() {
        let buffs = vec![BuffRegistry {
            effect: BuffEffect::Buff,
            duration: 5,
            ticks_remaining: 5,
            stacking_policy: StackingPolicy::Stack,
            stat_target: StatTarget::Str,
            flat_bonus: 20,
            permyriad_bonus: 5000,
        }];
        let result = apply_modifier(&buffs, 100);
        assert_eq!(result, 180);
    }
}
