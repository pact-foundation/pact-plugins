use log::debug;

use crate::init::EntryType;

wit_bindgen::generate!("plugin");

struct CsvPlugin;

impl PluginInterface for CsvPlugin {
  fn init_plugin(implementation: String, version: String) -> Result<Vec<CatalogueEntry>, ()> {
    debug!("Init request from {}/{}", implementation, version);
    Ok(vec![
        CatalogueEntry {
          entry_type: EntryType::ContentMatcher,
          key: "csv".to_string(),
          values: vec![
            ("content-types".to_string(), "text/csv;application/csv".to_string())
          ]
        },
        CatalogueEntry {
          entry_type: EntryType::ContentGenerator,
          key: "csv".to_string(),
          values: vec![
            ("content-types".to_string(), "text/csv;application/csv".to_string())
          ]
        }
    ])
  }
}

export_plugin_interface!(CsvPlugin);
