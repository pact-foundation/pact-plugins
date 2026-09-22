# Avro Area Calculator Example

This example demonstrates Pact contract testing with the [pact-avro-plugin](https://github.com/ZirekHQ/pact-avro-plugin)
using a simple area calculator scenario:

- A **producer** calculates areas of shapes and publishes `AreaResponse` messages in Avro binary format.
- A **JVM consumer** (Groovy/JUnit 5) and a **Node.js consumer** receive and decode those messages.

The Pact contracts ensure the producer's output always satisfies what each consumer expects.

## Schema

The Avro schemas are in [`schema/`](schema/):

| File | Record | Description |
|------|--------|-------------|
| `area_request.avsc` | `AreaRequest` | Shape to calculate (rectangle, circle, square) |
| `area_response.avsc` | `AreaResponse` | Calculated area (`shape` string + `value` float) |

## Prerequisites

### Install the Avro plugin (Rust native build)

The example requires the Rust port of the Avro plugin from the `worktree-rust-plugin-foundation` branch.

```sh
# Clone and build
git clone https://github.com/ZirekHQ/pact-avro-plugin.git
cd pact-avro-plugin
git checkout worktree-rust-plugin-foundation

cd modules/plugin-rs
cargo build --release

# Install manually into ~/.pact/plugins/avro-<version>/
PLUGIN_VERSION=$(cargo pkgid | cut -d'#' -f2)
PLUGIN_DIR="$HOME/.pact/plugins/avro-${PLUGIN_VERSION}"
mkdir -p "$PLUGIN_DIR"
cp target/release/pact-avro-plugin "$PLUGIN_DIR/"

# Write the plugin manifest
cat > "$PLUGIN_DIR/pact-plugin.json" <<EOF
{
  "manifestVersion": 1,
  "pluginInterfaceVersion": 1,
  "name": "avro",
  "version": "${PLUGIN_VERSION}",
  "executableType": "exec",
  "entryPoint": "pact-avro-plugin"
}
EOF
```

Verify the install:

```sh
ls ~/.pact/plugins/
# avro-0.1.0-dev/   (or whatever the current version is)
```

### JVM consumer and producer

- Java 17+
- Gradle (via the wrapper in each project, or `./gradlew`)

### Node consumer

- Node.js 18+
- npm

## Running the tests

### Step 1 — JVM consumer (generates pact files)

```sh
cd consumer-jvm
./gradlew :lib:test
```

Pact files are written to `../pacts/`:

```
pacts/
  area-calculator-jvm-consumer-area-calculator-producer.json
```

### Step 2 — Node consumer (generates additional pact files)

```sh
cd consumer-node
npm install
npm test
```

Pact files are written to `../pacts/`:

```
pacts/
  area-calculator-node-consumer-area-calculator-producer.json
```

### Step 3 — Producer verification (verifies all consumer pacts)

Run after both consumer test suites have generated their pact files.

```sh
cd producer
./gradlew :lib:test
```

The producer loads all `*.json` files from `../pacts/` and verifies it can produce
messages satisfying every consumer's expectations.

## Project layout

```
area_calculator/
├── schema/
│   ├── area_request.avsc       # AreaRequest Avro record
│   └── area_response.avsc      # AreaResponse Avro record
├── consumer-jvm/               # Groovy/JUnit 5 consumer
│   └── lib/
│       └── src/
│           ├── main/groovy/consumer/avro/
│           │   ├── AreaCalculatorClient.groovy
│           │   └── AreaResult.groovy
│           └── test/groovy/consumer/avro/
│               └── AreaCalculatorConsumerTest.groovy
├── consumer-node/              # Node.js/Jest consumer
│   └── src/
│       ├── area_calculator_client.js
│       └── area_calculator.consumer.test.js
├── producer/                   # Groovy/JUnit 5 producer (provider verification)
│   └── lib/
│       └── src/
│           ├── main/groovy/producer/avro/
│           │   └── AreaCalculatorProducer.groovy
│           └── test/groovy/producer/avro/
│               └── AreaCalculatorProducerTest.groovy
└── pacts/                      # Generated pact files (created by consumer tests)
```

## How it works

The Pact framework invokes the avro plugin at test time for:

1. **Consumer tests** — the plugin generates a sample Avro binary message matching the
   declared field constraints (`notEmpty`, `matching(decimal, ...)`, etc.) and hands the
   bytes to the test for validation. A pact file is saved recording the interaction.

2. **Producer tests** — the plugin deserialises the bytes produced by
   `@PactVerifyProvider` methods and checks they satisfy the matchers from the saved
   pact file.

Plugin config keys used in the interaction:

| Key | Value |
|-----|-------|
| `pact:avro` | Absolute path to the `.avsc` schema file |
| `pact:record-name` | Top-level Avro record name to encode/decode |
| `pact:content-type` | `avro/binary` |
| field keys | Pact matcher expressions (`notEmpty(...)`, `matching(decimal, ...)`, etc.) |
