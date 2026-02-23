-- 06_custom_commands.lua
-- Example: custom player commands (search, rest, talk)
-- This file demonstrates how to add new commands via on_action hooks.

-- "search" command: find hidden items in the current room
hooks.on_action("search", function(ctx)
    local entity = ctx.entity
    local session_id = ctx.session_id
    local room = space:entity_room(entity)

    if not room then
        output:send(session_id, "You are nowhere.")
        return true
    end

    -- Random chance to find something
    local roll = math.random(1, 100)
    if roll <= 30 then
        output:send(session_id, colors.green .. "You search carefully and find a hidden Healing Potion!" .. colors.reset)
        local potion = ecs:spawn()
        ecs:set(potion, "Name", "Healing Potion")
        ecs:set(potion, "ItemTag", true)
        ecs:set(potion, "Inventory", entity)
    else
        output:send(session_id, colors.yellow .. "You search the area but find nothing of interest." .. colors.reset)
    end

    return true
end)

-- "rest" command: slowly recover health
hooks.on_action("rest", function(ctx)
    local entity = ctx.entity
    local session_id = ctx.session_id

    local hp = ecs:get(entity, "Health")
    if not hp then
        output:send(session_id, "You don't need to rest.")
        return true
    end

    if hp.current >= hp.max then
        output:send(session_id, "You are already at full health.")
        return true
    end

    -- Check if in combat
    local target = ecs:get(entity, "CombatTarget")
    if target then
        output:send(session_id, colors.red .. "You can't rest while in combat!" .. colors.reset)
        return true
    end

    local heal = math.min(10, hp.max - hp.current)
    hp.current = hp.current + heal
    ecs:set(entity, "Health", hp)

    output:send(session_id, colors.green .. "You rest for a moment and recover " .. heal .. " HP. (" .. hp.current .. "/" .. hp.max .. ")" .. colors.reset)

    -- Notify room
    local room = space:entity_room(entity)
    if room then
        local name = ecs:get(entity, "Name") or "Someone"
        output:broadcast_room(room, colors.yellow .. name .. " sits down to rest." .. colors.reset, entity)
    end

    return true
end)

-- "talk" command: interact with friendly NPCs
hooks.on_action("talk", function(ctx)
    local entity = ctx.entity
    local session_id = ctx.session_id
    local args = ctx.args
    local room = space:entity_room(entity)

    if not room or not args or args == "" then
        output:send(session_id, "Talk to whom? Usage: talk <name>")
        return true
    end

    -- Find NPC in the same room by name
    local target_name = args:lower()
    local occupants = space:room_occupants(room)
    local found = nil

    for _, occ in ipairs(occupants) do
        if ecs:has(occ, "NpcTag") then
            local name = ecs:get(occ, "Name") or ""
            if name:lower():find(target_name) then
                found = occ
                break
            end
        end
    end

    if not found then
        output:send(session_id, "There is no one named '" .. args .. "' here.")
        return true
    end

    local npc_name = ecs:get(found, "Name") or "NPC"
    -- For now, just show a generic response
    -- In a full game, you'd look up dialogue from content data
    output:send(session_id, colors.cyan .. npc_name .. colors.reset .. " says: " .. colors.yellow .. "\"Greetings, traveler! How can I help you?\"" .. colors.reset)

    return true
end)
