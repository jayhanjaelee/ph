//! 시스템 시각. `core::clock::Clock` 어댑터는 조립 지점(bootstrap)에서 만든다.

use chrono::{DateTime, FixedOffset, Local};

/// 로컬 시간대 오프셋을 보존한 현재 시각.
pub fn now_local() -> DateTime<FixedOffset> {
    Local::now().fixed_offset()
}
