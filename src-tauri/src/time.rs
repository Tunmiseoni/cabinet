use time::macros::format_description;
use time::OffsetDateTime;

pub fn now_ms() -> u64 {
    let millis = OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000;
    millis.max(0) as u64
}

pub fn utc_stamp() -> String {
    let format = format_description!("[year][month][day]-[hour][minute][second]");
    OffsetDateTime::now_utc()
        .format(&format)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utc_stamp_has_the_expected_shape() {
        let stamp = utc_stamp();
        assert_eq!(stamp.len(), 15, "got {stamp}");
        assert_eq!(&stamp[8..9], "-");
    }

    #[test]
    fn now_ms_is_after_the_epoch() {
        assert!(now_ms() > 1_600_000_000_000);
    }
}
