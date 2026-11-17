# Run from the console: function phase2:probe
say [phase2] P2-F3 probe start
# P2-T1: function tag with a plain string entry
function #phase2:chain
# P2-F4: nested call through execute
execute run function phase2:nested
scoreboard players get #load p2
scoreboard players get #tick p2
say [phase2] P2-F3 probe end
