// SPDX-License-Identifier: GPL-3.0
// Copyright (C) 2026 caliberda (https://github.com/cesarkali)
// Ported from Mac Island & Dock 3.1.1: WeatherData, the Open-Meteo forecast request,
// WeatherConceptOf and Degrees. The island draws its own sky icons (no emoji in Inter).

use serde::{Deserialize, Serialize};

/// The mod's kWeatherRefreshSeconds.
pub const REFRESH_SECS: u64 = 30 * 60;

/// One call for everything the pill and the weather panel show (the mod's request).
pub fn forecast_url(latitude: f64, longitude: f64) -> String {
    format!(
        "https://api.open-meteo.com/v1/forecast?latitude={latitude:.4}&longitude={longitude:.4}\
         &current=temperature_2m,apparent_temperature,relative_humidity_2m,weather_code,cloud_cover,\
         pressure_msl,wind_speed_10m,is_day\
         &hourly=temperature_2m,weather_code,wind_speed_10m,is_day\
         &daily=weather_code,temperature_2m_max,temperature_2m_min,sunrise,sunset,uv_index_max,\
         precipitation_probability_max&timezone=auto&forecast_days=7"
    )
}

/// Open-Meteo's place search (settings: the city by name).
pub fn geocoding_url(name: &str) -> String {
    let mut q = String::new();
    for b in name.trim().bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => q.push(b as char),
            _ => q.push_str(&format!("%{b:02X}")),
        }
    }
    format!("https://geocoding-api.open-meteo.com/v1/search?name={q}&count=5&language={}&format=json", crate::i18n::current().code.split('-').next().unwrap_or("en"))
}

/// The places found: ("Cidade, Estado, País", latitude, longitude); empty when none or unreadable.
pub fn parse_places(json: &str) -> Vec<(String, f64, f64)> {
    #[derive(Deserialize)]
    struct Found {
        #[serde(default)]
        results: Vec<Place>,
    }
    #[derive(Deserialize)]
    struct Place {
        name: String,
        latitude: f64,
        longitude: f64,
        #[serde(default)]
        admin1: String,
        #[serde(default)]
        country: String,
    }
    let Ok(found) = serde_json::from_str::<Found>(json) else { return Vec::new() };
    found
        .results
        .into_iter()
        .map(|p| {
            let label = [p.name.as_str(), p.admin1.as_str(), p.country.as_str()]
                .into_iter()
                .filter(|s| !s.trim().is_empty())
                .collect::<Vec<_>>()
                .join(", ");
            (label, p.latitude, p.longitude)
        })
        .collect()
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Hour {
    /// The city's local time, "2026-11-11T17:00".
    pub time: String,
    pub temperature: f64,
    pub code: i32,
    pub wind: f64,
    pub day: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Day {
    /// "2026-11-11".
    pub date: String,
    pub code: i32,
    pub max: f64,
    pub min: f64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Weather {
    pub city: String,
    pub temperature: f64,
    pub feels_like: f64,
    pub humidity: i32,
    pub code: i32,
    pub day: bool,
    pub clouds: i32,
    pub pressure: f64,
    /// km/h.
    pub wind: f64,
    /// Today's, percent; None when the service has none.
    pub rain_chance: Option<i32>,
    pub uv: Option<f64>,
    /// Today's, "05:15".
    pub sunrise: String,
    pub sunset: String,
    /// The city's time now, "2026-11-11T16:45" (hours from here on are "Agora", ...).
    pub now: String,
    pub hours: Vec<Hour>,
    pub days: Vec<Day>,
}

#[derive(Deserialize)]
struct Response {
    current: Current,
    hourly: Hourly,
    daily: Daily,
}
#[derive(Deserialize)]
struct Current {
    time: String,
    temperature_2m: f64,
    apparent_temperature: f64,
    relative_humidity_2m: f64,
    weather_code: i32,
    cloud_cover: f64,
    pressure_msl: f64,
    wind_speed_10m: f64,
    is_day: i32,
}
#[derive(Deserialize)]
struct Hourly {
    time: Vec<String>,
    temperature_2m: Vec<Option<f64>>,
    weather_code: Vec<Option<i32>>,
    wind_speed_10m: Vec<Option<f64>>,
    is_day: Vec<Option<i32>>,
}
#[derive(Deserialize)]
struct Daily {
    time: Vec<String>,
    weather_code: Vec<Option<i32>>,
    temperature_2m_max: Vec<Option<f64>>,
    temperature_2m_min: Vec<Option<f64>>,
    sunrise: Vec<String>,
    sunset: Vec<String>,
    uv_index_max: Vec<Option<f64>>,
    precipitation_probability_max: Vec<Option<f64>>,
}

/// "2026-11-11T05:15" -> "05:15".
fn clock(time: &str) -> String {
    time.split_once('T').map_or(String::new(), |(_, t)| t.chars().take(5).collect())
}

impl Weather {
    pub fn from_open_meteo(city: &str, json: &str) -> anyhow::Result<Self> {
        let r: Response = serde_json::from_str(json)?;
        let hours = r.hourly.time.iter().enumerate().filter_map(|(i, time)| Some(Hour {
            time: time.clone(),
            temperature: (*r.hourly.temperature_2m.get(i)?)?,
            code: r.hourly.weather_code.get(i).copied().flatten().unwrap_or(0),
            wind: r.hourly.wind_speed_10m.get(i).copied().flatten().unwrap_or(0.0),
            day: r.hourly.is_day.get(i).copied().flatten().unwrap_or(1) != 0,
        })).collect();
        let days = r.daily.time.iter().enumerate().filter_map(|(i, date)| Some(Day {
            date: date.clone(),
            code: r.daily.weather_code.get(i).copied().flatten().unwrap_or(0),
            max: (*r.daily.temperature_2m_max.get(i)?)?,
            min: (*r.daily.temperature_2m_min.get(i)?)?,
        })).collect();
        Ok(Self {
            city: city.to_owned(),
            temperature: r.current.temperature_2m,
            feels_like: r.current.apparent_temperature,
            humidity: r.current.relative_humidity_2m.round() as i32,
            code: r.current.weather_code,
            day: r.current.is_day != 0,
            clouds: r.current.cloud_cover.round() as i32,
            pressure: r.current.pressure_msl,
            wind: r.current.wind_speed_10m,
            rain_chance: r.daily.precipitation_probability_max.first().copied().flatten().map(|p| p.round() as i32),
            uv: r.daily.uv_index_max.first().copied().flatten(),
            sunrise: r.daily.sunrise.first().map(|t| clock(t)).unwrap_or_default(),
            sunset: r.daily.sunset.first().map(|t| clock(t)).unwrap_or_default(),
            now: r.current.time,
            hours,
            days,
        })
    }

    /// The hours of `date` ("2026-11-11") to show, at most `count`: today's hour by hour from the
    /// current one; another day's every three hours, so the whole day fits.
    pub fn hours_of(&self, date: &str, count: usize) -> Vec<&Hour> {
        let current_hour = self.now.get(..13).unwrap_or("");
        let today = self.now.starts_with(date);
        self.hours.iter()
            .filter(|h| h.time.starts_with(date) && h.time.get(..13).unwrap_or("") >= current_hour)
            .filter(|h| today || h.time.get(11..13).and_then(|hh| hh.parse::<u32>().ok()).is_some_and(|hh| hh % 3 == 0))
            .take(count)
            .collect()
    }
}

/// The mod's kinds of sky (WeatherConcept), in its order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sky {
    ClearDay,
    ClearNight,
    PartlyDay,
    PartlyNight,
    Cloud,
    Rain,
    Drizzle,
    Snow,
    Sleet,
    Thunder,
    Fog,
}

impl Sky {
    /// The mod's WeatherConceptOf: a WMO weather code, by day or by night.
    pub fn of(code: i32, day: bool) -> Self {
        match code {
            0 => if day { Sky::ClearDay } else { Sky::ClearNight },
            1 => if day { Sky::PartlyDay } else { Sky::ClearNight },
            2 => if day { Sky::PartlyDay } else { Sky::PartlyNight },
            3 => Sky::Cloud,
            45 | 48 => Sky::Fog,
            51 | 53 | 55 => Sky::Drizzle,
            56 | 57 | 66 | 67 => Sky::Sleet,
            71 | 73 | 75 | 77 | 85 | 86 => Sky::Snow,
            95 | 96 | 99 => Sky::Thunder,
            // Rain, showers, and any code not known.
            _ => Sky::Rain,
        }
    }

    pub fn name(self) -> &'static str {
        crate::i18n::tr(match self {
            Sky::ClearDay | Sky::ClearNight => "weather.clear",
            Sky::PartlyDay | Sky::PartlyNight => "weather.partly",
            Sky::Cloud => "weather.cloudy",
            Sky::Rain => "weather.rain",
            Sky::Drizzle => "weather.drizzle",
            Sky::Snow => "weather.snow",
            Sky::Sleet => "weather.sleet",
            Sky::Thunder => "weather.thunder",
            Sky::Fog => "weather.fog",
        })
    }
}

/// The mod's Degrees: "19°", from °C (in °F when asked).
pub fn degrees(celsius: f64, fahrenheit: bool) -> String {
    let shown = if fahrenheit { celsius * 9.0 / 5.0 + 32.0 } else { celsius };
    format!("{}°", shown.round() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
        "current":{"time":"2026-11-11T16:45","temperature_2m":24.4,"apparent_temperature":25.2,
            "relative_humidity_2m":61,"weather_code":53,"cloud_cover":32,"pressure_msl":1018.3,
            "wind_speed_10m":12.1,"is_day":1},
        "hourly":{"time":["2026-11-11T15:00","2026-11-11T16:00","2026-11-11T17:00","2026-11-12T00:00"],
            "temperature_2m":[25.0,26.1,22.0,18.5],"weather_code":[2,61,80,null],
            "wind_speed_10m":[8,7.6,14,3],"is_day":[1,1,1,0]},
        "daily":{"time":["2026-11-11","2026-11-12"],"weather_code":[53,95],
            "temperature_2m_max":[27.2,26],"temperature_2m_min":[15.1,18],
            "sunrise":["2026-11-11T05:15","2026-11-12T05:14"],"sunset":["2026-11-11T18:26","2026-11-12T18:27"],
            "uv_index_max":[9.0,8.5],"precipitation_probability_max":[73,null]}
    }"#;

    #[test]
    fn the_forecast_reads_now_hours_and_days() {
        let w = Weather::from_open_meteo("Bom Jesus, São Paulo", SAMPLE).unwrap();
        assert_eq!((w.humidity, w.clouds, w.rain_chance, w.uv), (61, 32, Some(73), Some(9.0)));
        assert_eq!((w.sunrise.as_str(), w.sunset.as_str()), ("05:15", "18:26"));
        assert_eq!(w.days.len(), 2);
        assert_eq!(degrees(w.temperature, false), "24°");
        assert_eq!(degrees(w.temperature, true), "76°");
        // Today from the current hour on; a missing code is clear sky, not a lost hour.
        let today: Vec<_> = w.hours_of("2026-11-11", 8).iter().map(|h| h.time.as_str()).collect();
        assert_eq!(today, ["2026-11-11T16:00", "2026-11-11T17:00"]);
        assert_eq!(w.hours_of("2026-11-12", 8)[0].code, 0);
        assert_eq!(w.hours_of("2026-11-12", 8)[0].time, "2026-11-12T00:00");
    }

    #[test]
    fn weather_codes_group_into_the_mods_skies() {
        assert_eq!(Sky::of(0, false), Sky::ClearNight);
        assert_eq!(Sky::of(1, false), Sky::ClearNight);
        assert_eq!(Sky::of(2, false), Sky::PartlyNight);
        assert_eq!(Sky::of(53, true).name(), "Garoa");
        assert_eq!(Sky::of(66, true), Sky::Sleet);
        assert_eq!(Sky::of(96, true), Sky::Thunder);
        assert_eq!(Sky::of(81, true), Sky::Rain);
        assert!(Weather::from_open_meteo("x", "{}").is_err());
    }
}
