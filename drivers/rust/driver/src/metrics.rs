use std::env::consts::{ARCH, OS};
use std::env::var;
use std::process::Command;
use std::str;

use anyhow::anyhow;
use maplit::hashmap;
use reqwest::Client;
use tracing::{debug, warn};
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
          let osarch = format!("{}-{}", OS, ARCH);
          let uid = hostname_hash();

          let event_payload = hashmap!{
            "v" => "1",                                       // Version of the API
            "t" => "event",                                   // Hit type, Specifies the metric is for an event
            "tid" => "UA-117778936-1",                        // Property ID
            "cid" => uid.as_str(),                            // Anonymous Client ID.
            "an" => "pact-plugins-rust",                      // App name.
            "aid" => "pact-plugins-rust",                     // App Id
            "av" => env!("CARGO_PKG_VERSION"),                // App version.
            "aip" => "true",                                  // Anonymise IP address
            "ds" => "pact-plugins-rust",                      // Data source
            "cd2" => ci_context,                              // Custom Dimension 2: context
            "cd3" => osarch.as_str(),                         // Custom Dimension 3: osarch
            "cd4" => manifest.name.as_str(),                  // Custom Dimension 4: plugin_name
            "cd5" => manifest.version.as_str(),               // Custom Dimension 5: plugin_version
            "el" => "Plugin loaded",                          // Event
            "ec" => "Plugin",                                 // Category
            "ea" => "Loaded",                                 // Action
            "ev" => "1"                                       // Value
          };
          debug!("Sending event to GA - {:?}", event_payload);
          let result = Client::new().post("https://www.google-analytics.com/collect")
            .form(&event_payload)
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

fn hostname_hash() -> String {
  let host_name = if OS == "windows" {
    var("COMPUTERNAME")
  } else {
    var("HOSTNAME")
  }.or_else(|_| {
    exec_hostname_command()
  }).unwrap_or_else(|_| {
    Uuid::new_v4().to_string()
  });

  let digest = md5::compute(host_name.as_bytes());
  format!("{:x}", digest)
}

fn exec_hostname_command() -> anyhow::Result<String> {
  match Command::new("hostname").output() {
    Ok(output) => if output.status.success() {
      Ok(str::from_utf8(&*output.stdout)?.to_string())
    } else {
      Err(anyhow!("Failed to invoke hostname command: status {}", output.status))
    }
    Err(err) => Err(anyhow!("Failed to invoke hostname command: {}", err))
  }
}
