-- Agent Bootstrap Widget
-- Hands control of configured players to AgentBridge AI in multiplayer games.
-- Config: LuaUI/Config/agent_bootstrap.json

function widget:GetInfo()
    return {
        name    = "Agent Bootstrap",
        desc    = "Hands control of configured players to AgentBridge AI",
        author  = "afcomech",
        version = "0.1",
        date    = "2026",
        license = "MIT",
        layer   = 0,
        enabled = true,
    }
end

local JSON = VFS.Include("LuaUI/Utilities/json.lua", nil, VFS.RAW_FIRST) or {
    decode = function(s) return loadstring("return " .. s)() end,
}

local CONFIG_PATH = "LuaUI/Config/agent_bootstrap.json"
local config = nil

function widget:Initialize()
    local raw = VFS.LoadFile(CONFIG_PATH, VFS.RAW_FIRST)
    if not raw then
        Spring.Log("AgentBootstrap", LOG.INFO, "No config at " .. CONFIG_PATH .. ", widget inactive")
        widgetHandler:RemoveWidget(self)
        return
    end

    local ok, parsed = pcall(JSON.decode, raw)
    if not ok or not parsed or not parsed.players then
        Spring.Log("AgentBootstrap", LOG.WARNING, "Bad agent_bootstrap.json, widget inactive")
        widgetHandler:RemoveWidget(self)
        return
    end

    config = parsed
    Spring.Log("AgentBootstrap", LOG.INFO, "Loaded config with " .. #(table.keys and table.keys(parsed.players) or {}) .. " player entries")
end

function widget:GameStart()
    if not config then return end

    local myPlayerID = Spring.GetMyPlayerID()
    local myName = Spring.GetPlayerInfo(myPlayerID)

    local entry = config.players[myName]
    if entry then
        local ai = entry.ai or "AgentBridge"
        local version = entry.version or "0.1"
        Spring.SendCommands("aicontrol " .. ai .. " " .. version)
        Spring.Log("AgentBootstrap", LOG.INFO, "AI control handed to " .. ai .. " " .. version .. " for player " .. myName)
    else
        Spring.Log("AgentBootstrap", LOG.INFO, "No AI config for player '" .. myName .. "', doing nothing")
    end
end
