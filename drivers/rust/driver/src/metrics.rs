use std::env::consts::{ARCH, OS};
use std::env::var;

use log::{debug, info, warn};
use reqwest::Client;
use serde_json::json;
use uuid::Uuid;

use crate::plugin_models::PactPluginManifest;

static CIS: &'static [&str] = &[
  "CI",
  "CONTINUOUS_INTEGRATION",
  "BSTRUSE_BUILD_DIR",
  "APPVEYOR",
  "BUDDY_WORKSPACE_URL",
  "BUILDKITE",
  "CF_BUILD_URL",
  "CIRCLECI",
  "CODEBUILD_BUILD_ARN",
  "CONCOURSE_URL",
  "DRONE",
  "GITLAB_CI",
  "GO_SERVER_URL",
  "JENKINS_URL",
  "PROBO_ENVIRONMENT",
  "SEMAPHORE",
  "SHIPPABLE",
  "TDDIUM",
  "TEAMCITY_VERSION",
  "TF_BUILD",
  "TRAVIS",
  "WERCKER_ROOT"
];

pub(crate) fn send_metrics(manifest: &PactPluginManifest) {
  let do_not_track = match var("pact_do_not_track") {
    Ok(val) => val == "true",
    Err(_) => false
  };

  if do_not_track {
    debug!("'pact_do_not_track' environment variable is set, will not send metrics");
  } else {
    match tokio::runtime::Handle::try_current() {
      Ok(handle) => {
        let manifest = manifest.clone();
        handle.spawn(async move {
          warn!(
            "\n\nPlease note:\n\
            We are tracking this plugin load anonymously to gather important usage statistics.\n\
            To disable tracking, set the 'pact_do_not_track' environment variable to 'true'.\n\n"
          );

          let ci_context = if CIS.iter()
            .any(|n| var(n).map(|val| !val.is_empty()).unwrap_or(false)) {
            "CI"
          } else {
            "unknown"
          };
          let event_payload = json!({
          "v": 1,                                         // Version of the API
          "tid": "UA-117778936-1",                        // Property ID
          "cid": Uuid::new_v4().to_string(),              // Anonymous Client ID.
          "an": "pact-plugins-rust",                      // App name.
          "aid": "pact-plugins-rust",                     // App Id
          "av": env!("CARGO_PKG_VERSION"),                // App version.
          "aip": true,                                    // Anonymise IP address
          "ds": "pact-plugins-rust",                      // Data source
          "cd1": "pact-plugins-rust",                     // Custom Dimension 1: library
          "cd2": ci_context,                              // Custom Dimension 2: context
          "cd3": format!("{}-{}", OS, ARCH),              // Custom Dimension 3: osarch
          "cd4": manifest.name,                           // Custom Dimension 4: plugin_name
          "cd5": manifest.version,                        // Custom Dimension 5: plugin_version
          "el": "Plugin loaded",                          // Event
          "ec": "Plugin",                                 // Category
          "ea": "Loaded",                                 // Action
          "ev": 1                                         // Value
        });
          let result = Client::new().post("https://www.google-analytics.com/collect")
            .json(&event_payload)
            .send()
            .await;
          if let Err(err) = result {
            debug!("Failed to post plugin loaded event - {}", err);
          }
        });
      },
      Err(err) => {
        debug!("Could not get the tokio runtime, will not send metrics - {}", err)
      }
    }
  }
}
