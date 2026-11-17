# P2-F1: runs from #minecraft:load (plain string tag entry, the form Pumpkin supports)
scoreboard objectives add p2 dummy
scoreboard players add #load p2 1
say [phase2] P2-F1 load ok
