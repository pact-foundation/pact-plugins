# CSV Plugin prototype

This is an example plugin supporting creating and matching comma-separated value text payloads.

## Building the plugin

The plugin is built with Rust. Just run `cargo build --release`. This will create the plugin binary file `pact-plugin-csv` 
in the `target/release/` directory.

## Installing the plugin

The plugin binary and [manifest file pact-plugin.json](pact-plugin.json) need to be copied into the `$HOME/.pact/plugins/csv-0.0.1` directory. You can download
those from the release for the plugin.

## Running with a development version of the plugin

If you build the plugin without the `--release`, this will create a debug version in the `target/debug` directory.
Copy the [manifest file pact-plugin.json](pact-plugin.json) into the `$HOME/.pact/plugins/csv-0.0.1` directory. If you
then edit that file, and set the `entryPoint` to the absolute path of the `pact-plugin-csv` binary in `target/debug`,
you can then make changes to the plugin, build it, and then all test projects will use that version.

## Example Projects

There are three example projects in [examples/csv](../../examples/csv) that use this plugin:

* csv-consumer-jvm - consumer written in Java
* csv-consumer-rust - consumer written in Rust
* csv-provider - provider written in Rust

## CSV matching definitions

The plugin matches the columns of the CSV data using matching rule definitions. The columns can be specified by
header (if the CSV has a header row) or by index (starting with 1).

Using the CSV from the example projects, it has 3 columns: Name, Number and Date. The matching rules can be specified by
(in pseudo config)

If using headers:
```javascript
"response.contents": {
  "pact:content-type": "text/csv",                               // Set the content type to CSV
  "csvHeaders": true,                                            // We have a header row
  "column:Name": "matching(type,'Name')",                        // Column with header Name must match by type (which is actually useless with CSV)
  "column:Number", "matching(number,100)",                       // Column with header Number must match a number format
  "column:Date", "matching(datetime, 'yyyy-MM-dd','2000-01-01')" // Column with header Date must match an ISO format yyyy-MM-dd
}
```

Without headers:
```javascript
"response.contents": {
  "pact:content-type": "text/csv",
  "csvHeaders": false,
  "column:1": "matching(type,'Name')",
  "column:2": "matching(number,100)",
  "column:3": "matching(datetime, 'yyyy-MM-dd','2000-01-01')"
}
```
