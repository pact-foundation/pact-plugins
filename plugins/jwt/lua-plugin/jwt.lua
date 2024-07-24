local jwt = {}
local utils = require "utils"
local base64 = require "base64"
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

function jwt.build_payload(config)
    local claims = {
        jti = utils.random_hex(16),
        iat = os.time()
    }

    claims["sub"] = config["subject"] or "sub_" .. utils.random_str(4)
    claims["iss"] = config["issuer"] or "iss_" .. utils.random_str(4)
    claims["aud"] = config["audience"] or "aud_" .. utils.random_str(4)
    
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
            claims[k] = v
        end
    end

    return claims
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

function jwt.decode_token(contents)
    local encoded_string = utils.utf8_from(contents)
    logger("Encoded token = " .. encoded_string)
    local t = {}
    for str in string.gmatch(encoded_string, "([^\\.]+)") do
        table.insert(t, str)
    end
    local header = utils.utf8_from(b64_decode_no_pad(t[1]))
    logger("Token header = " .. inspect(header))
    local payload = utils.utf8_from(b64_decode_no_pad(t[2]))
    logger("Token payload = " .. inspect(payload))
    local signature = t[3]
    logger("Token signature = " .. signature)

    return { header = json.decode(header), payload = json.decode(payload), signature = signature, encoded = encoded_string }, nil
end

function jwt.validate_signature(token, algorithm, key)
    local parts = {}
    for str in string.gmatch(token, "([^\\.]+)") do
        table.insert(parts, str)
    end

    if algorithm ~= "RS512" then
        logger("Signature algorithm is set to " .. algorithm)
        return false, "Only the RS512 alogirthim is supported at the moment"
    end

    return rsa_validate(parts, algorithm, key)
end

return jwt
