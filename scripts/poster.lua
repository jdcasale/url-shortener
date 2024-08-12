-- brew install wrk
-- wrk -t32 -c200 -d30s -s poster.lua http://127.0.0.1:8080/submit     
local random = math.random
math.randomseed(os.clock())
local function uuid()
    local template ='xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'
    return string.gsub(template, '[xy]', function (c)
        local v = (c == 'x') and random(0, 0xf) or random(8, 0xb)
        return string.format('https://www.google.com/%x', v)
    end)
end


wrk.method = "POST"
wrk.headers["Content-Type"] = "application/json"


request = function()
    local body = '{"long_url": "' .. tostring(uuid()) .. '"}'
    return wrk.format(nil, nil, nil, body)
end

