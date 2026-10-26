[[ -o interactive && ${MUXY_SHELL_INTEGRATION-} == 1 ]] || return
(( ${+_murvix_installed} )) && return
typeset -g _murvix_installed=1 _murvix_running=0 _murvix_prompt='' _murvix_user_prompt=''

_murvix_directory() {
    emulate -L zsh
    local LC_ALL=C encoded='' char hex
    local -i i
    for (( i=1; i<=${#PWD}; i++ )); do
        char=$PWD[i]
        if [[ $char == [a-zA-Z0-9/._~-] ]]; then
            encoded+=$char
        else
            printf -v hex '%%%02X' "'$char"
            encoded+=$hex
        fi
    done
    printf '\e]7;file://%s\a' "$encoded"
}

_murvix_precmd() {
    local result=$?
    if (( _murvix_running )); then
        printf '\e]133;D;%d\a' "$result"
    fi
    _murvix_running=0
    _murvix_directory
    [[ $PS1 != $_murvix_prompt ]] && _murvix_user_prompt=$PS1
    _murvix_prompt=$'%{\e]133;A\a%}'"$_murvix_user_prompt"$'%{\e]133;B\a%}'
    PS1=$_murvix_prompt
    return "$result"
}

_murvix_preexec() {
    _murvix_running=1
    printf '\e]133;C\a'
}

_murvix_unbound_alt() { return 0 }

# Defer until user startup files and prompt frameworks have loaded.
_murvix_install() {
    local result=$?
    precmd_functions=(${precmd_functions:#_murvix_install} _murvix_precmd)
    preexec_functions+=(_murvix_preexec)
    # Consume complete unbound Alt arrows so ZLE cannot insert a CSI suffix.
    # Keep bindings installed by the user's startup files and prompt framework.
    zle -N _murvix_unbound_alt
    local keymap sequence binding
    for keymap in emacs viins vicmd; do
        for sequence in $'\e[1;3A' $'\e[1;3B'; do
            binding=$(bindkey -M "$keymap" "$sequence")
            if [[ ${binding##* } == undefined-key ]]; then
                bindkey -M "$keymap" "$sequence" _murvix_unbound_alt
            fi
        done
    done
    _murvix_precmd
    return "$result"
}
precmd_functions+=(_murvix_install)
