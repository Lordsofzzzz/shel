# shel bash hook — add to ~/.bashrc
__shel_session_id=$(uuidgen 2>/dev/null || cat /proc/sys/kernel/random/uuid 2>/dev/null || date +%s)
__shel_start_time=

__shel_preexec() {
    __shel_start_time=$(date +%s%3N)
}

__shel_precmd() {
    local exit_code=$?
    local cmd
    cmd=$(HISTTIMEFORMAT= history 1 | sed 's/^ *[0-9]* *//')
    [[ -z "$cmd" ]] && return
    local duration_ms=0
    if [[ -n "$__shel_start_time" ]]; then
        duration_ms=$(( $(date +%s%3N) - __shel_start_time ))
    fi
    (shel record "$cmd" \
        --exit-code "$exit_code" \
        --duration-ms "$duration_ms" \
        --session-id "$__shel_session_id" &) >/dev/null 2>&1
    __shel_start_time=
}

__shel_ctrl_r() {
    local selected
    selected=$(shel ui "$READLINE_LINE" 3>&1 1>&2 2>&3)
    if [[ -n "$selected" ]]; then
        READLINE_LINE="$selected"
        READLINE_POINT=${#READLINE_LINE}
    fi
}

trap '__shel_preexec' DEBUG
PROMPT_COMMAND="__shel_precmd${PROMPT_COMMAND:+;$PROMPT_COMMAND}"
bind -x '"\C-r": __shel_ctrl_r'
