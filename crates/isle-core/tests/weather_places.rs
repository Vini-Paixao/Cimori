use isle_core::weather::{geocoding_url, parse_places};

/// Shape of Open-Meteo's geocoding answer (fields trimmed to the ones read and a few neighbours).
const RIO_PEQUENO: &str = r#"{"results":[
 {"id":3451235,"name":"Bom Jesus","latitude":-23.5505,"longitude":-46.6333,"elevation":760.0,
  "feature_code":"PPLX","country_code":"BR","admin1_id":3448433,"timezone":"America/Sao_Paulo",
  "country_id":3469034,"country":"Brasil","admin1":"São Paulo","admin2":"São Paulo"},
 {"id":3451234,"name":"Bom Jesus","latitude":-26.0,"longitude":-49.1,"country":"Brasil","admin1":"Santa Catarina"},
 {"id":1,"name":"Lugar","latitude":1.5,"longitude":2.5,"country":"","admin1":""}
],"generationtime_ms":0.6}"#;

#[test]
fn places_read_name_state_and_country() {
    let places = parse_places(RIO_PEQUENO);
    assert_eq!(places.len(), 3);
    assert_eq!(places[0], ("Bom Jesus, São Paulo, Brasil".to_string(), -23.5505, -46.6333));
    assert_eq!(places[1].0, "Bom Jesus, Santa Catarina, Brasil");
    assert_eq!(places[2].0, "Lugar", "empty parts are skipped");
}

#[test]
fn no_results_or_bad_json_mean_no_places() {
    assert!(parse_places(r#"{"generationtime_ms":0.1}"#).is_empty());
    assert!(parse_places("not json").is_empty());
}

#[test]
fn the_name_is_percent_encoded() {
    assert_eq!(
        geocoding_url("São Paulo"),
        "https://geocoding-api.open-meteo.com/v1/search?name=S%C3%A3o%20Paulo&count=5&language=pt&format=json"
    );
    assert!(geocoding_url("a&b=c").contains("name=a%26b%3Dc&"));
}
