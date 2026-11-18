// SPDX-License-Identifier: GPL-3.0-or-later
//! The PC's time zone and region, read from Windows (the mod gets both from the system too).

/// Seconds east of UTC from GetTimeZoneInformation's biases (minutes, west-positive): the standard
/// bias, plus the daylight one only while daylight saving is in effect.
pub fn offset_from_bias(bias: i32, standard_bias: i32, daylight_bias: i32, daylight_now: bool) -> i64 {
    -((bias + if daylight_now { daylight_bias } else { standard_bias }) as i64) * 60
}

/// The zone Windows is set to, now (daylight saving included); None when it cannot be read.
pub fn utc_offset_secs() -> Option<i64> {
    #[cfg(windows)]
    {
        use windows::Win32::System::Time::{GetTimeZoneInformation, TIME_ZONE_INFORMATION};
        let mut tz = TIME_ZONE_INFORMATION::default();
        // TIME_ZONE_ID_UNKNOWN 0, STANDARD 1, DAYLIGHT 2; u32::MAX on failure.
        let id = unsafe { GetTimeZoneInformation(&mut tz) };
        if id == u32::MAX { return None; }
        Some(offset_from_bias(tz.Bias, tz.StandardBias, tz.DaylightBias, id == 2))
    }
    #[cfg(not(windows))]
    None
}

/// The two-letter country of Windows' region setting ("BR"), upper case.
pub fn region_code() -> Option<String> {
    #[cfg(windows)]
    {
        let mut buf = [0u16; 16];
        let n = unsafe { windows::Win32::Globalization::GetUserDefaultGeoName(&mut buf) };
        let name = String::from_utf16_lossy(buf.get(..(n.max(1) as usize - 1))?);
        (name.len() == 2 && name.chars().all(|c| c.is_ascii_alphabetic())).then(|| name.to_uppercase())
    }
    #[cfg(not(windows))]
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bias_is_west_positive_and_daylight_only_when_in_effect() {
        assert_eq!(offset_from_bias(180, 0, -60, false), -3 * 3600); // São Paulo
        assert_eq!(offset_from_bias(0, 0, -60, true), 3600); // London in summer
        assert_eq!(offset_from_bias(0, 0, -60, false), 0);
        assert_eq!(offset_from_bias(-330, 0, 0, false), 19_800); // India, +5:30
    }

    #[test]
    fn this_pc_answers() {
        assert!(utc_offset_secs().is_some_and(|s| s.abs() <= 14 * 3600));
        assert!(region_code().is_none_or(|c| c.len() == 2));
    }
}
