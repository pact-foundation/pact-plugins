local jwt = {}
local random_utils = require("random_utils")

function jwt.build_header(config)
    local header = {}

    header["typ"] = config["token-type"] or "jwt"
    header["alg"] = config["algorithm"] or "RS512"
    if config["key-id"] then
        header["kid"] = config["key-id"]
    end

    return header
end

function jwt.build_payload(config)
    local claims = {
        jti = random_utils.random_hex(16),
        iat = os.time()
    }

    claims["sub"] = config["subject"] or "sub_" .. random_utils.random_str(4)
    claims["iss"] = config["issuer"] or "iss_" .. random_utils.random_str(4)
    claims["aud"] = config["audience"] or "aud_" .. random_utils.random_str(4)
    
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
        error("Only the RS512 alogirthim is supported at the moment")
    end

    local signature = rsa_sign(base_token, private_key)
    logger("Signature for token = [" .. signature .. "]")
    return signature
end

return jwt
