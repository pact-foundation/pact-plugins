use regex::Regex;
use tonic::{Request, Response, Status};
use tonic::metadata::{Ascii, MetadataValue};
use tonic::transport::Server;
use tracing::{error, info};
use tracing_subscriber::FmtSubscriber;
use crate::pb::test_server::{Test, TestServer};
use crate::pb::{ValidateTokenRequest, ValidateTokenResult};

pub mod pb {
    tonic::include_proto!("metadatatest");
}

#[derive(Default)]
pub struct TokenValidator {}

#[tonic::async_trait]
impl Test for TokenValidator {
    async fn validate_token(
        &self,
        request: Request<ValidateTokenRequest>
    ) -> Result<Response<ValidateTokenResult>, Status> {
        let auth = request.metadata().get("Auth");
        match auth {
            None => {
                error!("No Auth provided");
                Err(Status::failed_precondition("No Auth provided"))
            },
            Some(auth) => if let Ok(auth) = auth.to_str() {
                let re = Regex::new(r"[A-Z]{3}\d+").unwrap();
                let mut response = Response::new(ValidateTokenResult {
                    ok: re.is_match(auth)
                });
                if let Ok(value) = "1234".parse::<MetadataValue<Ascii>>() {
                    response.metadata_mut().insert("code", value);
                }
                Ok(response)
            } else {
                error!("Auth is not valid ASCII");
                Err(Status::failed_precondition("Auth is not valid ASCII"))
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let subscriber = FmtSubscriber::builder()
      .pretty()
      .finish();
    if let Err(err) = tracing::subscriber::set_global_default(subscriber) {
        eprintln!("WARN: Failed to initialise global tracing subscriber - {err}");
    };

    let addr = "[::1]:50051".parse().unwrap();
    let validator = TokenValidator::default();

    info!("TokenValidator listening on {}", addr);

    Server::builder()
      .add_service(TestServer::new(validator))
      .serve(addr)
      .await?;

    Ok(())
}
