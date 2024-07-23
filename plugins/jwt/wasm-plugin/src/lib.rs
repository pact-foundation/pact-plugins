wit_bindgen::generate!();

struct JwtPlugin {

}

export!(JwtPlugin);

impl Guest for JwtPlugin {
    fn init(implementation: String, version: String) -> Vec<CatalogueEntry> {
        log(format!("hello from the JWT plugin: {}, {}", implementation, version).as_str());

        vec![
            CatalogueEntry {
                entry_type: EntryType::ContentMatcher,
                key: "jwt".to_string(),
                values: vec![("content-types".to_string(), "application/jwt;application/jwt+json".to_string())]
            },
            CatalogueEntry {
                entry_type: EntryType::ContentGenerator,
                key: "jwt".to_string(),
                values: vec![("content-types".to_string(), "application/jwt;application/jwt+json".to_string())]
            }
        ]
    }

    fn update_catalogue(_catalogue: Vec<CatalogueEntry>) {
        // no-op
    }
}

#[cfg(test)]
mod tests {
    use expectest::prelude::*;
    use super::*;

    #[test]
    fn it_works() {
        let result = JwtPlugin::init("".to_string(), "".to_string());
        expect!(result.len()).to(be_equal_to(2));
    }
}
