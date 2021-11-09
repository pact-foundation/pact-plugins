#[cfg(test)]
mod tests {
  use std::ffi::{CStr, CString};
  use std::fs::remove_dir_all;
  use std::panic::catch_unwind;
  use std::path::Path;
  use std::ptr::null;

  use expectest::prelude::*;
  use pact_ffi::mock_server::handles::{InteractionPart, pactffi_new_interaction, pactffi_new_pact, pactffi_response_status, pactffi_with_request};
  use pact_ffi::mock_server::{pactffi_cleanup_mock_server, pactffi_create_mock_server_for_pact, pactffi_mock_server_mismatches, pactffi_write_pact_file};
  use pact_ffi::plugins::{pactffi_cleanup_plugins, pactffi_interaction_contents, pactffi_using_plugin};
  use reqwest::blocking::Client;
  use serde_json::json;

  #[test]
  fn test_csv_client() {
    let _ = env_logger::builder().is_test(true).try_init();

    let consumer_name = CString::new("csv-consumer").unwrap();
    let provider_name = CString::new("csv-provider").unwrap();
    let plugin_name = CString::new("csv").unwrap();
    let pact_handle = pactffi_new_pact(consumer_name.as_ptr(), provider_name.as_ptr());

    expect!(pactffi_using_plugin(pact_handle, plugin_name.as_ptr(), null())).to(be_equal_to(0));

    let description = CString::new("request for a report").unwrap();
    let interaction = pactffi_new_interaction(pact_handle, description.as_ptr());

    let method = CString::new("GET").unwrap();
    let path = CString::new("/reports/report.csv").unwrap();
    pactffi_with_request(interaction, method.as_ptr(), path.as_ptr());

    let content_type = CString::new("text/csv").unwrap();
    let contents = CString::new(json!({
      "csvHeaders": false,
      "column:1": "matching(type,'Name')",
      "column:2": "matching(number,100)",
      "column:3": "matching(datetime, 'yyyy-MM-dd','2000-01-01')"
    }).to_string()).unwrap();
    pactffi_response_status(interaction, 200);
    pactffi_interaction_contents(interaction, InteractionPart::Response, content_type.as_ptr(), contents.as_ptr());

    let address = CString::new("127.0.0.1:0").unwrap();
    let port = pactffi_create_mock_server_for_pact(pact_handle, address.as_ptr(), false);

    let test = catch_unwind(|| {
        let client = Client::default();
        let result = client.get(format!("http://127.0.0.1:{}/reports/report.csv", port).as_str())
          .send();

        match result {
          Ok(res) => {
            expect!(res.status()).to(be_eq(200));
            expect!(res.text()).to(be_ok());
          },
          Err(err) => panic!("expected 200 response but request failed - {}", err)
        };
    });

    let mismatches = unsafe {
      CStr::from_ptr(pactffi_mock_server_mismatches(port)).to_string_lossy().into_owned()
    };

    let file_path = CString::new("/tmp/test_csv_client").unwrap();
    pactffi_write_pact_file(port, file_path.as_ptr(), true);
    pactffi_cleanup_mock_server(port);
    pactffi_cleanup_plugins(pact_handle);

    expect!(mismatches).to(be_equal_to("[]"));
    expect!(test).to(be_ok());

    let path = Path::new(file_path.to_str().unwrap());
    let _ = remove_dir_all(path);
  }

  #[test]
  fn test_message_client() {
    let _ = env_logger::builder().is_test(true).try_init();
  }
}
