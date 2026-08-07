local jwt = {}
local utils = require "utils"
local json = require "json"
local inspect = require "inspect"

function jwt.build_header(config)
    local header = {}

    header["typ"] = config["token-type"] or "JWT"
    header["alg"] = config["algorithm"] or "RS512"
    if config["key-id"] then
        header["kid"] = config["key-id"]
    end

    return header
end

-- A claim value can be given either directly, or as an integration-JSON matcher table:
--
--   customer_id = { ["pact:matcher:type"] = "regex", regex = "CUST-%d+", value = "CUST-123456" }
--
-- in which case the example value goes into the token and the rule is returned separately, to be
-- persisted with the interaction and applied when the token is matched (see plugin.lua). Returns
-- the value to use, and the rule table (or nil when the value was given directly).
local function claim_value_and_rule(value)
    if type(value) == "table" and value["pact:matcher:type"] then
        local values = {}
        for k, v in pairs(value) do
            if k ~= "pact:matcher:type" and k ~= "value" then
                values[k] = v
            end
        end
        return value["value"], { type = value["pact:matcher:type"], values = values }
    end
    return value, nil
end

-- Builds the token payload from the test's configuration. Returns the claims, and the matching
-- rules any of them were declared with, keyed by claim name.
function jwt.build_payload(config)
    local claims = {
        jti = utils.random_hex(16),
        iat = os.time()
    }
    local rules = {}

    local function set_claim(claim, configured, default)
        local value, rule = claim_value_and_rule(configured)
        claims[claim] = value or default
        if rule then
            rules[claim] = rule
        end
    end

    set_claim("sub", config["subject"], "sub_" .. utils.random_str(4))
    set_claim("iss", config["issuer"], "iss_" .. utils.random_str(4))
    set_claim("aud", config["audience"], "aud_" .. utils.random_str(4))

    -- exp: now + expiryInMinutes * 60, // Current time + STS_TOKEN_EXPIRY_MINUTES minutes
    claims["exp"] = os.time() + 5 * 60

    config["subject"] = nil
    config["issuer"] = nil
    config["audience"] = nil
    config["token-type"] = nil
    config["algorithm"] = nil
    config["key-id"] = nil
    config["private-key"] = nil
    config["public-key"] = nil
    for k, v in pairs(config) do
        if v then
            set_claim(k, v)
        end
    end

    return claims, rules
end

function jwt.sign_token(config, header, private_key, base_token)
    if header["alg"] ~= "RS512" then
        logger("Signature algorithm is set to " .. header["alg"])
        error("Only the RS512 algorithm is supported at the moment")
    end

    local signature = rsa_sign(base_token, private_key)
    logger("Signature for token = [" .. signature .. "]")
    return signature
end

-- Decodes a signed JWT (header.payload.signature). `contents` is the raw token as a Lua
-- string (the driver hands over body content as a native binary-safe Lua string, not a
-- byte array).
function jwt.decode_token(contents)
    logger("Encoded token = " .. contents)
    local t = {}
    for str in string.gmatch(contents, "([^\\.]+)") do
        table.insert(t, str)
    end
    if #t ~= 3 then
        return nil, "Not a valid JWT: expected 3 parts (header.payload.signature), got " .. #t
    end

    local header = b64_decode_no_pad(t[1])
    logger("Token header = " .. inspect(header))
    local payload = b64_decode_no_pad(t[2])
    logger("Token payload = " .. inspect(payload))
    local signature = t[3]
    logger("Token signature = " .. signature)

    return { header = json.decode(header), payload = json.decode(payload), signature = signature, encoded = contents }, nil
end

function jwt.validate_signature(token, algorithm, key)
    local parts = {}
    for str in string.gmatch(token, "([^\\.]+)") do
        table.insert(parts, str)
    end

    if algorithm ~= "RS512" then
        logger("Signature algorithm is set to " .. algorithm)
        return false, "Only the RS512 algorithm is supported at the moment"
    end

    return rsa_validate(parts, algorithm, key)
end

return jwt
