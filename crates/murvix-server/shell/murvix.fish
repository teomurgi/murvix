# Older fish versions permit return only inside a function.
function __murvix_setup
    status is-interactive; or return
    if test "$MUXY_SHELL_INTEGRATION" != 1; or set -q __murvix_installed
        return
    end
    set -g __murvix_installed 1

    function __murvix_directory --on-event fish_prompt
        printf '\e]7;file://%s\a' (string escape --style=url -- $PWD | string replace -a '%2F' '/')
    end

    # Fish 4 emits its own prompt marks. Do not wrap or duplicate them.
    if status test-feature mark-prompt 2>/dev/null
        return
    end

    function __murvix_preexec --on-event fish_preexec
        printf '\e]133;C\a'
    end

    function __murvix_postexec --on-event fish_postexec
        set -l result $status
        printf '\e]133;D;%d\a' $result
    end

    function __murvix_status
        return $argv[1]
    end

    # Wrap after the user's configuration has defined fish_prompt.
    function __murvix_install --on-event fish_prompt
        functions --copy fish_prompt __murvix_user_prompt
        function fish_prompt
            set -l result $status
            printf '\e]133;A\a'
            __murvix_status $result
            __murvix_user_prompt
            printf '\e]133;B\a'
        end
        functions --erase __murvix_install
    end
end

__murvix_setup
functions --erase __murvix_setup

# Workspace-aware agent launchers. Inside a Murvix terminal, bare
# `omp`/`claude`/`codex`/`copilot` resolve the live workspace once at launch
# (like `murvix agent <tool>`) instead of seeing only the shell's cwd.
# Outside a Murvix terminal, with wrappers disabled
# (`MUXY_WORKSPACE_WRAPPERS=0`), or when the server is unreachable, the plain
# tool runs unchanged. `command <tool>` always bypasses the wrapper.
function _murvix_with_workspace
    set -l tool $argv[1]; set -e argv[1]
    if test "$MUXY_WORKSPACE_WRAPPERS" = 0; or test -z "$MUXY_SERVER_ID"; or test -z "$MUXY_SESSION_ID"; or not command -v murvix >/dev/null 2>&1
        command $tool $argv
        return
    end
    set -l roots (command murvix project here 2>/dev/null)
    if test $status -eq 0 -a -n "$roots"
        set -l extra
        for root in $roots
            if test -n "$root" -a -d "$root"
                set -a extra --add-dir="$root"
            end
        end
        command $tool $extra $argv
    else
        command $tool $argv
    end
end
for _murvix_tool in omp claude codex copilot
    eval "function $_murvix_tool; _murvix_with_workspace $_murvix_tool \$argv; end"
end
functions --erase _murvix_tool 2>/dev/null; set --erase _murvix_tool
