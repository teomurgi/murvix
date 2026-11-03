# Restore the user's startup directory before zsh reads the remaining startup files.
if [[ ${MURVIX_ZDOTDIR_SET-} == 1 ]]; then
    export ZDOTDIR=$MURVIX_ORIGINAL_ZDOTDIR
else
    unset ZDOTDIR
fi
unset MURVIX_ZDOTDIR_SET MURVIX_ORIGINAL_ZDOTDIR
[[ -r ${ZDOTDIR-$HOME}/.zshenv ]] && source "${ZDOTDIR-$HOME}/.zshenv"
[[ -o interactive && ${MURVIX_SHELL_INTEGRATION-} == 1 ]] && source "$MURVIX_SHELL_INTEGRATION_DIR/murvix.zsh"
