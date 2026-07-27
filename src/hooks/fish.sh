# shel fish hook — save to ~/.config/fish/conf.d/shel.fish
set -g __shel_session_id (uuidgen 2>/dev/null; or date +%s)
set -g __shel_start_time 0

function __shel_on_event --on-event fish_preexec
    set __shel_start_time (date +%s%3N)
end

function __shel_on_postexec --on-event fish_postexec
    set -l exit_code $status
    set -l cmd $argv[1]
    set -l duration_ms 0
    if test $__shel_start_time -gt 0
        set duration_ms (math (date +%s%3N) - $__shel_start_time)
    end
    shel record "$cmd" \
        --exit-code $exit_code \
        --duration-ms $duration_ms \
        --session-id $__shel_session_id &>/dev/null &
    set __shel_start_time 0
end

function __shel_ctrl_r
    set -l selected (shel ui (commandline) 2>/dev/null)
    if test -n "$selected"
        commandline -- $selected
    end
    commandline -f repaint
end

bind \cr __shel_ctrl_r
