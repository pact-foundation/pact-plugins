use actix_web::{App, HttpResponse, HttpServer, post, web};
use actix_web::middleware::Logger;
use bytes::BytesMut;
use log::debug;
use maplit::btreemap;
use prost::Message;
use prost_types::{ListValue, Struct, Value};
use prost_types::value::Kind;

tonic::include_proto!("google_structs");

#[post("/")]
async fn message(data: web::Json<serde_json::Value>) -> HttpResponse {
    debug!("GET message request {:?}", data);

    let mut buffer = BytesMut::new();
    let request = Request {
        name: "The Message".to_string(),
        params: Some(Struct {
            fields: btreemap!{
              "message".to_string() => Value {
                kind: Some(Kind::StringValue("test".to_string()))
              },
              "kind".to_string() => Value {
                kind: Some(Kind::StringValue("general".to_string()))
              },
              "xids".to_string() => Value {
                kind: Some(Kind::ListValue(ListValue {
                  values: vec![
                    Value {
                      kind: Some(Kind::NumberValue(1.0))
                    }
                  ]
                }))
              },
              "other".to_string() => Value {
                kind: Some(Kind::StructValue(Struct {
                  fields: btreemap!{
                    "a".to_string() => Value {
                      kind: Some(Kind::StringValue("test".to_string()))
                    }
                  }
                }))
              }
            }
        })
    };
    request.encode(&mut buffer).unwrap();

    HttpResponse::Ok()
      .content_type("application/protobuf; message=.google_structs.Request")
      .body(buffer.freeze())
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
  env_logger::init_from_env(env_logger::Env::new().default_filter_or("info"));

  HttpServer::new(|| {
      App::new()
        .wrap(Logger::default())
        .service(message)
  })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
