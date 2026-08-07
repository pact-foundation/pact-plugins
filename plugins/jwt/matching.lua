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
    if expiration_time and expiration_time < os.time() then
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

function matching.match_claims(expected_claims, actual_claims, claim_rules)
    logger("matching JWT claims")
    logger("expected claims: " .. inspect(expected_claims))
    logger("actual claims: " .. inspect(actual_claims))
    -- "exp" is deliberately not compulsory here (unlike iss/sub/aud): it's a timestamp that
    -- legitimately differs between the expected and actual token, and its presence/validity
    -- is already checked semantically by validate_token above.
    return match_map(expected_claims, actual_claims, Set({"iss", "sub", "aud"}),
        {}, Set({"exp", "nbf", "iat", "jti"}), claim_rules)
end

-- Applies a matching rule to a single claim by calling back into the host Pact framework, rather
-- than reimplementing the rule here. This plugin knows how to decode and validate a JWT; it has no
-- business owning what "regex" or "date" mean, and the host already has the correct implementation
-- of both. See proposals 006 (field-level matchers) and 009 (host-provided core matching).
--
-- Returns a mismatch table, or nil if the claim matched.
function match_claim_with_rule(key, rule, expected_value, actual_value)
    if type(host_match_field) ~= "function" then
        return {
            expected = expected_value,
            actual = actual_value,
            path = key,
            mismatch = "The '" .. rule.type .. "' matching rule was declared on claim '" .. key ..
                "', but this driver does not provide the host_match_field function"
        }
    end

    logger("Delegating claim '" .. key .. "' to the host's '" .. rule.type .. "' matching rule")
    -- The rule name is the catalogue key, so the type a rule arrives with is directly usable
    local result = host_match_field(rule.type, {
        rule = rule,
        path = "$." .. key,
        mismatch_type = "body",
        expected = expected_value,
        actual = actual_value
    })

    if result.error then
        return {
            expected = expected_value,
            actual = actual_value,
            path = key,
            mismatch = "Could not apply the '" .. rule.type .. "' matching rule to claim '" ..
                key .. "' - " .. result.error
        }
    end

    local mismatches = result.mismatches or {}
    if #mismatches == 0 then
        return nil
    end

    local descriptions = {}
    for _, mismatch in ipairs(mismatches) do
        table.insert(descriptions, mismatch.mismatch or inspect(mismatch))
    end
    return {
        expected = expected_value,
        actual = actual_value,
        path = key,
        mismatch = table.concat(descriptions, ", ")
    }
end

function match_map(expected, actual, compulsory_keys, allowed_keys, keys_to_ignore, rules)
    local result = {}
    rules = rules or {}

    for k, v in pairs(expected) do
        local rule = rules[k]
        if rule then
            -- A rule wins over the ignore list: a claim that normally varies freely between the
            -- two tokens (exp, iat) is being constrained deliberately if a test names it
            local mismatch = match_claim_with_rule(k, rule, v, actual[k])
            if mismatch then
                result[k] = mismatch
            end
        elseif not keys_to_ignore[k] then
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

    for k, v in pairs(compulsory_keys) do
        if not actual[k] then
            result[k] = {
                mismatch = k .. " is a compulsory key, but was missing",
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
