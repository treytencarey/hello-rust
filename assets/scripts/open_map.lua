local room = nil

function on_update()
    local PlayerState = world:get_type_by_name("PlayerState")
    local player_state = world:get_component(entity, PlayerState)

    if player_state.room ~= room then
        room = player_state.room
        local file_path = "assets/map_" .. room .. ".tmx"
        os.execute('start "" "' .. file_path .. '"')
        print("Room changed: ", room)
    end
end