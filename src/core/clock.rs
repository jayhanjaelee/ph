//! 시간 주입용 trait.

use chrono::{DateTime, FixedOffset};

/// 현재 시각 공급자. core 는 시스템 시계를 직접 읽지 않는다.
pub trait Clock {
    /// 현재 시각.
    fn now(&self) -> DateTime<FixedOffset>;
}

/// 항상 같은 시각을 돌려주는 테스트용 시계.
#[derive(Debug, Clone, Copy)]
pub struct FixedClock(pub DateTime<FixedOffset>);

impl Clock for FixedClock {
    fn now(&self) -> DateTime<FixedOffset> {
        self.0
    }
}
