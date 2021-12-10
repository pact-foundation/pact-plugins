#[cfg(test)]
mod tests {
  use std::ffi::{CStr, CString};
  use std::fs::{create_dir_all, remove_dir_all, write};
  use std::panic::catch_unwind;
  use std::path::{Path, PathBuf};
  use std::ptr::null;

  use expectest::prelude::*;
  use pact_ffi::mock_server::{
    pactffi_cleanup_mock_server,
    pactffi_create_mock_server_for_pact,
    pactffi_mock_server_mismatches,
    pactffi_write_pact_file
  };
  use pact_ffi::mock_server::handles::{InteractionPart, pactffi_free_pact_handle, pactffi_interaction_test_name, pactffi_new_interaction, pactffi_new_message_interaction, pactffi_new_pact, pactffi_new_sync_message_interaction, pactffi_pact_handle_get_message_iter, pactffi_pact_handle_get_sync_message_iter, pactffi_response_status, pactffi_with_request};
  use pact_ffi::models::iterators::{pactffi_pact_message_iter_delete, pactffi_pact_message_iter_next, pactffi_pact_sync_message_iter_delete, pactffi_pact_sync_message_iter_next};
  use pact_ffi::models::message::{pactffi_message_get_contents_bin, pactffi_message_get_contents_length};
  use pact_ffi::models::sync_message::{
    pactffi_sync_message_get_request_contents_bin,
    pactffi_sync_message_get_request_contents_length,
    pactffi_sync_message_get_response_contents_bin,
    pactffi_sync_message_get_response_contents_length
  };
  use pact_ffi::plugins::{pactffi_cleanup_plugins, pactffi_interaction_contents, pactffi_using_plugin};
  use prost::Message;
  use reqwest::blocking::Client;
  use serde_json::json;

  use pact_plugin_driver::proto::{InitPluginRequest, InitPluginResponse};

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
    pactffi_free_pact_handle(pact_handle);

    expect!(mismatches).to(be_equal_to("[]"));
    expect!(test).to(be_ok());

    let path = Path::new(file_path.to_str().unwrap());
    let _ = remove_dir_all(path);
  }

  #[test]
  fn test_message_client() {
    let _ = env_logger::builder().is_test(true).try_init();

    let consumer_name = CString::new("protobuf-consumer").unwrap();
    let provider_name = CString::new("protobuf-provider").unwrap();
    let plugin_name = CString::new("protobuf").unwrap();
    let pact_handle = pactffi_new_pact(consumer_name.as_ptr(), provider_name.as_ptr());

    let dir = PathBuf::from("/tmp/test_message_client");
    create_dir_all(dir.clone()).unwrap();
    let file_path = dir.join("plugin.proto");
    let proto_file = include_str!("../../driver/plugin.proto");
    write(file_path.as_path(), proto_file).unwrap();

    expect!(pactffi_using_plugin(pact_handle, plugin_name.as_ptr(), null())).to(be_equal_to(0));

    let result = catch_unwind(|| {
      let description = CString::new("init plugin message").unwrap();
      let interaction = pactffi_new_message_interaction(pact_handle, description.as_ptr());

      let content_type = CString::new("application/protobuf").unwrap();
      let contents = CString::new(json!({
        "pact:proto": file_path.to_str().unwrap(),
        "pact:message-type": "InitPluginRequest",
        "pact:content-type": "application/protobuf",
        "implementation": "notEmpty('pact-jvm-driver')",
        "version": "matching(semver, '0.0.0')"
      }).to_string()).unwrap();

      expect!(pactffi_interaction_contents(interaction, InteractionPart::Request, content_type.as_ptr(), contents.as_ptr())).to(be_equal_to(0));

      let messages = pactffi_pact_handle_get_message_iter(pact_handle);
      let mut message = pactffi_pact_message_iter_next(messages);
      expect!(message.is_null()).to(be_false());

      while !message.is_null() {
        let contents_len = pactffi_message_get_contents_length(message);
        let contents = pactffi_message_get_contents_bin(message);

        let slice: &[u8] = unsafe { std::slice::from_raw_parts(contents, contents_len) };
        let request = InitPluginRequest::decode(slice).unwrap();

        expect!(request.implementation).to(be_equal_to("pact-jvm-driver"));
        expect!(request.version).to(be_equal_to("0.0.0"));

        message = pactffi_pact_message_iter_next(messages);
      }

      pactffi_pact_message_iter_delete(messages);
    });

    pactffi_cleanup_plugins(pact_handle);
    pactffi_free_pact_handle(pact_handle);

    let _ = remove_dir_all(dir.clone());

    expect!(result).to(be_ok());
  }

  #[test]
  fn test_proto_service() {
    let _ = env_logger::builder().is_test(true).try_init();

    let consumer_name = CString::new("protobuf-consumer").unwrap();
    let provider_name = CString::new("protobuf-provider").unwrap();
    let plugin_name = CString::new("protobuf").unwrap();
    let pact_handle = pactffi_new_pact(consumer_name.as_ptr(), provider_name.as_ptr());

    let dir = PathBuf::from("/tmp/test_proto_service");
    create_dir_all(dir.clone()).unwrap();
    let file_path = dir.join("plugin.proto");
    let proto_file = include_str!("../../driver/plugin.proto");
    write(file_path.as_path(), proto_file).unwrap();

    expect!(pactffi_using_plugin(pact_handle, plugin_name.as_ptr(), null())).to(be_equal_to(0));

    let result = catch_unwind(|| {
      let description = CString::new("init plugin request").unwrap();
      let interaction = pactffi_new_sync_message_interaction(pact_handle, description.as_ptr());
      let test_name = CString::new("test_proto_service").unwrap();
      expect!(pactffi_interaction_test_name(interaction, test_name.as_ptr())).to(be_equal_to(0));

      let content_type = CString::new("application/protobuf").unwrap();
      let contents = CString::new(json!({
        "pact:proto": file_path.to_str().unwrap(),
        "pact:proto-service": "PactPlugin/InitPlugin",
        "pact:content-type": "application/protobuf",
        "request": {
          "implementation": "notEmpty('plugin-driver-rust')",
          "version": "matching(semver, '0.0.0')"
        },
        "response": {
          "catalogue": {
            "pact:match" : "eachValue(matching($'CatalogueEntry'))",
            "CatalogueEntry": {
              "type": "matching(regex, 'content-matcher|content-generator', 'content-matcher')",
              "key": "notEmpty('test')"
            }
          }
        }
      }).to_string()).unwrap();

      expect!(pactffi_interaction_contents(interaction, InteractionPart::Request, content_type.as_ptr(), contents.as_ptr())).to(be_equal_to(0));

      let messages = pactffi_pact_handle_get_sync_message_iter(pact_handle);
      let mut message = pactffi_pact_sync_message_iter_next(messages);
      expect!(message.is_null()).to(be_false());

      while !message.is_null() {
        let request_len = pactffi_sync_message_get_request_contents_length(message);
        let request_contents = pactffi_sync_message_get_request_contents_bin(message);

        let slice: &[u8] = unsafe { std::slice::from_raw_parts(request_contents, request_len) };
        let request = InitPluginRequest::decode(slice).unwrap();

        let response_len = pactffi_sync_message_get_response_contents_length(message, 0);
        let response_contents = pactffi_sync_message_get_response_contents_bin(message, 0);

        let response_slice: &[u8] = unsafe { std::slice::from_raw_parts(response_contents, response_len) };
        let response = InitPluginResponse::decode(response_slice).unwrap();

        expect!(request.implementation).to(be_equal_to("plugin-driver-rust"));
        expect!(request.version).to(be_equal_to("0.0.0"));
        expect!(response.catalogue.len()).to(be_equal_to(1));
        expect!(&response.catalogue.first().unwrap().key).to(be_equal_to("test"));

        message = pactffi_pact_sync_message_iter_next(messages);
      }

      pactffi_pact_sync_message_iter_delete(messages);
    });

    pactffi_cleanup_plugins(pact_handle);
    pactffi_free_pact_handle(pact_handle);

    let _ = remove_dir_all(dir.clone());

    expect!(result).to(be_ok());
  }
}
