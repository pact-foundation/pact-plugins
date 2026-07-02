local utils = {}

utils.S = "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ"

function utils.random_str(count)
    local result = {}
    for i = 1, count do
        local index = math.random(1, string.len(utils.S))
        table.insert(result, string.sub(utils.S, index, index))
    end
    return table.concat(result)
end

function utils.random_hex(count)
    local result = {}
    for _ = 1, count do
        table.insert(result, string.format("%X", math.random(0, 15)))
    end
    return table.concat(result)
end

return utils
