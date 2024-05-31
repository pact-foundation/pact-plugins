local jwt = require "jwt"
local inspect = require "inspect"

local matching = {}

function matching.validate_token(token, algorithm, key)
    local result = {}

    local signature_valid = jwt.validate_signature(token.encoded, algorithm, key)
    if not signature_valid then
        table.insert(result, "Actual token signature is not valid")
    end

    local expiration_time = token.payload["exp"]
    if expiration_time < os.time() then
        table.insert(result, "Actual token has expired")
    end

    local not_before_time = token.payload["nbf"]
    if not_before_time and not_before_time > os.time() then
        table.insert(result, "Actual token is not to be used yet")
    end

    return result
end

function matching.match_headers(expected_header, actual_header)
    logger("matching JWT headers")
    logger("expected headers: " .. inspect(expected_header))
    logger("actual headers: " .. inspect(actual_header))
    return match_map(expected_header, actual_header, Set({"typ", "alg"}), 
        Set({"alg", "jku", "jwk", "kid", "x5u", "x5c", "x5t", "x5t#S256", "typ", "cty", "crit"}), Set({"jku"}))
end

function matching.match_claims(expected_claims, actual_claims)
    logger("matching JWT claims")
    logger("expected claims: " .. inspect(expected_claims))
    logger("actual claims: " .. inspect(actual_claims))
    return match_map(expected_claims, actual_claims, Set({"iss", "sub", "aud", "exp"}), {}, Set({"exp", "nbf", "iat", "jti"}))
end

function match_map(expected, actual, compulsary_keys, allowed_keys, keys_to_ignore)
    local result = {}

    for k, v in pairs(expected) do
        if not keys_to_ignore[k] then 
            if actual[k] ~= v then 
                result[k] = {
                    expected = v,
                    actual = actual[k],
                    mismatch = "Expected value " .. inspect(v) .. " but got value " .. inspect(actual[k]),
                    path = k
                }
            end
        end
    end

    local allowed_keys_empty = next(allowed_keys) == nil
    for k, v in pairs(actual) do
        if not allowed_keys_empty and not allowed_keys[k] then
            result[k] = {
                actual = v,
                mismatch = k .. " is not allowed as a key",
                path = k
            }
        end
    end

    for k, v in pairs(compulsary_keys) do
        if not actual[k] then 
            result[k] = {
                mismatch = k .. " is a compulsary key, but was missing",
                path = k
            }
        end
    end

    return result
end

function Set(list)
    local set = {}
    for _, l in ipairs(list) do set[l] = true end
    return set
end

return matching
