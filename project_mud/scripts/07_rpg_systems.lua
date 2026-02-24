-- 07_rpg_systems.lua: RPG systems (skills, cooldowns, leveling, status)

-- Cooldown tracking: cooldowns[tostring(entity)][skill_id] = available_at_tick
local cooldowns = {}

-- Global current tick (updated by on_tick, used by on_action for cooldown checks)
_current_tick = 0

hooks.on_tick(function(tick)
    _current_tick = tick
end)

--- Calculate experience reward from a target entity.
function calc_exp_reward(target)
    local hp = ecs:get(target, "Health")
    local atk = ecs:get(target, "Attack") or 0
    local def = ecs:get(target, "Defense") or 0
    local max_hp = hp and hp.max or 0
    return math.floor(max_hp / 2 + atk + def)
end

--- Award experience to an entity. Returns true if leveled up.
function award_exp(entity, amount)
    local level = ecs:get(entity, "Level") or 1
    local exp = ecs:get(entity, "Experience") or 0
    exp = exp + amount
    local leveled_up = false

    local entry = level_table and level_table[level]
    while entry and exp >= entry.exp_required do
        exp = exp - entry.exp_required
        level = level + 1

        -- Apply level table bonuses
        local hp = ecs:get(entity, "Health")
        if hp then
            hp.max = hp.max + entry.hp_bonus
            hp.current = hp.max  -- Full heal on level up
            ecs:set(entity, "Health", hp)
        end

        local mp = ecs:get(entity, "Mana")
        if mp then
            mp.max = mp.max + entry.mp_bonus
            mp.current = mp.max  -- Full restore on level up
            ecs:set(entity, "Mana", mp)
        end

        local atk = ecs:get(entity, "Attack") or 0
        ecs:set(entity, "Attack", atk + entry.atk_bonus)

        local def = ecs:get(entity, "Defense") or 0
        ecs:set(entity, "Defense", def + entry.def_bonus)

        leveled_up = true
        entry = level_table and level_table[level]
    end

    ecs:set(entity, "Level", level)
    ecs:set(entity, "Experience", exp)
    return leveled_up
end

-- Get cooldown key for entity
local function cd_key(entity)
    return tostring(entity)
end

-- Check if a skill is on cooldown. Returns remaining ticks or 0.
local function get_cooldown_remaining(entity, skill_id)
    local key = cd_key(entity)
    if not cooldowns[key] then return 0 end
    local available_at = cooldowns[key][skill_id]
    if not available_at then return 0 end
    local remaining = available_at - _current_tick
    if remaining <= 0 then
        cooldowns[key][skill_id] = nil
        return 0
    end
    return remaining
end

-- Set skill cooldown
local function set_cooldown(entity, skill_id, ticks)
    local key = cd_key(entity)
    if not cooldowns[key] then cooldowns[key] = {} end
    cooldowns[key][skill_id] = _current_tick + ticks
end

-- Handle "status" action
hooks.on_action("status", function(ctx)
    output:send(ctx.session_id, format_status(ctx.entity))
    return true
end)

-- Handle "skill_list" action
hooks.on_action("skill_list", function(ctx)
    local sid = ctx.session_id
    local entity = ctx.entity

    local skills_data = ecs:get(entity, "Skills")
    if not skills_data or not skills_data.learned or #skills_data.learned == 0 then
        output:send(sid, "배운 스킬이 없습니다.")
        return true
    end

    local lines = {colors.bold .. colors.magenta .. "=== 보유 스킬 ===" .. colors.reset}
    for _, skill_id in ipairs(skills_data.learned) do
        local def = get_skill_def(skill_id)
        local cd_remaining = get_cooldown_remaining(entity, skill_id)
        local cd_text = ""
        if cd_remaining > 0 then
            cd_text = colors.red .. " [쿨다운: " .. tostring(cd_remaining) .. "틱]" .. colors.reset
        else
            cd_text = colors.green .. " [사용 가능]" .. colors.reset
        end

        local desc = ""
        if def then
            desc = " - " .. def.description
        end

        table.insert(lines, "  " .. colors.yellow .. skill_id .. colors.reset .. desc .. cd_text)
    end

    table.insert(lines, "")
    table.insert(lines, "사용법: <스킬이름> 스킬")
    output:send(sid, table.concat(lines, "\n"))
    return true
end)

-- Handle "use_skill" action
hooks.on_action("use_skill", function(ctx)
    local sid = ctx.session_id
    local entity = ctx.entity
    local skill_name = ctx.args

    -- Dead check
    if ecs:has(entity, "Dead") then
        output:send(sid, "죽은 상태에서는 스킬을 사용할 수 없습니다.")
        return true
    end

    -- Check if player has this skill
    local skills_data = ecs:get(entity, "Skills")
    if not skills_data or not skills_data.learned then
        output:send(sid, "배운 스킬이 없습니다.")
        return true
    end

    local has_skill = false
    for _, learned_name in ipairs(skills_data.learned) do
        if learned_name == skill_name then
            has_skill = true
            break
        end
    end

    if not has_skill then
        output:send(sid, "'" .. skill_name .. "' 스킬을 배우지 않았습니다.")
        return true
    end

    -- Get skill definition
    local skill_def = get_skill_def(skill_name)
    if not skill_def then
        output:send(sid, "'" .. skill_name .. "' 스킬 정보를 찾을 수 없습니다.")
        return true
    end

    -- Cooldown check
    local cd_remaining = get_cooldown_remaining(entity, skill_name)
    if cd_remaining > 0 then
        output:send(sid, colors.red .. "'" .. skill_name .. "' 쿨다운 중입니다. (" .. tostring(cd_remaining) .. "틱 남음)" .. colors.reset)
        return true
    end

    local player_name = get_name(entity)
    local room = space:entity_room(entity)
    local skill_type = skill_def.type

    if skill_type == "heal" then
        -- Heal: instant self-heal, no target needed
        local hp = ecs:get(entity, "Health")
        if hp then
            local old_hp = hp.current
            hp.current = math.min(hp.current + skill_def.heal_amount, hp.max)
            ecs:set(entity, "Health", hp)
            local healed = hp.current - old_hp
            output:send(sid, colors.green .. "'" .. skill_name .. "' 사용! " .. tostring(healed) .. " 회복. (" .. tostring(hp.current) .. "/" .. tostring(hp.max) .. ")" .. colors.reset)
            if room then
                broadcast_room(room, colors.green .. player_name .. "이(가) '" .. skill_name .. "'을(를) 사용했습니다." .. colors.reset, entity)
            end
        end
        set_cooldown(entity, skill_name, skill_def.cooldown)
        return true
    end

    -- attack or attack_heal: need a CombatTarget
    local ct = ecs:get(entity, "CombatTarget")
    if not ct then
        output:send(sid, "전투 중이 아닙니다. 먼저 대상을 공격하세요.")
        return true
    end

    local target = ct
    if ecs:has(target, "Dead") then
        output:send(sid, "대상이 이미 죽었습니다.")
        ecs:remove(entity, "CombatTarget")
        return true
    end

    local atk = ecs:get(entity, "Attack") or 0
    local def_val = ecs:get(target, "Defense") or 0
    local base_damage = math.max(atk - def_val, 1)
    local damage = math.floor(base_damage * skill_def.damage_mult)

    local target_name = get_name(target)
    local hp = ecs:get(target, "Health")
    if hp then
        hp.current = hp.current - damage
        ecs:set(target, "Health", hp)
        local display_hp = math.max(hp.current, 0)

        output:send(sid, colors.yellow .. "'" .. skill_name .. "'! " .. target_name .. "에게 " .. tostring(damage) .. " 데미지! (" .. tostring(display_hp) .. "/" .. tostring(hp.max) .. ")" .. colors.reset)

        local tgt_sid = sessions:session_for(target)
        if tgt_sid then
            output:send(tgt_sid, colors.red .. player_name .. "이(가) '" .. skill_name .. "'(으)로 " .. tostring(damage) .. " 데미지! (" .. tostring(display_hp) .. "/" .. tostring(hp.max) .. ")" .. colors.reset)
        end

        if room then
            local occupants = space:room_occupants(room)
            for _, occ in ipairs(occupants) do
                if occ ~= entity and occ ~= target then
                    local occ_sid = sessions:session_for(occ)
                    if occ_sid then
                        output:send(occ_sid, player_name .. "이(가) " .. target_name .. "에게 '" .. skill_name .. "' 스킬로 " .. tostring(damage) .. " 데미지를 입혔습니다.")
                    end
                end
            end
        end

        -- Self-heal for attack_heal type
        if skill_type == "attack_heal" and skill_def.heal_amount > 0 then
            local my_hp = ecs:get(entity, "Health")
            if my_hp then
                local old_hp = my_hp.current
                my_hp.current = math.min(my_hp.current + skill_def.heal_amount, my_hp.max)
                ecs:set(entity, "Health", my_hp)
                local healed = my_hp.current - old_hp
                if healed > 0 then
                    output:send(sid, colors.green .. tostring(healed) .. " 회복! (" .. tostring(my_hp.current) .. "/" .. tostring(my_hp.max) .. ")" .. colors.reset)
                end
            end
        end

        -- Check death
        if hp.current <= 0 then
            ecs:set(target, "Dead", true)
            ecs:remove(entity, "CombatTarget")

            local dead_sid = sessions:session_for(target)
            if dead_sid then
                output:send(dead_sid, colors.bold .. colors.red .. "당신은 죽었습니다!" .. colors.reset)
            end
            if room then
                broadcast_room(room, colors.red .. target_name .. "이(가) 쓰러졌습니다!" .. colors.reset, target)
            end

            -- Award experience and gold
            if ecs:has(entity, "PlayerTag") and ecs:has(target, "NpcTag") then
                local exp = calc_exp_reward(target)

                local leveled = award_exp(entity, exp)
                output:send(sid, colors.bright_yellow .. "경험치 +" .. tostring(exp) .. colors.reset)

                if leveled then
                    local new_level = ecs:get(entity, "Level") or 1
                    output:send(sid, colors.bold .. colors.bright_yellow .. "레벨 업! Lv." .. tostring(new_level) .. colors.reset)
                end

                -- Award gold from loot_table
                local gold_earned = calc_gold_drop(target)
                if gold_earned > 0 then
                    local current_gold = ecs:get(entity, "Gold") or 0
                    ecs:set(entity, "Gold", current_gold + gold_earned)
                    output:send(sid, colors.yellow .. "골드 +" .. tostring(gold_earned) .. colors.reset)
                end
            end
        end
    end

    set_cooldown(entity, skill_name, skill_def.cooldown)
    return true
end)
