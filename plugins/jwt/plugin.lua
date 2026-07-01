-- JWT plugin written in Lua
--
-- This is the entry point loaded by the driver (see pact-plugin.json's "entryPoint"). It
-- defines the global functions the driver calls into:
--   init(implementation, version)                -> catalogue entries
--   configure_interaction(content_type, config)   -> interaction contents + plugin config
--   match_contents(match_request)                 -> comparison result
--
-- generate_content and update_catalogue are optional; this plugin does not define them
-- (a single opaque token body has nothing to generate field-by-field, and there is no
-- state that needs to react to catalogue updates from other plugins).

local jwt = require "jwt"
local json = require "json"
local inspect = require "inspect"
local matching = require "matching"

-- Called once after the plugin script is loaded. Must return an array of catalogue entries
-- to be added to the global catalogue.
function init(implementation, version)
    logger("hello from the JWT plugin: " .. implementation .. ", " .. version)

    -- Add some entropy to the random number generator
    math.randomseed(os.time())

    -- The driver treats each semicolon-separated content type as a regex pattern when
    -- matching against an actual content type (a substring search in the Rust driver, a full
    -- match in the JVM driver), so the "+" in the "+json" structured syntax suffix must be
    -- escaped - otherwise it's interpreted as a regex quantifier and silently fails to match.
    local params = { ["content-types"] = "application/jwt;application/jwt\\+json" }
    local catalogue_entries = {}
    table.insert(catalogue_entries, { entryType = "CONTENT_MATCHER", key = "jwt", values = params })
    table.insert(catalogue_entries, { entryType = "CONTENT_GENERATOR", key = "jwt", values = params })
    return catalogue_entries
end

-- Called to set up the data for an interaction. `config` is the data supplied by the user
-- in the consumer test. Builds and signs a JWT from that data.
function configure_interaction(content_type, config)
    logger("Setting up interaction for " .. content_type)

    if not config["private-key"] then
        error("No private-key given. An RSA private key is required to create a signed JWT")
    end

    local private_key = config["private-key"]
    local public_key = config["public-key"] or rsa_public_key(private_key)

    local header = jwt.build_header(config)
    local payload = jwt.build_payload(config)

    local base64 = require "base64"
    local encoded_header = base64.encode(json.encode(header))
    local encoded_payload = base64.encode(json.encode(payload))
    local base_token = encoded_header .. "." .. encoded_payload

    local signature = jwt.sign_token(config, header, private_key, base_token)
    local signed_token = base_token .. "." .. signature

    -- Only the public key and algorithm are persisted to the Pact file: verification only
    -- ever needs to validate a token, never to mint one.
    local plugin_config = {
        interaction_configuration = {
            ["public-key"] = public_key,
            algorithm = header["alg"]
        }
    }

    return {
        interactions = {
            {
                contents = {
                    contents = signed_token,
                    content_type = "application/jwt+json",
                    -- Content type ends in "+json" (a structured syntax suffix), but a compact
                    -- JWT ("header.payload.signature") isn't itself JSON, so it must be hinted
                    -- as BINARY - otherwise implementations that special-case "+json" content
                    -- types try to parse the body as JSON when persisting/reading pact files.
                    content_type_hint = "BINARY"
                },
                part_name = "",
                plugin_config = plugin_config
            }
        },
        plugin_config = plugin_config
    }
end

-- Compares the actual JWT received against the expected one from the Pact interaction.
function match_contents(match_request)
    logger("Got a match request: " .. inspect(match_request))

    -- The JVM driver namespaces interaction_configuration under "request"/"response" keys
    -- when the same plugin configures both parts of one interaction (so the two configs
    -- don't collide); the Rust driver keeps it flat. Handle both: prefer "request" (this is
    -- always called to match a request or a response body, and our config is identical for
    -- both), falling back to "response" or the flat table.
    local interaction_config = match_request.plugin_configuration.interaction_configuration
    local part_config = interaction_config.request or interaction_config.response or interaction_config
    local public_key = part_config["public-key"]
    local algorithm = part_config["algorithm"]

    local expected_jwt, expected_error = jwt.decode_token(match_request.expected.contents)
    if expected_error then
        return { error = expected_error }
    end
    logger("Expected JWT: " .. inspect(expected_jwt))

    local actual_jwt, actual_error = jwt.decode_token(match_request.actual.contents)
    if actual_error then
        return { error = actual_error }
    end
    logger("Actual JWT: " .. inspect(actual_jwt))

    local mismatches = {}

    local token_issues = matching.validate_token(actual_jwt, algorithm, public_key)
    mismatches["$"] = token_issues

    local header_mismatches = matching.match_headers(expected_jwt.header, actual_jwt.header)
    for k, v in pairs(header_mismatches) do
        mismatches["header:" .. k] = v
    end

    local claim_mismatches = matching.match_claims(expected_jwt.payload, actual_jwt.payload)
    for k, v in pairs(claim_mismatches) do
        mismatches["claims:" .. k] = v
    end

    local result = { mismatches = mismatches }
    logger("returning match result " .. inspect(result))
    return result
end
