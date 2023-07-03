# aliesce

Write, save and run scripts in multiple languages from a single source file.

- [Why?](#why)
- [How?](#how)
  - [Source file setup](#source-file-setup)
  - [Running aliesce](#running-aliesce)
  - [There's more...](#theres-more)
    - [Specifying paths](#specifying-paths)
    - [Extending commands](#extending-commands)
    - [Avoiding stages](#avoiding-stages)
    - [Labelling scripts](#labelling-scripts)
- [Options](#options)
  - [Provision in-file](#provision-in-file)
- [Streams](#streams)
- [Defaults](#defaults)
- [Getting started](#getting-started)
- [Making changes](#making-changes)
  - [Tests](#tests)
- [Development plan](#development-plan)

## Why?

For smoother development of related code, to keep the source about as closely collocated as possible, or for practice, for a more direct absorption of syntax and idiom.

## How?

By providing a simple tag line above each script containing values to be used in saving and running it.

### Source file setup

Create a file for your scripts. Give it any name, and any extension or none. Use the current default name - 'src.txt' - to avoid passing an argument later.

Add the scripts to the file.

Immediately above each script, insert a tag line starting by default with `###`. Include in the tag line the following elements:

- the file extension for that language, or the full output filename including extension, or the full output path including directory and extension
- the command to be run, if any, e.g. the program to be used to run the file and any arguments to pass to that program - the path to the file is added as the final argument by default

Ensure each element is separated by one or more spaces.

For example, a possible tag line for a script in Elixir:

```
### exs elixir -r setup
```

This tells aliesce to save the script following the tag line in a file with the `exs` extension, then run that file with the `elixir` command, applying one option, to require a file named 'setup'.

A template source file can be created and scripts written in other files appended to an existing file via the command line (see [Options](#options) below).

### Running aliesce

If aliesce is compiled and ready to go (see [Getting started](#getting-started) below), run the `aliesce` command, adding the source file path if not the default.

For example, for a source file named only 'src':

```shell
aliesce src
```

The script files are saved and run in order of appearance in the source file.

### There's more...

#### Specifying paths

The stem of the output filename will be the stem of the source filename, i.e. 'src' by default. The file is saved by default to a folder in the current directory named `scripts`, which is created if not present. This default directory can be overridden via the command line (see [Options](#options) below).

For an output file named 'script.exs', the following would be used:

```
### script.exs elixir -r setup
```

For an output directory named 'elixir' holding 'script.exs':

```
### elixir/script.exs elixir -r setup
```

For a subdirectory within the default or overridden output directory, a placeholder can be used, by default `>`. For an output path of 'scripts/elixir/script.exs', i.e. with the default output directory name and the subdirectory and script named as above:

```
### >/elixir/script.exs elixir -r setup
```

#### Extending commands

For a command in which the path to the file is not the last argument, e.g. when piping to another program, a placeholder can be used, by default `><`. The whole is then run by the default program-flag pair `bash -c`. For a command of `bash -c "elixir -r setup scripts/src.exs | sort"`:

```
### exs elixir -r setup >< | sort
```

The output path of a different script can be selected by using its number in the placeholder. For the output path of script no. 1, rather than the fixed 'setup':

```
### exs elixir -r >1< >< | sort
```

#### Avoiding stages

To avoid a script being saved and run, simply include the `!` signal as a tag line element, before the extension or full output filename or path:

```
### ! script.exs elixir -r setup
```

To save the script but avoid the run stage, include the `!` signal as an element after the extension or full output filename or path but before the command to run the code:

```
### script.exs ! elixir -r setup
```

Alternatively, a specific subset of scripts can be included (see [Options](#options) below), to avoid the need to add tag line elements to others.

#### Labelling scripts

To add a label to a script, include it after the tag head and follow it with the tag tail, which is `#` by default:

```
### script label # script.exs elixir -r setup
```

Spacing between tag head and tail is retained for list entries (see [Options](#options) below).

## Options

The following can be passed to `aliesce` before any source file path:

- `--dest` / `-d`  `DIRNAME`, to set the default output dirname ('scripts') to `DIRNAME`
- `--list` / `-l`, to print for each script in the source (def. 'src.txt') its number and tag line content, without saving or running
- `--only` / `-o`  `SUBSET`, to include only the scripts the numbers of which appear in `SUBSET`, comma-separated and/or as ranges, e.g. `-o 1,3-5`
- `--push` / `-p`  `LINE` `PATH`, to append to the source (def. 'src.txt') `LINE`, adding the tag head if none, followed by the content at `PATH` then exit
- `--edit` / `-e`  `N` `LINE`, to update the tag line for script number N to LINE, adding the tag head if none, then exit
- `--init` / `-i`, to add a source at the default path ('src.txt') then exit
- `--version` / `-v`, to show name and version number then exit
- `--help` / `-h`, to show usage, flags available and notes then exit

### Provision in-file

Any or all of the options above can also be selected by providing their arguments in the source file itself, avoiding the need to list them with each use of the `aliesce` command.

Arguments provided in-file are simply placed above the initial tag line, arranged in the usual order, whether on a single line or multiple. They are processed each time the file is handled by aliesce.

Arguments passed directly on the command line are processed first, followed by those in the file, with the latter overriding the former in the event that an option is selected using both approaches.

This is similar to the use of the source file directly via hashbang, described in [Getting started](#getting-started) below.

## Streams

One or more paths can be piped to `aliesce` to append the content at each to the source file as a script, auto-preceded by a tag line including the `!` signal, then exit.

## Defaults

The default core path, tag, signal, placeholder and command values are defined close to the top of the project source file, i.e. 'src/main.rs', should you prefer to modify any pre-compilation (see [Getting started](#getting-started) below).

The default temporary test directory is defined close to the top of the test module, also in the project source file.

## Getting started

With Rust and Cargo installed, at the root of the aliesce directory run `cargo build --release` to compile. The binary is created in the 'target/release' directory.

The binary can be run with the command `./aliesce` while in the same directory, and from elsewhere using the pattern `path/to/aliesce`. It can be run from any directory with `aliesce` by placing it in a directory listed in `$PATH`, presumably '/bin' or '/usr/bin'.

A source file can be used directly by adding to the top of the file a hashbang with the path to the aliesce binary, e.g. `#!/usr/bin/aliesce`. If flags are to be passed (see [Options](#options) above), it may be possible to use the `env` binary with its split string option, e.g. `#!/bin/env -S aliesce <flag>[ ...]`. This inclusion of flags is similar to the approach described in [Provision in-file](#provision-in-file) above.

## Making changes

Running the tests after making changes and adding tests to cover new behaviour is recommended.

### Tests

The tests can be run with the following command:

```
cargo test
```

For the purpose of testing a subset of CLI options a temporary test directory is created (see [Defaults](#defaults) above).

The tests themselves are in the test module at the base of the file.

## Development plan

The following are the expected next steps in the development of the code base. The general medium-term aim is a convenient parallel scripting tool. Pull requests are welcome for these and other potential improvements.

- add source file variables available to tag line and script:
  - passed to aliesce via CLI
  - declared in file, including from the environment
  - for defaults
- extend and/or revise the set of placeholders for:
  - all default path parts
  - use across save path and command
- provide tag line options for:
  - multiple save paths
  - auxiliary commands
- provide or extend CLI options for:
  - output verbosity
  - applying a single stage
  - listing save paths
  - importing a script to an arbitrary position
  - interaction with existing scripts:
    - reordering
    - deleting
- refactor as more idiomatic
- improve error handling
- extend test module
