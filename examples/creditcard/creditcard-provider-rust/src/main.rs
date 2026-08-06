//! Provider for the creditcard plugin example.
//!
//! An ordinary JSON HTTP service - it knows nothing about Pact or about the plugin. That is the
//! point of the example: the `creditcard` matching rule applies to one field inside a body this
//! provider produces without any special handling, unlike a content-matcher plugin which owns the
//! whole content type.
//!
//! The card numbers below are the well-known test numbers published by the card schemes. They are
//! not real accounts, but they are Luhn-valid, which is what the plugin's rule checks.

use actix_web::{App, HttpResponse, HttpServer, get, web};
use log::*;
use serde_json::json;

/// A Luhn-valid test number for each brand the plugin knows, so the provider can answer whichever
/// brand a consumer asked for
fn card_for(brand: &str) -> Option<&'static str> {
    match brand {
        "amex" => Some("378282246310005"),
        "diners" => Some("30569309025904"),
        "discover" => Some("6011111111111117"),
        "jcb" => Some("3530111333300000"),
        "mastercard" => Some("5555555555554444"),
        "visa" => Some("4111111111111111"),
        _ => None
    }
}

#[get("/cards/{brand}")]
async fn get_card(path: web::Path<String>) -> HttpResponse {
    let brand = path.into_inner();
    debug!("GET request for a {} card", brand);

    match card_for(brand.as_str()) {
        Some(number) => HttpResponse::Ok()
            .content_type("application/json; charset=utf-8")
            .body(json!({
                "card": {
                    "number": number,
                    "expiry": "04/28",
                    "brand": brand
                }
            }).to_string()),
        None => HttpResponse::NotFound()
            .content_type("application/json; charset=utf-8")
            .body(json!({
                "error": format!("'{}' is not a card brand this service issues", brand)
            }).to_string())
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let _ = simple_log::quick();
    info!("Starting the creditcard provider on 127.0.0.1:8080");
    HttpServer::new(|| App::new().service(get_card))
        .bind("127.0.0.1:8080")?
        .run()
        .await
}
