-- JWT plugin written in Lua

local jwt = require "jwt"
local json = require "json"
local inspect = require "inspect"
local matching = require "matching"

-- Init function is called after the plugin script is loaded. It needs to return the plugin catalog
-- entries to be added to the global catalog
function init(implementation, version)
    logger("hello from the JWT plugin: " .. implementation .. ", " .. version)

    -- Add some entropy to the random number generator
    math.randomseed(os.time())

    local params = { ["content-types"] = "application/jwt;application/jwt+json" }
    local catalogue_entries = {}
    catalogue_entries[0] = { entryType="CONTENT_MATCHER", providerType="PLUGIN", key="jwt", values=params }
    catalogue_entries[1] = { entryType="CONTENT_GENERATOR", providerType="PLUGIN", key="jwt", values=params }
    return catalogue_entries
end

-- Use to setup the data for an interaction. The config is the data supplied by the user test.
-- In this case, we use the data to create a JWT.
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

    --[[
        /// Data to persist on the interaction
        pub interaction_configuration: HashMap<String, Value>,
        /// Data to persist in the Pact metadata
        pub pact_configuration: HashMap<String, Value>
    --]]
    local plugin_config = {
        interaction_configuration = {
            ["public-key"] = public_key,
            algorithm = header["alg"]
        },
        pact_configuration = {}
    }

    local contents = {}
    --[[ 
        /// Description of what part this interaction belongs to (in the case of there being more than
        /// one, for instance, request/response messages)
        pub part_name: String,

        /// Body/Contents of the interaction
        pub body: OptionalBody,

        /// Matching rules to apply
        pub rules: Option<MatchingRuleCategory>,

        /// Generators to apply
        pub generators: Option<Generators>,

        /// Message metadata
        pub metadata: Option<HashMap<String, Value>>,

        /// Matching rules to apply to message metadata
        pub metadata_rules: Option<MatchingRuleCategory>,

        /// Plugin configuration data to apply to the interaction
        pub plugin_config: PluginConfiguration,

        /// Markup for the interaction to display in any UI
        pub interaction_markup: String,

        /// The type of the markup (CommonMark or HTML)
        pub interaction_markup_type: String
    --]]
    contents[0] = {
        body = {
            contents = signed_token,
            content_type = "application/jwt+json",
            content_type_hint = "TEXT"
        },
        plugin_config = plugin_config
    }

    -- (Vec<InteractionContents>, Option<PluginConfiguration>)
    return { contents = contents, plugin_config = plugin_config }
end

-- This function does the actual matching
function match_contents(match_request)
    --[[
    /// The expected contents from the Pact interaction
    pub expected_contents: OptionalBody,
    /// The actual contents that was received
    pub actual_contents: OptionalBody,
    /// Where there are keys or attributes in the data, indicates whether unexpected values are allowed
    pub allow_unexpected_keys: bool,
    /// Matching rules that apply
    pub matching_rules: HashMap<DocPath, RuleList>,
    /// Plugin configuration form the Pact
    pub plugin_configuration: Option<PluginInteractionConfig>
    --]]
    logger("Got a match request: " .. inspect(match_request))

    local public_key = match_request.plugin_configuration.interaction_configuration["public-key"]
    local algorithm = match_request.plugin_configuration.interaction_configuration["algorithm"]

    local expected_jwt, error = jwt.decode_token(match_request.expected_contents.contents)
    if error then
        return { error = error }
    end
    logger("Expected JWT: " .. inspect(expected_jwt))

    local actual_jwt, actual_error = jwt.decode_token(match_request.actual_contents.contents)
    if actual_error then
        return { error = actual_error }
    end
    logger("Actual JWT: " .. inspect(actual_jwt))

    --[[
    /// An error occurred trying to compare the contents
    Error(String),
    /// The content type was incorrect
    TypeMismatch(String, String),
    /// There were mismatched results
    Mismatches(HashMap<String, Vec<ContentMismatch>>),
    /// All OK
    OK
    --]]

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
