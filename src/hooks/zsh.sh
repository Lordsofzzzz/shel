# shel zsh hook — add to ~/.zshrc
__shel_session_id=$(uuidgen 2>/dev/null || date +%s)
__shel_start_time=

__shel_preexec() {
    __shel_start_time=${EPOCHREALTIME-$(date +%s%3N)}
}

__shel_precmd() {
    local exit_code=$?
    local cmd=$history[$HISTCMD]
    [[ -z "$cmd" ]] && return
    local duration_ms=0
    if [[ -n "$__shel_start_time" ]]; then
        duration_ms=$(( $(date +%s%3N) - ${__shel_start_time%%.*}000 ))
    fi
    (shel record "$cmd" \
        --exit-code "$exit_code" \
        --duration-ms "$duration_ms" \
        --session-id "$__shel_session_id" &) >/dev/null 2>&1
    __shel_start_time=
}

__shel_ctrl_r() {
    zle -I
    local selected
    selected=$(shel ui "$BUFFER" 3>&1 1>&2 2>&3)
    zle reset-prompt
    [[ -z "$selected" ]] && return
    BUFFER="$selected"
    CURSOR=${#BUFFER}
}

autoload -Uz add-zsh-hook
add-zsh-hook preexec __shel_preexec
add-zsh-hook precmd __shel_precmd
zle -N __shel_ctrl_r
bindkey '^R' __shel_ctrl_r
