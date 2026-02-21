-- 03_combat.lua: Combat resolution system via on_tick hook

hooks.on_tick(function(tick)
    local combatants = ecs:query("CombatTarget")
    if #combatants == 0 then
        return
    end

    -- Collect combat data first to avoid mutation during iteration
    local rounds = {}
    local to_remove = {}

    for _, attacker in ipairs(combatants) do
        -- Skip dead attackers
        if ecs:has(attacker, "Dead") then
            table.insert(to_remove, attacker)
        else
            local ct = ecs:get(attacker, "CombatTarget")
            if ct then
                local target = ct

                -- Check both in same room
                local atk_room = space:entity_room(attacker)
                local tgt_room = space:entity_room(target)

                if not atk_room or atk_room ~= tgt_room then
                    table.insert(to_remove, attacker)
                elseif ecs:has(target, "Dead") then
                    table.insert(to_remove, attacker)
                else
                    local atk_stat = ecs:get(attacker, "Attack") or 5
                    local def_stat = ecs:get(target, "Defense") or 0

                    table.insert(rounds, {
                        attacker = attacker,
                        target = target,
                        atk = atk_stat,
                        def = def_stat,
                    })
                end
            end
        end
    end

    -- Apply combat rounds
    local deaths = {}

    for _, round in ipairs(rounds) do
        local damage = math.max(round.atk - round.def, 1)

        local hp = ecs:get(round.target, "Health")
        if not hp then
            table.insert(to_remove, round.attacker)
        else
            local new_hp = hp.current - damage
            ecs:set(round.target, "Health", {current = new_hp, max = hp.max})

            local atk_name = get_name(round.attacker)
            local tgt_name = get_name(round.target)
            local display_hp = math.max(new_hp, 0)

            -- Notify attacker (yellow damage)
            local atk_sid = sessions:session_for(round.attacker)
            if atk_sid then
                output:send(atk_sid, tgt_name .. "에게 " .. colors.yellow .. tostring(damage) .. " 데미지" .. colors.reset .. "를 입혔습니다. (" .. tostring(display_hp) .. "/" .. tostring(hp.max) .. ")")
            end

            -- Notify target (red damage)
            local tgt_sid = sessions:session_for(round.target)
            if tgt_sid then
                output:send(tgt_sid, atk_name .. "이(가) 당신에게 " .. colors.red .. tostring(damage) .. " 데미지" .. colors.reset .. "를 입혔습니다. (" .. tostring(display_hp) .. "/" .. tostring(hp.max) .. ")")
            end

            -- Broadcast to room (exclude attacker and target)
            local room = space:entity_room(round.attacker)
            if room then
                local occupants = space:room_occupants(room)
                for _, occ in ipairs(occupants) do
                    if occ ~= round.attacker and occ ~= round.target then
                        local sid = sessions:session_for(occ)
                        if sid then
                            output:send(sid, atk_name .. "이(가) " .. tgt_name .. "을(를) 공격하여 " .. tostring(damage) .. " 데미지를 입혔습니다.")
                        end
                    end
                end
            end

            -- Check for death
            if new_hp <= 0 then
                table.insert(deaths, round.target)
                table.insert(to_remove, round.attacker)
            end
        end
    end

    -- Process deaths
    for _, dead_entity in ipairs(deaths) do
        ecs:set(dead_entity, "Dead", true)
        ecs:remove(dead_entity, "CombatTarget")

        local dead_name = get_name(dead_entity)

        -- Notify dead entity if player
        local dead_sid = sessions:session_for(dead_entity)
        if dead_sid then
            output:send(dead_sid, colors.bold .. colors.red .. "당신은 죽었습니다!" .. colors.reset)
        end

        -- Broadcast death to room
        local room = space:entity_room(dead_entity)
        if room then
            local occupants = space:room_occupants(room)
            for _, occ in ipairs(occupants) do
                if occ ~= dead_entity then
                    local sid = sessions:session_for(occ)
                    if sid then
                        output:send(sid, colors.red .. dead_name .. "이(가) 쓰러졌습니다!" .. colors.reset)
                    end
                end
            end
        end
    end

    -- Remove CombatTarget from resolved combats
    for _, entity in ipairs(to_remove) do
        ecs:remove(entity, "CombatTarget")
    end
end)
