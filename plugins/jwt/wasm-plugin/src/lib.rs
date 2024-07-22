wit_bindgen::generate!();

struct JwtPlugin {

}

export!(JwtPlugin);

impl Guest for JwtPlugin {
    fn init(implementation: String, version: String) -> Vec<CatalogueEntry> {
        // logger("hello from the JWT plugin: " .. implementation .. ", " .. version)

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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = 4; //add(2, 2);
        assert_eq!(result, 4);
    }
}
