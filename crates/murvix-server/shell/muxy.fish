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
