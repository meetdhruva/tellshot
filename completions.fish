# Print an optspec for argparse to handle cmd's options that are independent of any subcommand.
function __fish_tellshot_global_optspecs
    string join \n json h/help V/version
end

function __fish_tellshot_needs_command
    # Figure out if the current invocation already has a command.
    set -l cmd (commandline -opc)
    set -e cmd[1]
    argparse -s (__fish_tellshot_global_optspecs) -- $cmd 2>/dev/null
    or return
    if set -q argv[1]
        # Also print the command, so this can be used to figure out what it is.
        echo $argv[1]
        return 1
    end
    return 0
end

function __fish_tellshot_using_subcommand
    set -l cmd (__fish_tellshot_needs_command)
    test -z "$cmd"
    and return 1
    contains -- $cmd[1] $argv
end

complete -c tellshot -n "__fish_tellshot_needs_command" -l json -d 'Print machine-readable JSON instead of text (also for errors)'
complete -c tellshot -n "__fish_tellshot_needs_command" -s h -l help -d 'Print help'
complete -c tellshot -n "__fish_tellshot_needs_command" -s V -l version -d 'Print version'
complete -c tellshot -n "__fish_tellshot_needs_command" -f -a "targets" -d 'List monitors that can be captured'
complete -c tellshot -n "__fish_tellshot_needs_command" -f -a "capture" -d 'Capture a monitor or region to a .tellshot project or .png'
complete -c tellshot -n "__fish_tellshot_needs_command" -f -a "new" -d 'Create a .tellshot project from a PNG'
complete -c tellshot -n "__fish_tellshot_needs_command" -f -a "describe" -d 'List the annotations in a project'
complete -c tellshot -n "__fish_tellshot_needs_command" -f -a "edit" -d 'Apply edits (JSON) to a project, all or nothing. See `tellshot schema edit`'
complete -c tellshot -n "__fish_tellshot_needs_command" -f -a "export" -d 'Render a project to a PNG with annotations flattened (redactions destroy pixels)'
complete -c tellshot -n "__fish_tellshot_needs_command" -f -a "stitch" -d 'Stitch overlapping frames of a scrolling page into one tall image'
complete -c tellshot -n "__fish_tellshot_needs_command" -f -a "schema" -d 'Print the JSON Schema for edits or documents'
complete -c tellshot -n "__fish_tellshot_needs_command" -f -a "mcp" -d 'Run a Model Context Protocol server on stdio for AI agents'
complete -c tellshot -n "__fish_tellshot_needs_command" -f -a "completions" -d 'Print a shell completion script, for example `tellshot completions bash`'
complete -c tellshot -n "__fish_tellshot_needs_command" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
complete -c tellshot -n "__fish_tellshot_using_subcommand targets" -l backend -r -f -a "auto\t'Best native backend for this session'
synthetic\t'Deterministic test pattern; works anywhere, including headless CI and sandboxes'"
complete -c tellshot -n "__fish_tellshot_using_subcommand targets" -l json -d 'Print machine-readable JSON instead of text (also for errors)'
complete -c tellshot -n "__fish_tellshot_using_subcommand targets" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c tellshot -n "__fish_tellshot_using_subcommand targets" -s V -l version -d 'Print version'
complete -c tellshot -n "__fish_tellshot_using_subcommand capture" -s o -l output -d 'Output path (.tellshot keeps everything editable; .png is a flat image)' -r -F
complete -c tellshot -n "__fish_tellshot_using_subcommand capture" -l target -d 'Target id from `tellshot targets` (default: primary monitor)' -r
complete -c tellshot -n "__fish_tellshot_using_subcommand capture" -l region -d 'Region as X,Y,WIDTH,HEIGHT in physical desktop pixels' -r
complete -c tellshot -n "__fish_tellshot_using_subcommand capture" -l backend -r -f -a "auto\t'Best native backend for this session'
synthetic\t'Deterministic test pattern; works anywhere, including headless CI and sandboxes'"
complete -c tellshot -n "__fish_tellshot_using_subcommand capture" -l overwrite -d 'Replace the output if it exists'
complete -c tellshot -n "__fish_tellshot_using_subcommand capture" -l json -d 'Print machine-readable JSON instead of text (also for errors)'
complete -c tellshot -n "__fish_tellshot_using_subcommand capture" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c tellshot -n "__fish_tellshot_using_subcommand capture" -s V -l version -d 'Print version'
complete -c tellshot -n "__fish_tellshot_using_subcommand new" -s o -l output -r -F
complete -c tellshot -n "__fish_tellshot_using_subcommand new" -l overwrite
complete -c tellshot -n "__fish_tellshot_using_subcommand new" -l json -d 'Print machine-readable JSON instead of text (also for errors)'
complete -c tellshot -n "__fish_tellshot_using_subcommand new" -s h -l help -d 'Print help'
complete -c tellshot -n "__fish_tellshot_using_subcommand new" -s V -l version -d 'Print version'
complete -c tellshot -n "__fish_tellshot_using_subcommand describe" -l json -d 'Print machine-readable JSON instead of text (also for errors)'
complete -c tellshot -n "__fish_tellshot_using_subcommand describe" -s h -l help -d 'Print help'
complete -c tellshot -n "__fish_tellshot_using_subcommand describe" -s V -l version -d 'Print version'
complete -c tellshot -n "__fish_tellshot_using_subcommand edit" -l edit -d 'One edit or an array of edits as JSON; `-` reads from stdin' -r
complete -c tellshot -n "__fish_tellshot_using_subcommand edit" -l edits-file -d 'A file containing a JSON array of edits' -r -F
complete -c tellshot -n "__fish_tellshot_using_subcommand edit" -l json -d 'Print machine-readable JSON instead of text (also for errors)'
complete -c tellshot -n "__fish_tellshot_using_subcommand edit" -s h -l help -d 'Print help'
complete -c tellshot -n "__fish_tellshot_using_subcommand edit" -s V -l version -d 'Print version'
complete -c tellshot -n "__fish_tellshot_using_subcommand export" -s o -l output -r -F
complete -c tellshot -n "__fish_tellshot_using_subcommand export" -l overwrite
complete -c tellshot -n "__fish_tellshot_using_subcommand export" -l json -d 'Print machine-readable JSON instead of text (also for errors)'
complete -c tellshot -n "__fish_tellshot_using_subcommand export" -s h -l help -d 'Print help'
complete -c tellshot -n "__fish_tellshot_using_subcommand export" -s V -l version -d 'Print version'
complete -c tellshot -n "__fish_tellshot_using_subcommand stitch" -s o -l output -r -F
complete -c tellshot -n "__fish_tellshot_using_subcommand stitch" -l overwrite
complete -c tellshot -n "__fish_tellshot_using_subcommand stitch" -l json -d 'Print machine-readable JSON instead of text (also for errors)'
complete -c tellshot -n "__fish_tellshot_using_subcommand stitch" -s h -l help -d 'Print help'
complete -c tellshot -n "__fish_tellshot_using_subcommand stitch" -s V -l version -d 'Print version'
complete -c tellshot -n "__fish_tellshot_using_subcommand schema" -l json -d 'Print machine-readable JSON instead of text (also for errors)'
complete -c tellshot -n "__fish_tellshot_using_subcommand schema" -s h -l help -d 'Print help (see more with \'--help\')'
complete -c tellshot -n "__fish_tellshot_using_subcommand schema" -s V -l version -d 'Print version'
complete -c tellshot -n "__fish_tellshot_using_subcommand mcp" -l root -d 'Only allow reading and writing files inside this folder (recommended)' -r -F
complete -c tellshot -n "__fish_tellshot_using_subcommand mcp" -l json -d 'Print machine-readable JSON instead of text (also for errors)'
complete -c tellshot -n "__fish_tellshot_using_subcommand mcp" -s h -l help -d 'Print help'
complete -c tellshot -n "__fish_tellshot_using_subcommand mcp" -s V -l version -d 'Print version'
complete -c tellshot -n "__fish_tellshot_using_subcommand completions" -l json -d 'Print machine-readable JSON instead of text (also for errors)'
complete -c tellshot -n "__fish_tellshot_using_subcommand completions" -s h -l help -d 'Print help'
complete -c tellshot -n "__fish_tellshot_using_subcommand completions" -s V -l version -d 'Print version'
complete -c tellshot -n "__fish_tellshot_using_subcommand help; and not __fish_seen_subcommand_from targets capture new describe edit export stitch schema mcp completions help" -f -a "targets" -d 'List monitors that can be captured'
complete -c tellshot -n "__fish_tellshot_using_subcommand help; and not __fish_seen_subcommand_from targets capture new describe edit export stitch schema mcp completions help" -f -a "capture" -d 'Capture a monitor or region to a .tellshot project or .png'
complete -c tellshot -n "__fish_tellshot_using_subcommand help; and not __fish_seen_subcommand_from targets capture new describe edit export stitch schema mcp completions help" -f -a "new" -d 'Create a .tellshot project from a PNG'
complete -c tellshot -n "__fish_tellshot_using_subcommand help; and not __fish_seen_subcommand_from targets capture new describe edit export stitch schema mcp completions help" -f -a "describe" -d 'List the annotations in a project'
complete -c tellshot -n "__fish_tellshot_using_subcommand help; and not __fish_seen_subcommand_from targets capture new describe edit export stitch schema mcp completions help" -f -a "edit" -d 'Apply edits (JSON) to a project, all or nothing. See `tellshot schema edit`'
complete -c tellshot -n "__fish_tellshot_using_subcommand help; and not __fish_seen_subcommand_from targets capture new describe edit export stitch schema mcp completions help" -f -a "export" -d 'Render a project to a PNG with annotations flattened (redactions destroy pixels)'
complete -c tellshot -n "__fish_tellshot_using_subcommand help; and not __fish_seen_subcommand_from targets capture new describe edit export stitch schema mcp completions help" -f -a "stitch" -d 'Stitch overlapping frames of a scrolling page into one tall image'
complete -c tellshot -n "__fish_tellshot_using_subcommand help; and not __fish_seen_subcommand_from targets capture new describe edit export stitch schema mcp completions help" -f -a "schema" -d 'Print the JSON Schema for edits or documents'
complete -c tellshot -n "__fish_tellshot_using_subcommand help; and not __fish_seen_subcommand_from targets capture new describe edit export stitch schema mcp completions help" -f -a "mcp" -d 'Run a Model Context Protocol server on stdio for AI agents'
complete -c tellshot -n "__fish_tellshot_using_subcommand help; and not __fish_seen_subcommand_from targets capture new describe edit export stitch schema mcp completions help" -f -a "completions" -d 'Print a shell completion script, for example `tellshot completions bash`'
complete -c tellshot -n "__fish_tellshot_using_subcommand help; and not __fish_seen_subcommand_from targets capture new describe edit export stitch schema mcp completions help" -f -a "help" -d 'Print this message or the help of the given subcommand(s)'
