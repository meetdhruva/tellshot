#compdef tellshot

autoload -U is-at-least

_tellshot() {
    typeset -A opt_args
    typeset -a _arguments_options
    local ret=1

    if is-at-least 5.2; then
        _arguments_options=(-s -S -C)
    else
        _arguments_options=(-s -C)
    fi

    local context curcontext="$curcontext" state line
    _arguments "${_arguments_options[@]}" : \
'--json[Print machine-readable JSON instead of text (also for errors)]' \
'-h[Print help]' \
'--help[Print help]' \
'-V[Print version]' \
'--version[Print version]' \
":: :_tellshot_commands" \
"*::: :->tellshot" \
&& ret=0
    case $state in
    (tellshot)
        words=($line[1] "${words[@]}")
        (( CURRENT += 1 ))
        curcontext="${curcontext%:*:*}:tellshot-command-$line[1]:"
        case $line[1] in
            (targets)
_arguments "${_arguments_options[@]}" : \
'--backend=[]:BACKEND:((auto\:"Best native backend for this session"
synthetic\:"Deterministic test pattern; works anywhere, including headless CI and sandboxes"))' \
'--json[Print machine-readable JSON instead of text (also for errors)]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
'-V[Print version]' \
'--version[Print version]' \
&& ret=0
;;
(capture)
_arguments "${_arguments_options[@]}" : \
'-o+[Output path (.tellshot keeps everything editable; .png is a flat image)]:OUTPUT:_files' \
'--output=[Output path (.tellshot keeps everything editable; .png is a flat image)]:OUTPUT:_files' \
'--target=[Target id from \`tellshot targets\` (default\: primary monitor)]:TARGET:_default' \
'--region=[Region as X,Y,WIDTH,HEIGHT in physical desktop pixels]:REGION:_default' \
'--backend=[]:BACKEND:((auto\:"Best native backend for this session"
synthetic\:"Deterministic test pattern; works anywhere, including headless CI and sandboxes"))' \
'--overwrite[Replace the output if it exists]' \
'--json[Print machine-readable JSON instead of text (also for errors)]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
'-V[Print version]' \
'--version[Print version]' \
&& ret=0
;;
(new)
_arguments "${_arguments_options[@]}" : \
'-o+[]:OUTPUT:_files' \
'--output=[]:OUTPUT:_files' \
'--overwrite[]' \
'--json[Print machine-readable JSON instead of text (also for errors)]' \
'-h[Print help]' \
'--help[Print help]' \
'-V[Print version]' \
'--version[Print version]' \
':image -- Source image:_files' \
&& ret=0
;;
(describe)
_arguments "${_arguments_options[@]}" : \
'--json[Print machine-readable JSON instead of text (also for errors)]' \
'-h[Print help]' \
'--help[Print help]' \
'-V[Print version]' \
'--version[Print version]' \
':project -- A .tellshot project:_files' \
&& ret=0
;;
(edit)
_arguments "${_arguments_options[@]}" : \
'(--edits-file)--edit=[One edit or an array of edits as JSON; \`-\` reads from stdin]:EDIT:_default' \
'--edits-file=[A file containing a JSON array of edits]:EDITS_FILE:_files' \
'--json[Print machine-readable JSON instead of text (also for errors)]' \
'-h[Print help]' \
'--help[Print help]' \
'-V[Print version]' \
'--version[Print version]' \
':project -- A .tellshot project:_files' \
&& ret=0
;;
(export)
_arguments "${_arguments_options[@]}" : \
'-o+[]:OUTPUT:_files' \
'--output=[]:OUTPUT:_files' \
'--overwrite[]' \
'--json[Print machine-readable JSON instead of text (also for errors)]' \
'-h[Print help]' \
'--help[Print help]' \
'-V[Print version]' \
'--version[Print version]' \
':project:_files' \
&& ret=0
;;
(stitch)
_arguments "${_arguments_options[@]}" : \
'-o+[]:OUTPUT:_files' \
'--output=[]:OUTPUT:_files' \
'--overwrite[]' \
'--json[Print machine-readable JSON instead of text (also for errors)]' \
'-h[Print help]' \
'--help[Print help]' \
'-V[Print version]' \
'--version[Print version]' \
'*::frames -- Frames in scroll order (PNG):_files' \
&& ret=0
;;
(schema)
_arguments "${_arguments_options[@]}" : \
'--json[Print machine-readable JSON instead of text (also for errors)]' \
'-h[Print help (see more with '\''--help'\'')]' \
'--help[Print help (see more with '\''--help'\'')]' \
'-V[Print version]' \
'--version[Print version]' \
':which:((edit\:"A list of edits for \`tellshot edit\` and the \`apply_edits\` MCP tool"
document\:"The \`document.json\` inside a .tellshot project"))' \
&& ret=0
;;
(mcp)
_arguments "${_arguments_options[@]}" : \
'--root=[Only allow reading and writing files inside this folder (recommended)]:ROOT:_files' \
'--json[Print machine-readable JSON instead of text (also for errors)]' \
'-h[Print help]' \
'--help[Print help]' \
'-V[Print version]' \
'--version[Print version]' \
&& ret=0
;;
(completions)
_arguments "${_arguments_options[@]}" : \
'--json[Print machine-readable JSON instead of text (also for errors)]' \
'-h[Print help]' \
'--help[Print help]' \
'-V[Print version]' \
'--version[Print version]' \
':shell:(bash elvish fish powershell zsh)' \
&& ret=0
;;
(help)
_arguments "${_arguments_options[@]}" : \
":: :_tellshot__subcmd__help_commands" \
"*::: :->help" \
&& ret=0

    case $state in
    (help)
        words=($line[1] "${words[@]}")
        (( CURRENT += 1 ))
        curcontext="${curcontext%:*:*}:tellshot-help-command-$line[1]:"
        case $line[1] in
            (targets)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(capture)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(new)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(describe)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(edit)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(export)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(stitch)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(schema)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(mcp)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(completions)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
(help)
_arguments "${_arguments_options[@]}" : \
&& ret=0
;;
        esac
    ;;
esac
;;
        esac
    ;;
esac
}

(( $+functions[_tellshot_commands] )) ||
_tellshot_commands() {
    local commands; commands=(
'targets:List monitors that can be captured' \
'capture:Capture a monitor or region to a .tellshot project or .png' \
'new:Create a .tellshot project from a PNG' \
'describe:List the annotations in a project' \
'edit:Apply edits (JSON) to a project, all or nothing. See \`tellshot schema edit\`' \
'export:Render a project to a PNG with annotations flattened (redactions destroy pixels)' \
'stitch:Stitch overlapping frames of a scrolling page into one tall image' \
'schema:Print the JSON Schema for edits or documents' \
'mcp:Run a Model Context Protocol server on stdio for AI agents' \
'completions:Print a shell completion script, for example \`tellshot completions bash\`' \
'help:Print this message or the help of the given subcommand(s)' \
    )
    _describe -t commands 'tellshot commands' commands "$@"
}
(( $+functions[_tellshot__subcmd__capture_commands] )) ||
_tellshot__subcmd__capture_commands() {
    local commands; commands=()
    _describe -t commands 'tellshot capture commands' commands "$@"
}
(( $+functions[_tellshot__subcmd__completions_commands] )) ||
_tellshot__subcmd__completions_commands() {
    local commands; commands=()
    _describe -t commands 'tellshot completions commands' commands "$@"
}
(( $+functions[_tellshot__subcmd__describe_commands] )) ||
_tellshot__subcmd__describe_commands() {
    local commands; commands=()
    _describe -t commands 'tellshot describe commands' commands "$@"
}
(( $+functions[_tellshot__subcmd__edit_commands] )) ||
_tellshot__subcmd__edit_commands() {
    local commands; commands=()
    _describe -t commands 'tellshot edit commands' commands "$@"
}
(( $+functions[_tellshot__subcmd__export_commands] )) ||
_tellshot__subcmd__export_commands() {
    local commands; commands=()
    _describe -t commands 'tellshot export commands' commands "$@"
}
(( $+functions[_tellshot__subcmd__help_commands] )) ||
_tellshot__subcmd__help_commands() {
    local commands; commands=(
'targets:List monitors that can be captured' \
'capture:Capture a monitor or region to a .tellshot project or .png' \
'new:Create a .tellshot project from a PNG' \
'describe:List the annotations in a project' \
'edit:Apply edits (JSON) to a project, all or nothing. See \`tellshot schema edit\`' \
'export:Render a project to a PNG with annotations flattened (redactions destroy pixels)' \
'stitch:Stitch overlapping frames of a scrolling page into one tall image' \
'schema:Print the JSON Schema for edits or documents' \
'mcp:Run a Model Context Protocol server on stdio for AI agents' \
'completions:Print a shell completion script, for example \`tellshot completions bash\`' \
'help:Print this message or the help of the given subcommand(s)' \
    )
    _describe -t commands 'tellshot help commands' commands "$@"
}
(( $+functions[_tellshot__subcmd__help__subcmd__capture_commands] )) ||
_tellshot__subcmd__help__subcmd__capture_commands() {
    local commands; commands=()
    _describe -t commands 'tellshot help capture commands' commands "$@"
}
(( $+functions[_tellshot__subcmd__help__subcmd__completions_commands] )) ||
_tellshot__subcmd__help__subcmd__completions_commands() {
    local commands; commands=()
    _describe -t commands 'tellshot help completions commands' commands "$@"
}
(( $+functions[_tellshot__subcmd__help__subcmd__describe_commands] )) ||
_tellshot__subcmd__help__subcmd__describe_commands() {
    local commands; commands=()
    _describe -t commands 'tellshot help describe commands' commands "$@"
}
(( $+functions[_tellshot__subcmd__help__subcmd__edit_commands] )) ||
_tellshot__subcmd__help__subcmd__edit_commands() {
    local commands; commands=()
    _describe -t commands 'tellshot help edit commands' commands "$@"
}
(( $+functions[_tellshot__subcmd__help__subcmd__export_commands] )) ||
_tellshot__subcmd__help__subcmd__export_commands() {
    local commands; commands=()
    _describe -t commands 'tellshot help export commands' commands "$@"
}
(( $+functions[_tellshot__subcmd__help__subcmd__help_commands] )) ||
_tellshot__subcmd__help__subcmd__help_commands() {
    local commands; commands=()
    _describe -t commands 'tellshot help help commands' commands "$@"
}
(( $+functions[_tellshot__subcmd__help__subcmd__mcp_commands] )) ||
_tellshot__subcmd__help__subcmd__mcp_commands() {
    local commands; commands=()
    _describe -t commands 'tellshot help mcp commands' commands "$@"
}
(( $+functions[_tellshot__subcmd__help__subcmd__new_commands] )) ||
_tellshot__subcmd__help__subcmd__new_commands() {
    local commands; commands=()
    _describe -t commands 'tellshot help new commands' commands "$@"
}
(( $+functions[_tellshot__subcmd__help__subcmd__schema_commands] )) ||
_tellshot__subcmd__help__subcmd__schema_commands() {
    local commands; commands=()
    _describe -t commands 'tellshot help schema commands' commands "$@"
}
(( $+functions[_tellshot__subcmd__help__subcmd__stitch_commands] )) ||
_tellshot__subcmd__help__subcmd__stitch_commands() {
    local commands; commands=()
    _describe -t commands 'tellshot help stitch commands' commands "$@"
}
(( $+functions[_tellshot__subcmd__help__subcmd__targets_commands] )) ||
_tellshot__subcmd__help__subcmd__targets_commands() {
    local commands; commands=()
    _describe -t commands 'tellshot help targets commands' commands "$@"
}
(( $+functions[_tellshot__subcmd__mcp_commands] )) ||
_tellshot__subcmd__mcp_commands() {
    local commands; commands=()
    _describe -t commands 'tellshot mcp commands' commands "$@"
}
(( $+functions[_tellshot__subcmd__new_commands] )) ||
_tellshot__subcmd__new_commands() {
    local commands; commands=()
    _describe -t commands 'tellshot new commands' commands "$@"
}
(( $+functions[_tellshot__subcmd__schema_commands] )) ||
_tellshot__subcmd__schema_commands() {
    local commands; commands=()
    _describe -t commands 'tellshot schema commands' commands "$@"
}
(( $+functions[_tellshot__subcmd__stitch_commands] )) ||
_tellshot__subcmd__stitch_commands() {
    local commands; commands=()
    _describe -t commands 'tellshot stitch commands' commands "$@"
}
(( $+functions[_tellshot__subcmd__targets_commands] )) ||
_tellshot__subcmd__targets_commands() {
    local commands; commands=()
    _describe -t commands 'tellshot targets commands' commands "$@"
}

if [ "$funcstack[1]" = "_tellshot" ]; then
    _tellshot "$@"
else
    compdef _tellshot tellshot
fi
