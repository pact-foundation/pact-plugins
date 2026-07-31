-- Credit card number helpers: normalisation, Luhn checksum, brand rules and generation.
--
-- Nothing in here knows anything about Pact - plugin.lua does all the talking to the driver.

local creditcard = {}

-- Issuer identification number (IIN) rules per brand.
--
--   prefixes  - Lua patterns matched against the start of the number
--   ranges    - { min, max } pairs matched against the first 4 digits as a number, for brands
--               whose IIN ranges don't fall on a clean prefix boundary
--   lengths   - permitted total digit counts
--   seeds     - literal prefixes used when *generating* a number for this brand (the patterns
--               above can't be used for that - "^5[1-5]" isn't a number)
--
-- These are the commonly published ranges rather than an exhaustive registry: a test double
-- needs to produce and recognise plausible numbers, not to be an authoritative BIN database.
creditcard.brands = {
    visa = {
        name = "Visa",
        prefixes = { "^4" },
        lengths = { 13, 16, 19 },
        seeds = { "4" }
    },
    mastercard = {
        name = "Mastercard",
        prefixes = { "^5[1-5]" },
        ranges = { { 2221, 2720 } },
        lengths = { 16 },
        seeds = { "51", "52", "53", "54", "55", "2221", "2500", "2720" }
    },
    amex = {
        name = "American Express",
        prefixes = { "^34", "^37" },
        lengths = { 15 },
        seeds = { "34", "37" }
    },
    discover = {
        name = "Discover",
        prefixes = { "^6011", "^64[4-9]", "^65" },
        lengths = { 16, 19 },
        seeds = { "6011", "644", "65" }
    },
    jcb = {
        name = "JCB",
        prefixes = { "^35" },
        lengths = { 16, 19 },
        seeds = { "3528", "3589" }
    },
    diners = {
        name = "Diners Club",
        prefixes = { "^30[0-5]", "^36", "^38" },
        lengths = { 14, 16, 19 },
        seeds = { "300", "36", "38" }
    }
}

-- Brand detection has to be order-dependent, and `pairs` order over a Lua table is not stable,
-- so keep an explicit order. Most specific ranges first.
creditcard.brand_order = { "amex", "diners", "discover", "jcb", "mastercard", "visa" }

-- Shortest and longest number accepted when no brand is configured (ISO/IEC 7812 allows up
-- to 19 digits; 12 is the shortest number in real-world use).
creditcard.min_length = 12
creditcard.max_length = 19

--- Convert a value from a Pact interaction into a plain digit string.
-- Accepts a string (with spaces and dashes as separators, as cards are usually written) or an
-- integer. Returns nil plus a description of the problem if it isn't one.
function creditcard.normalise(value)
    local as_string
    if type(value) == "string" then
        as_string = value
    elseif type(value) == "number" then
        if math.type(value) ~= "integer" then
            return nil, "is not a whole number"
        end
        as_string = string.format("%d", value)
    else
        return nil, "is a " .. type(value) .. ", expected a string of digits"
    end

    local digits = as_string:gsub("[%s%-]", "")
    if digits == "" then
        return nil, "is empty"
    end
    if not digits:match("^%d+$") then
        return nil, "contains characters that are not digits (after removing spaces and dashes)"
    end
    return digits
end

-- Sum of the digits under the Luhn doubling rule, working right to left.
local function luhn_sum(digits)
    local sum = 0
    local double = false
    for i = #digits, 1, -1 do
        local digit = tonumber(digits:sub(i, i))
        if double then
            digit = digit * 2
            if digit > 9 then
                digit = digit - 9
            end
        end
        sum = sum + digit
        double = not double
    end
    return sum
end

--- Is the number's final digit a valid Luhn check digit for the digits preceding it?
function creditcard.luhn_valid(digits)
    return #digits > 0 and luhn_sum(digits) % 10 == 0
end

--- The check digit to append to a number that doesn't have one yet.
function creditcard.luhn_check_digit(partial)
    -- Appending a digit flips the doubling parity of every digit already there, so compute the
    -- sum of the number as it will be once a placeholder digit is on the end.
    return (10 - (luhn_sum(partial .. "0") % 10)) % 10
end

--- Does the number look like it was issued under `brand_key`?
-- Returns true, or false plus the reason it didn't match.
function creditcard.matches_brand(digits, brand_key)
    local brand = creditcard.brands[brand_key]
    if not brand then
        return false, "'" .. tostring(brand_key) .. "' is not a brand this plugin knows about"
    end

    local prefix_matched = false
    for _, pattern in ipairs(brand.prefixes or {}) do
        if digits:match(pattern) then
            prefix_matched = true
            break
        end
    end
    if not prefix_matched and brand.ranges then
        local leading = tonumber(digits:sub(1, 4))
        for _, range in ipairs(brand.ranges) do
            if leading and leading >= range[1] and leading <= range[2] then
                prefix_matched = true
                break
            end
        end
    end
    if not prefix_matched then
        return false, "does not start with an issuer identification number used by " .. brand.name
    end

    for _, length in ipairs(brand.lengths) do
        if #digits == length then
            return true
        end
    end
    return false, string.format("has %d digits, but %s numbers have %s",
        #digits, brand.name, table.concat(brand.lengths, " or "))
end

--- The brand a number appears to belong to, or nil if it doesn't look like any known brand.
function creditcard.detect_brand(digits)
    for _, brand_key in ipairs(creditcard.brand_order) do
        if creditcard.matches_brand(digits, brand_key) then
            return brand_key
        end
    end
    return nil
end

--- Generate a fresh, Luhn-valid number for the given brand.
function creditcard.generate(brand_key)
    local brand = creditcard.brands[brand_key]
    if not brand then
        return nil, "'" .. tostring(brand_key) .. "' is not a brand this plugin knows about"
    end

    local prefix = brand.seeds[math.random(#brand.seeds)]
    local length = brand.lengths[math.random(#brand.lengths)]

    local digits = prefix
    while #digits < length - 1 do
        digits = digits .. tostring(math.random(0, 9))
    end
    return digits .. tostring(creditcard.luhn_check_digit(digits))
end

--- The list of brands this plugin knows, for error messages.
function creditcard.known_brands()
    local names = {}
    for _, brand_key in ipairs(creditcard.brand_order) do
        table.insert(names, brand_key)
    end
    table.sort(names)
    return table.concat(names, ", ")
end

return creditcard
