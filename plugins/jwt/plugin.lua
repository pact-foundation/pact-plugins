-- JWT plugin written in Lua

local jwt = require "jwt"
local json = require "json"

-- Init function is called after the plugin script is loaded. It needs to return the plugin catalog
-- entries to be added to the global catalog
function init(implementation, version)
    logger("hello from the JWT plugin: " .. implementation .. ", " .. version)

    -- Add some entropy to the random number generator
    math.randomseed(os.time())

    local params = { ["content-types"] = "application/jwt" }
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
    --[[ pub part_name: String,
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
        body = signed_token,
        plugin_config = plugin_config
    }

    -- (Vec<InteractionContents>, Option<PluginConfiguration>)
    return { contents = contents, plugin_config = plugin_config }
end
