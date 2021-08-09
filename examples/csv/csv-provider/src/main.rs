use actix_web::{App, get, HttpResponse, HttpServer};
use csv::Writer;
use rand::prelude::*;
use fakeit::{name, datetime};

#[get("/data")]
async fn data() -> HttpResponse {
    let rows: u8 = random();
    let mut wtr = Writer::from_writer(vec![]);
    for _row in 0..(rows + 1) {
        let num: u8 = random();
        let month = datetime::month().parse::<u8>().unwrap_or_default();
        let day = datetime::day().parse::<u8>().unwrap_or_default();
        let _ = wtr.write_record(&[
            name::full().as_str(),
            num.to_string().as_str(),
            format!("{}-{:02}-{:02}", datetime::year(), month, day).as_str()
        ]);
    }
    HttpResponse::Ok()
      .content_type("text/csv")
      .body(wtr.into_inner().unwrap_or_default())
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new().service(data)
    })
      .bind("127.0.0.1:8080")?
      .run()
      .await
}
