local options = {
    {
        key  = 'socket_path',
        name = 'GameManager Socket Path',
        desc = 'Unix socket path for IPC with GameManager',
        type = 'string',
        def  = '/tmp/game-manager.sock',
    },
}

return options
