-- Credit card plugin written in Lua
--
-- This is the entry point loaded by the driver (see pact-plugin.json's "entryPoint"). Unlike the
-- JWT plugin next door, which owns a whole content type, this plugin contributes a single
-- matching rule and a single generator that apply to one *value* inside somebody else's content
-- - a field in a JSON body, a header, a message metadata value, and so on.
--
-- It defines the global functions the driver calls into:
--   init(implementation, version)  -> catalogue entries
--   match_field(request)           -> mismatches for one value
--   generate_field(request)        -> a replacement value
--
-- There is no configure_interaction/match_contents here: those belong to content matchers, and
-- this plugin never owns the content it appears in.
--
-- See docs/proposals/006_Field_level_matchers_and_generators.md for the design this implements.

local creditcard = require "creditcard"

-- Called once after the plugin script is loaded. Must return an array of catalogue entries to be
-- added to the global catalogue.
--
-- "MATCHER" and "GENERATOR" entries are named after the rule they provide: the key IS the name a
-- test writes, so `matching(creditcard, ...)` in a test resolves to the entry registered here.
-- Both entries deliberately share the key "creditcard" - the entry type is what tells them apart.
--
-- "config-key" names the values key that a single positional config argument in a matching rule
-- definition expression maps to, so `matching(creditcard, 'visa', '4111111111111111')` is stored
-- in the Pact file as { "match": "creditcard", "brand": "visa" }.
function init(implementation, version)
    logger("hello from the credit card plugin: " .. implementation .. ", " .. version)

    -- Add some entropy to the random number generator, used when generating card numbers
    math.randomseed(os.time())

    local params = { ["config-key"] = "brand" }
    return {
        { entryType = "MATCHER", key = "creditcard", values = params },
        { entryType = "GENERATOR", key = "creditcard", values = params }
    }
end

-- Field values arrive either as a plain Lua value or, for binary data, as a { binary = "..." }
-- wrapper - the same convention message metadata values already use. A card number is text, so a
-- binary value is always a mismatch rather than something to decode.
local function unwrap(value)
    if type(value) == "table" and value.binary ~= nil then
        return nil, "is binary data, expected a string of digits"
    end
    return value
end

local function display(value)
    if value == nil then
        return ""
    elseif type(value) == "table" then
        return "<binary>"
    else
        return tostring(value)
    end
end

-- Called to apply the "creditcard" matching rule to a single value.
--
-- `request` has:
--   key                   - the catalogue key ("creditcard")
--   rule                  - { type = "creditcard", values = { brand = "visa" } }
--   path                  - where the value lives, e.g. "$.card.number"
--   mismatch_type         - which part of the interaction it came from, e.g. "body"
--   expected              - the example value from the Pact file
--   actual                - the value received
--   plugin_configuration  - anything this plugin persisted into the Pact file (unused here)
--   test_context          - context data from the test framework
--
-- Returns { mismatches = { ... } } - an empty (or absent) list means the value matched - or
-- { error = "..." } if the rule itself could not be applied.
function match_field(request)
    local brand_key = (request.rule.values or {}).brand
    if brand_key ~= nil and creditcard.brands[brand_key] == nil then
        -- The rule is misconfigured, which is the test author's mistake rather than the
        -- provider's, so it's an error and not a mismatch.
        return { error = "'" .. tostring(brand_key) .. "' is not a credit card brand this plugin "
            .. "knows about. Known brands: " .. creditcard.known_brands() }
    end

    local function mismatch(description)
        return {
            mismatches = {
                {
                    expected = display(request.expected),
                    actual = display(request.actual),
                    mismatch = description,
                    path = request.path,
                    mismatch_type = request.mismatch_type
                }
            }
        }
    end

    local value, binary_error = unwrap(request.actual)
    if binary_error then
        return mismatch("Expected a credit card number, but the value " .. binary_error)
    end

    local digits, normalise_error = creditcard.normalise(value)
    if normalise_error then
        return mismatch("Expected a credit card number, but the value " .. normalise_error)
    end

    if not creditcard.luhn_valid(digits) then
        return mismatch("Expected a credit card number, but '" .. digits ..
            "' fails the Luhn check (its last digit is not a valid check digit)")
    end

    if brand_key then
        local matched, reason = creditcard.matches_brand(digits, brand_key)
        if not matched then
            return mismatch("Expected a " .. creditcard.brands[brand_key].name ..
                " credit card number, but '" .. digits .. "' " .. reason)
        end
    else
        -- No brand configured: any number that is well formed and passes the check digit is
        -- acceptable, whatever issuer it belongs to.
        if #digits < creditcard.min_length or #digits > creditcard.max_length then
            return mismatch(string.format(
                "Expected a credit card number, but '%s' has %d digits (credit card numbers have %d to %d)",
                digits, #digits, creditcard.min_length, creditcard.max_length))
        end
    end

    logger("'" .. digits .. "' at " .. tostring(request.path) .. " is a valid credit card number")
    return { mismatches = {} }
end

-- Called to generate a value for the "creditcard" generator, replacing the example value from
-- the Pact file with a fresh, valid one.
--
-- `request` has `key`, `generator` ({ type, values }), `path`, `example_value`,
-- `plugin_configuration`, `test_context` and `test_mode` ("Consumer"/"Provider"/"Unknown").
--
-- Returns { value = ... } or { error = "..." }.
--
-- This is a pure function of the request: the brand comes from the generator's own configuration,
-- falling back to whatever brand the example value in the Pact file looks like, and finally to
-- Visa. Nothing is read from anywhere else.
function generate_field(request)
    local brand_key = (request.generator.values or {}).brand

    if brand_key == nil then
        local value = unwrap(request.example_value)
        local digits = value ~= nil and creditcard.normalise(value) or nil
        brand_key = digits and creditcard.detect_brand(digits) or "visa"
    end

    local number, error_message = creditcard.generate(brand_key)
    if error_message then
        return { error = error_message .. ". Known brands: " .. creditcard.known_brands() }
    end

    logger("generated a " .. creditcard.brands[brand_key].name .. " number for " .. tostring(request.path))
    return { value = number }
end
