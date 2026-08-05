-- Standalone test for the credit card plugin. Run it with a Lua 5.4 interpreter from this
-- directory:
--
--   $ lua5.4 test.lua
--
-- It stands in for the driver: it stubs the `logger` host function, loads plugin.lua, and calls
-- the plugin's global functions with the same table shapes the driver builds. Field-level
-- matchers aren't implemented in either driver yet (see
-- ../../docs/proposals/006_Field_level_matchers_and_generators.md), so until they are, this is
-- the only way to exercise the plugin at all.

local script_dir = arg[0]:match("^(.*)[/\\][^/\\]*$") or "."
package.path = script_dir .. "/?.lua;" .. package.path

-- Host functions the driver registers as globals before loading a plugin script.
function logger(_) end

dofile(script_dir .. "/plugin.lua")

local creditcard = require "creditcard"

local failures = 0
local function check(description, ok, detail)
    if ok then
        print("ok   - " .. description)
    else
        failures = failures + 1
        print("FAIL - " .. description .. (detail and (" : " .. detail) or ""))
    end
end

-- Build a match_field request the way the driver would.
local function match(actual, brand, expected)
    return match_field({
        key = "creditcard",
        rule = { type = "creditcard", values = brand and { brand = brand } or {} },
        path = "$.card.number",
        mismatch_type = "body",
        expected = expected or "4111111111111111",
        actual = actual
    })
end

local function matched(result)
    return result.error == nil and (result.mismatches == nil or #result.mismatches == 0)
end

local function generate(brand, example)
    return generate_field({
        key = "creditcard",
        generator = { type = "creditcard", values = brand and { brand = brand } or {} },
        path = "$.card.number",
        example_value = example,
        test_mode = "Consumer"
    })
end

local function first_mismatch(result)
    return result.mismatches and result.mismatches[1] and result.mismatches[1].mismatch or "<none>"
end

-- init --------------------------------------------------------------------------------------

local entries = init("test-harness", "0.0.0")
check("init returns two catalogue entries", #entries == 2, tostring(#entries))
check("init registers a MATCHER named creditcard",
    entries[1].entryType == "MATCHER" and entries[1].key == "creditcard")
check("init registers a GENERATOR named creditcard",
    entries[2].entryType == "GENERATOR" and entries[2].key == "creditcard")
check("init declares brand as the positional config key",
    entries[1].values["config-key"] == "brand" and entries[2].values["config-key"] == "brand")

-- matching valid numbers --------------------------------------------------------------------

-- The publicly published test numbers each card scheme provides. All are Luhn valid.
local test_numbers = {
    { "4111111111111111", "visa" },
    { "4012888888881881", "visa" },
    { "5555555555554444", "mastercard" },
    { "2223003122003222", "mastercard" },
    { "378282246310005",  "amex" },
    { "6011111111111117", "discover" },
    { "3530111333300000", "jcb" },
    { "36227206271667",   "diners" }
}
for _, case in ipairs(test_numbers) do
    local number, brand = case[1], case[2]
    check("matches " .. number .. " with no brand configured", matched(match(number)),
        first_mismatch(match(number)))
    check("matches " .. number .. " configured as " .. brand, matched(match(number, brand)),
        first_mismatch(match(number, brand)))
    check("detects " .. number .. " as " .. brand, creditcard.detect_brand(number) == brand,
        tostring(creditcard.detect_brand(number)))
end

check("tolerates spaces and dashes between digit groups", matched(match("4111-1111 1111-1111")))
check("accepts a number supplied as a JSON integer", matched(match(4111111111111111)))

-- mismatches ---------------------------------------------------------------------------------

local bad_luhn = match("4111111111111112")
check("rejects a number with a bad check digit", not matched(bad_luhn))
check("says so in the mismatch description", first_mismatch(bad_luhn):find("Luhn") ~= nil,
    first_mismatch(bad_luhn))
check("reports the mismatch against the request's path", bad_luhn.mismatches[1].path == "$.card.number")
check("reports the part of the interaction it came from", bad_luhn.mismatches[1].mismatch_type == "body")
check("reports both the expected and the actual value",
    bad_luhn.mismatches[1].expected == "4111111111111111"
    and bad_luhn.mismatches[1].actual == "4111111111111112")

check("rejects a value that isn't digits", not matched(match("not a card")))
check("rejects an empty value", not matched(match("")))
check("rejects a boolean", not matched(match(true)))
check("rejects a non-integer number", not matched(match(4111111111111111.5)))
check("rejects binary data", not matched(match({ binary = "\1\2\3" })))
check("rejects a number that is too short", not matched(match("4111111111")))

local wrong_brand = match("5555555555554444", "visa")
check("rejects a valid number of the wrong brand", not matched(wrong_brand))
check("names the configured brand in the mismatch", first_mismatch(wrong_brand):find("Visa") ~= nil,
    first_mismatch(wrong_brand))

local short_visa = match("4111111111111", "amex")
check("rejects a number of the wrong length for its brand", not matched(short_visa))

-- a misconfigured rule is the test author's mistake, so an error rather than a mismatch
local unknown_brand = match("4111111111111111", "amx")
check("treats an unknown configured brand as an error, not a mismatch",
    unknown_brand.error ~= nil and unknown_brand.mismatches == nil, unknown_brand.error)

-- generation ----------------------------------------------------------------------------------

for _, brand in ipairs(creditcard.brand_order) do
    local result = generate(brand)
    check("generates a " .. brand .. " number", result.error == nil and result.value ~= nil, result.error)
    if result.value then
        check("the generated " .. brand .. " number (" .. result.value .. ") passes its own matcher",
            matched(match(result.value, brand)), first_mismatch(match(result.value, brand)))
    end
end

local from_example = generate(nil, "378282246310005")
check("takes the brand from the example value when not configured",
    from_example.value ~= nil and creditcard.detect_brand(from_example.value) == "amex",
    tostring(from_example.value))

local no_example = generate(nil, nil)
check("falls back to Visa with neither configuration nor an example",
    no_example.value ~= nil and creditcard.detect_brand(no_example.value) == "visa",
    tostring(no_example.value))

local first, second = generate("visa").value, generate("visa").value
check("generates a different number on each call", first ~= second, first .. " / " .. second)

check("treats an unknown brand for generation as an error", generate("nope").error ~= nil)

-- ----------------------------------------------------------------------------------------------

print("")
if failures == 0 then
    print("All checks passed")
else
    print(failures .. " check(s) failed")
    os.exit(1)
end
