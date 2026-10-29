# Source explicitly from your interactive Bash startup file. Bash startup is not redirected.
[[ $- == *i* && ${MUXY_SHELL_INTEGRATION-} == 1 ]] || return
[[ ${_murvix_installed-} == 1 ]] && return
_murvix_installed=1
_murvix_running=0
_murvix_in_prompt=0
_murvix_debug_checked=0
_murvix_prompt=''
_murvix_user_prompt=''

_murvix_directory() {
    local LC_ALL=C encoded='' char hex i
    for (( i=0; i<${#PWD}; i++ )); do
        char=${PWD:i:1}
        case $char in
            [a-zA-Z0-9/._~-]) encoded+=$char ;;
            *) printf -v hex '%%%02X' "'$char"; encoded+=$hex ;;
        esac
    done
    printf '\e]7;file://%s\a' "$encoded"
}

_murvix_precmd() {
    local result=$?
    if (( _murvix_running )); then
        printf '\e]133;D;%d\a' "$result"
    fi
    _murvix_running=0
    _murvix_in_prompt=1
    _murvix_status=$result
    return "$result"
}

_murvix_prompt_end() {
    _murvix_directory
    [[ $PS1 != "$_murvix_prompt" ]] && _murvix_user_prompt=$PS1
    _murvix_prompt=$'\[\e]133;A\a\]'"$_murvix_user_prompt"$'\[\e]133;B\a\]'
    PS1=$_murvix_prompt
    _murvix_in_prompt=0
    return "$_murvix_status"
}

_murvix_prompt_status() {
    return "$_murvix_status"
}

# Bash 5 hides the caller's DEBUG trap while sourcing. Inspect it afterward,
# directly in PROMPT_COMMAND, without replacing a user's trap.
read -r -d '' _murvix_debug_setup <<'MUXY_DEBUG_SETUP' || :
if [[ ${_murvix_debug_checked-0} == 0 ]]; then
    _murvix_debug_checked=1
    if [[ -z $(trap -p DEBUG) ]]; then
        trap 'if [[ $_murvix_running == 0 && $_murvix_in_prompt == 0 && $BASH_COMMAND != _murvix_* && $BASH_COMMAND != "$PROMPT_COMMAND" ]]; then _murvix_running=1; printf "\e]133;C\a"; fi' DEBUG
    fi
fi
_murvix_prompt_status
MUXY_DEBUG_SETUP
if (( BASH_VERSINFO[0] > 5 || (BASH_VERSINFO[0] == 5 && BASH_VERSINFO[1] >= 1) )); then
    PROMPT_COMMAND=(_murvix_precmd "$_murvix_debug_setup" "${PROMPT_COMMAND[@]}" _murvix_prompt_end)
else
    PROMPT_COMMAND=$'_murvix_precmd\n'"$_murvix_debug_setup"$'\n'"${PROMPT_COMMAND-}"$'\n_murvix_prompt_end'
fi

_murvix_bind_keys() {
    local keymap sequence bindings
    for keymap in emacs-standard vi-insert vi-command; do
        bindings=$(bind -m "$keymap" -p; bind -m "$keymap" -s)
        for sequence in '\e[1;3A' '\e[1;3B'; do
            if [[ $bindings != *"\"$sequence\":"* ]]; then
                bind -m "$keymap" "\"$sequence\": \"\""
            fi
        done
    done
}
_murvix_bind_keys
unset -f _murvix_bind_keys

# Workspace-aware agent launchers. Inside a Murvix terminal, bare
# `omp`/`claude`/`codex`/`copilot` resolve the live workspace once at launch
# (like `murvix agent <tool>`) instead of seeing only the shell's cwd.
# Outside a Murvix terminal, with wrappers disabled
# (`MUXY_WORKSPACE_WRAPPERS=0`), or when the server is unreachable, the plain
# tool runs unchanged. `command <tool>` always bypasses the wrapper.
_murvix_with_workspace() {
    local tool=$1; shift
    if [[ ${MUXY_WORKSPACE_WRAPPERS-} == 0 || -z ${MUXY_SERVER_ID-} || -z ${MUXY_SESSION_ID-} ]] || ! command -v murvix >/dev/null 2>&1; then
        command "$tool" "$@"
        return
    fi
    local roots root extra=()
    if roots=$(command murvix project here 2>/dev/null) && [[ -n $roots ]]; then
        while IFS= read -r root; do
            [[ -n $root && -d $root ]] && extra+=(--add-dir="$root")
        done <<<"$roots"
        ((${#extra[@]})) || extra=()
        command "$tool" "${extra[@]}" "$@"
    else
        command "$tool" "$@"
    fi
}
for _murvix_tool in omp claude codex copilot; do
    eval "$_murvix_tool() { _murvix_with_workspace $_murvix_tool \"\$@\"; }"
done
unset _murvix_tool
