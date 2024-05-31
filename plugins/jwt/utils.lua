local utils = {}

function utils.random_str(count)
    local result = {}
    
    for _i, ch in random_str_iter(count) do
        table.insert(result, ch)
    end

    return table.concat(result)
end

function random_str_iter(count)
    return random_str_gen, count, 0
end

utils.S = "0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ"

function random_str_gen(count, index)
    if index < count then
        index = index + 1
        local i = math.random(string.len(utils.S))
        return index, string.sub(utils.S, i, i + 1)
    end
end

function utils.random_hex(count)
    local result = {}

    local i = 0
    while i < count do
        table.insert(result, string.format("%X", math.random(0, 16)))
        i = i + 1
    end

    return table.concat(result)
end

function utils.utf8_from(t)
    local bytearr = {}
    for _, v in ipairs(t) do
      local utf8byte = v < 0 and (0xff + v + 1) or v
      table.insert(bytearr, string.char(utf8byte))
    end
    return table.concat(bytearr)
end

return utils
